# GDOM Code & Engineering Conventions

This document outlines the coding standards, language idioms, architectural conventions, and development practices required for contributing to **GDOM**.

---

## 1. Core Principles

1. **Safety First**: GDOM transfers ownership of irreplaceable personal Google Drive files. Code must be deterministic, auditable, and resilient to sudden termination.
2. **Clean Boundaries**: Adhere strictly to Clean Architecture. High-level policies must never depend on low-level implementation details.
3. **Zero AI / Compiler Slop**: No unused abstractions, premature generalizations, unchecked unwrap calls, or silenced compiler warnings.
4. **Test-Driven Invariants**: Critical invariants (token routing, account immutability, state transitions) must be verified by automated tests using mock HTTP.

---

## 2. Rust Backend Conventions

### 2.1 Toolchain & Edition
- **Rust Edition**: 2024 edition (`rust-toolchain.toml`).
- **Standard Formatting**: Always format with `cargo fmt`.
- **Zero Clippy Warnings**: All code must pass `cargo clippy --all-targets --all-features -- -D warnings`. Warnings are treated as build errors.

### 2.2 Type Safety & Domain Modeling
- **Parse, Don't Validate**: Never pass naked strings or integers across layer boundaries. Wrap primitives in domain newtypes:
  ```rust
  // GOOD: Type-safe, invariant-preserving identifiers
  #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
  pub struct AccountId(u128);

  #[derive(Clone, Debug, Eq, Hash, PartialEq)]
  pub struct GooglePermissionId(String);

  // BAD: Primitive obsession, easy to mix up arguments
  pub fn connect(account_id: u128, permission_id: String) { ... }
  ```
- **Exhaustive Matching**: Always match enums exhaustively. Avoid wildcard matches (`_ => ...`) on domain states so that compiler checks alert developers when new states are introduced:
  ```rust
  // GOOD: Compiler forces handling if new JobStatus is added
  match self.status {
      JobStatus::Draft => { /* ... */ }
      JobStatus::Scanning => Err(JobError::AccountPairLocked),
  }
  ```
- **Error Handling**:
  - Never use `.unwrap()` or `.expect()` in production application, domain, or infrastructure code. Use `?` or explicit error matching.
  - Define structured, typed error enums implementing `std::error::Error` and `fmt::Display`.
  - Provide meaningful error messages and implement `.source()` to preserve causal error chains.
- **Redaction of Sensitive Information**:
  - Never expose OAuth tokens, refresh tokens, PKCE verifiers, or authorization codes in logs or debug dumps.
  - Implement custom `fmt::Debug` for types containing credentials:
    ```rust
    impl fmt::Debug for OAuthGrant {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("OAuthGrant([REDACTED])")
        }
    }
    ```

### 2.3 Layering & Modularity
- **Domain Purity**: Code in `domain/` must never import `tauri`, `sqlx`, `rusqlite`, `reqwest`, `keyring`, or external cloud SDKs.
- **Consumer-Owned Ports**: Interfaces (traits) are defined in the layer that *uses* them (`application/`), not where they are implemented (`infrastructure/`):
  ```rust
  // application/connect_account.rs owns the port
  pub trait TokenExchangePort {
      fn exchange_code(&self, grant: OAuthGrant)
          -> impl std::future::Future<Output = Result<TokenResponse, TokenExchangeError>> + Send;
  }
  ```
- **Keep Files Focused**: Follow the ~250 LOC guideline where feasible. Avoid monolithic files and split complex services into cohesive sub-modules.
- **Compensating Rollback in Local Persistence**: Multi-step local state mutations (such as saving an account in SQLite followed by storing credentials in the OS keychain) must provide compensating rollback if subsequent local steps fail. Note: Automatic rollback does not apply to Google Drive ownership transfers, which require manual intervention and reconciliation.

### 2.4 Async & Concurrency Rules
- Use Tokio async primitives (`tokio::sync::Mutex`, `CancellationToken`).
- Scope lock holding times to the absolute minimum. Never hold a mutex across network I/O unless explicitly coordinating a single-flight operation (like an account refresh mutex).
- For cancellation, accept a `tokio_util::sync::CancellationToken` and check `token.is_cancelled()` or select on `token.cancelled()` in loops.

---

## 3. Frontend Conventions (React & TypeScript)

### 3.1 Tech Stack
- **Framework**: React 19 with TypeScript (~5.8).
- **Bundler**: Vite.
- **Package Manager**: `pnpm` (version locked via `package.json` / CI).

### 3.2 Strict TypeScript Standards
- `strict: true` and `noImplicitAny: true` must always remain active.
- **Strictly Prohibited**:
  - `as any` or `any` type escapes.
  - `@ts-ignore` or `@ts-expect-error`.
- Define explicit interfaces/types for all Tauri IPC payloads and responses.

### 3.3 Linting & Formatting
- Code must pass `pnpm lint` (`eslint . --max-warnings 0`) with zero warnings.
- Keep dependencies lean. Avoid bloated third-party component libraries when semantic HTML and clean CSS suffice.

### 3.4 Accessibility (WCAG 2.2 AA)
- Normal text contrast must be at least **4.5:1** against the background.
- Large text, focus indicators, and meaningful boundaries must be at least **3:1**.
- **No status by color alone**: Always pair color indicators with text labels, aria attributes, or distinct icons.
- Ensure complete keyboard operability: all buttons, dialogs, and interactive controls must have visible, high-contrast `:focus-visible` styling.

---

## 4. Repository & Git Conventions

### 4.1 Line Endings & Encoding
- **LF Line Endings**: All tracked text files (Rust, TS, TSX, CSS, JSON, Markdown, YAML) **must use LF line endings**, enforced via `.gitattributes` and `.editorconfig`.
- **UTF-8**: All text files must be saved with UTF-8 encoding without BOM.
- Never let Windows Git configuration automatically convert files to CRLF.

### 4.2 Commit Message Protocol
Follow the Conventional / Lore commit message standard:

```
<type>(<scope>): <concise description in imperative mood>

[optional body explaining why, not what]

[optional issue/PR reference: (#123)]
```

#### Allowed Types:
- `feat`: A new user-facing or architectural capability.
- `fix`: A bug fix.
- `docs`: Documentation changes only.
- `refactor`: Code change that neither fixes a bug nor adds a feature.
- `test`: Adding or correcting tests.
- `chore`: Build system, CI, toolchain, or dependency updates.

#### Allowed Scopes:
- `domain`, `application`, `infrastructure`, `runtime`, `commands`, `ui`, `oauth`, `ci`.

#### Examples:
- `feat(application): add connect account service with self-documenting ports and rollback`
- `fix(oauth): preserve legitimate callbacks under local handler exhaustion`
- `docs(architecture): specify token-routing and security boundaries`

### 4.3 Git Discipline
- Keep branches focused and short-lived (`feat/...`, `fix/...`, `docs/...`).
- Atomic commits: Each commit represents a coherent, buildable, testable change.
- Never commit secrets, credentials, API keys, or machine-specific artifacts.

---

## 5. Continuous Integration (CI) Gates

Every Pull Request must pass the CI workflow (`.github/workflows/ci.yml`) on Windows:

| Check | Command | Description |
|---|---|---|
| Rust Formatting | `cargo fmt --check` | Enforces standard formatting and LF line endings |
| Rust Clippy | `cargo clippy --all-targets --all-features -- -D warnings` | Zero compiler and linter warnings |
| Rust Tests | `cargo test --all-features` | Runs all unit and contract tests |
| Frontend Lint | `pnpm lint` | ESLint validation with zero warnings |
| Frontend Build | `pnpm build` | TypeScript type-checking and Vite bundling |
