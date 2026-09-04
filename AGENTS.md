# AGENTS.md — Agent & Contributor Guide for GDOM

Welcome to **GDOM (Google Drive Owner Migrator)**. This guide provides AI agents and human contributors with essential context, architectural rules, repository layout, and index to technical documentation.

---

## 1. Project Mission & Identity

**GDOM** is a local-first desktop application engineered to safely and recursively transfer Google Drive file and folder ownership between connected personal Gmail accounts (`@gmail.com` / `@googlemail.com`).

- **Runtime Target**: Windows 11 desktop application.
- **Frontend Stack**: React 19, TypeScript (~5.8), Vite, pnpm.
- **Backend Stack**: Tauri 2, Rust (2024 edition), Tokio, SQLite (WAL mode), Windows Credential Manager (`keyring`).
- **License**: GNU General Public License v3.0 or later (`GPL-3.0-or-later`).

---

## 2. Non-Negotiable Core Guardrails

Any modification made by an agent or developer must uphold these invariants:

1. **Token Isolation**:
   - OAuth access and refresh tokens **never** cross into the React frontend WebView.
   - Refresh tokens are stored exclusively in the OS Keychain (Windows Credential Manager).
   - Access tokens are held strictly in memory.
   - Secrets and tokens must implement redacted debug formatting (never print to logs).
2. **Token Routing Invariant**:
   - **Source Account Token**: Scanning folder trees, reading item metadata, creating or updating `pendingOwner` permissions.
   - **Target Account Token**: Accepting ownership transfer (`role=owner`), post-transfer verification.
3. **No Ambient Account**:
   - Every Google Drive operation requires an explicit `AccountContext`.
4. **Account Pair Immutability**:
   - A `MigrationJob` has exactly one source and one target account (`source != target`).
   - The account pair becomes strictly immutable once scanning begins.
5. **Single Mutation Lease**:
   - Only one migration job globally may issue ownership mutations at a time.
6. **No Live Mutation in Tests**:
   - Automated tests and CI must use mock HTTP (e.g. `wiremock`) or mock ports.
   - Real integration tests require an explicit environment variable and dedicated test accounts.
7. **Zero Remote Leaks & AI Telemetry**:
   - No Drive metadata, credentials, or file paths may be sent to AI models, telemetry servers, or cloud backends.

---

## 3. Documentation Index

Detailed architectural specifications, engineering standards, and product decisions are maintained under `docs/`:

| Document | Description |
|---|---|
| [docs/architecture.md](docs/architecture.md) | **System Architecture**: Clean Architecture layers, multi-account domain model, security boundaries, loopback listener design, and token-routing invariants. *(Also indexed as [docs/architecure.md](docs/architecure.md))* |
| [docs/code-convention.md](docs/code-convention.md) | **Code Conventions**: Rust 2024 idioms, strict TypeScript standards, error handling without unwrap, WCAG 2.2 AA accessibility, LF line endings, and Conventional commit rules. |
| [docs/code-review.md](docs/code-review.md) | **Code Review Guidelines**: Reviewer checklists, verification gates (Security, Architecture, Invariants, Durability), and author responsibilities. |
| [docs/DECISIONS.md](docs/DECISIONS.md) | **Architectural Decision Records (ADR)**: Scope boundaries, OAuth PKCE flow, loopback connection limits, full Drive scope rationale, and rate limit policies. |
| [docs/PLAN.md](docs/PLAN.md) | **Master Implementation Plan**: Comprehensive multi-account specifications, state machines, implementation waves (0 through 6), and Definition of Done. |
| [docs/LIMITED_USE_DISCLOSURE.md](docs/LIMITED_USE_DISCLOSURE.md) | **Limited Use Disclosure**: Google API Services User Data Policy adherence. |
| [docs/PRIVACY_POLICY_DRAFT.md](docs/PRIVACY_POLICY_DRAFT.md) | **Privacy Policy Draft**: Local-first data governance and user privacy commitments. |

---

## 4. Repository Structure & Responsibility Boundaries

```
gdom/
|-- .github/workflows/ci.yml       # Windows-based CI pipeline
|-- docs/                          # Architecture, conventions, ADRs, policies
|-- src/                           # Frontend React application
|   |-- App.tsx                    # Main shell component
|   |-- main.tsx                   # React entrypoint
|   +-- App.css                    # UI styles (WCAG 2.2 AA accessible)
|-- src-tauri/                     # Trusted Rust backend
|   |-- Cargo.toml                 # Backend dependencies (Rust 2024)
|   |-- tauri.conf.json            # Tauri v2 configuration & permissions
|   |-- build.rs                   # Tauri build script
|   +-- src/
|       |-- lib.rs                 # Composition root & Tauri initialization
|       |-- domain/                # Entities (Account, Job), invariants (Pure Rust, 0 external deps)
|       |-- application/           # Use cases (ConnectAccountService) & consumer-owned ports
|       |-- infrastructure/        # Adapters (Google API, OAuth, Keychain, SQLite)
|       |-- runtime/               # Workers, rate limiters, retry policies, scheduling
|       +-- commands/              # Thin Tauri IPC adapters & input validation
+-- package.json                   # Frontend scripts & tooling dependencies
```

---

## 5. Development & Verification Commands

All changes must pass local verification before submitting a PR:

### Rust Backend
```powershell
# Check code formatting (LF line endings enforced)
cargo fmt --check --manifest-path src-tauri/Cargo.toml

# Run Clippy with warnings as errors
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings

# Run all unit and contract tests
cargo test --manifest-path src-tauri/Cargo.toml --all-features
```

### Frontend
```powershell
# Run ESLint (zero warnings allowed)
pnpm lint

# TypeScript compilation & Vite build
pnpm build
```

### Full Desktop Dev
```powershell
# Launch Tauri 2 desktop application in development mode
pnpm tauri dev
```

---

## 6. Commit & Pull Request Discipline

1. **Branch Naming**:
   - Features: `feat/<feature-name>`
   - Bug fixes: `fix/<bug-name>`
   - Documentation: `docs/<topic>`
2. **Commit Messages**: Follow Conventional / Lore commits:
   - `feat(scope): concise description`
   - `fix(scope): concise description`
   - `docs(scope): concise description`
3. **Pull Requests**:
   - Target the `main` branch.
   - Include a concise summary of changes, motivation, test verification evidence, and security evaluation.
   - Verify that all CI checks pass.
