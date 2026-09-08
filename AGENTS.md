# AGENTS.md — Agent & Contributor Guide for GDOM

Welcome to **GDOM (Google Drive Owner Migrator)**. This guide provides AI agents and human contributors with essential context, architectural rules, repository layout, and index to technical documentation.

---

## 1. Project Mission & Identity

**GDOM** is a local-first desktop application engineered to safely and recursively transfer Google Drive file and folder ownership between connected personal Gmail accounts (`@gmail.com` / `@googlemail.com`).

- **Runtime Targets**: Windows 11, macOS and Linux desktop applications, each on x64 and ARM64.
- **Frontend Stack**: React 19, TypeScript (~5.8), Vite, pnpm.
- **Backend Stack**: Tauri 2, Rust (2024 edition), Tokio, SQLite (WAL mode), OS-native credential stores (Windows Credential Manager, macOS Keychain, Linux Secret Service).
- **License**: GNU General Public License v3.0 or later (`GPL-3.0-or-later`).

---

## 2. Non-Negotiable Core Guardrails

Any modification made by an agent or developer must uphold these invariants:

1. **Token Isolation**:
   - OAuth access and refresh tokens **never** cross into the React frontend WebView.
   - Refresh tokens are stored exclusively in the OS credential store (Windows Credential Manager, macOS Keychain or Linux Secret Service).
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
| [RELEASING.md](RELEASING.md) | **Release procedure**: Compatibility-first SemVer, changelog, six-platform build/sign/draft/publish checks and recovery. |
| [docs/DESIGN.md](docs/DESIGN.md) | **UI/UX Design Direction**: Approved visual and interaction contract, file-type icons, responsive layouts, progress feedback, accessibility, and acceptance criteria. |
| [docs/architecture.md](docs/architecture.md) | **System Architecture**: Clean Architecture layers, multi-account domain model, security boundaries, loopback listener design, and token-routing invariants. |
| [docs/code-convention.md](docs/code-convention.md) | **Code Conventions**: Rust 2024 idioms, strict TypeScript standards, error handling without unwrap, WCAG 2.2 AA accessibility, LF line endings, and Conventional commit rules. |
| [docs/code-review.md](docs/code-review.md) | **Code Review Guidelines**: Reviewer checklists, verification gates (Security, Architecture, Invariants, Durability), and author responsibilities. |
| [docs/DECISIONS.md](docs/DECISIONS.md) | **Architectural Decision Records (ADR)**: Scope boundaries, OAuth PKCE flow, loopback connection limits, full Drive scope rationale, rate limit policies, and canary requirements. |
| [docs/LIMITED_USE_DISCLOSURE.md](docs/LIMITED_USE_DISCLOSURE.md) | **Limited Use Disclosure**: Google API Services User Data Policy adherence. |
| [docs/PRIVACY_POLICY_DRAFT.md](docs/PRIVACY_POLICY_DRAFT.md) | **Privacy Policy Draft**: Local-first data governance and user privacy commitments. |

---

## 4. Repository Structure & Responsibility Boundaries

```
gdom/
|-- .github/workflows/ci.yml       # Six-target desktop CI pipeline
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

Keep developer setup instructions in contributor documentation, never in release UI. Account screens must not display environment variable names, shell commands, OAuth client IDs, or CI secret configuration. Preserve user-facing permission explanations, privacy disclosures, and actionable sign-in recovery.

- Enable Google Drive API in the OAuth client's Google Cloud project and add dedicated Gmail test accounts as OAuth test users.
- For local sign-in, set `GDOM_GOOGLE_CLIENT_SECRET` in the shell before launching the app. Never commit or log the value.
- Release builds receive `GDOM_DEFAULT_CLIENT_SECRET` at compile time from the GitHub Actions protected `release` environment secret. Verify sign-in readiness before distributing an installer.
- If a stored custom OAuth client overrides the bundled client, use **Restore default sign-in settings** in the connect-account dialog to reset it explicitly. Do not silently overwrite custom settings.

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

### Dedicated live-test setup

The ignored live canary harness is documented in [docs/testing.md](docs/testing.md). Set `GDOM_LIVE_DRIVE_TESTS=1` and `GDOM_LIVE_MANIFEST` only in a local operator shell. The harness refuses CI and defaults to Drive-read-only preflight. Transfer requires selecting one `GDOM_LIVE_TARGET` (`B` or `C`) and confirming its exact root through `GDOM_LIVE_CONFIRM_ROOT` after explicit operator approval. Keep the manifest, database, raw reports and all credentials outside Git; never show these setup details in release UI. Three accounts ready for SSO do not establish test fixtures or authorize live mutations.
