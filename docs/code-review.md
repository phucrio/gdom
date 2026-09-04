# GDOM Code Review Guidelines

This document establishes the code review protocol, security checklists, architectural gates, and quality standards for all contributions to **GDOM**.

---

## 1. Code Review Philosophy

GDOM is a local-first desktop application granted full Google Drive scope (`https://www.googleapis.com/auth/drive`) to manage personal files and folders. In this context, bugs can cause irreversible data loss, permission orphaning, or credential exposure.

Reviewers and authors must approach every PR with rigorous attention to:
1. **Security & Privacy**: Zero data leakage, zero token exposure to the frontend, strict loopback listener limits.
2. **Correctness of Invariants**: Token routing (source vs. target), account pair immutability, and single mutation job lease.
3. **Resilience & Durability**: Deterministic state transitions, crash recovery from SQLite checkpoints, and no fast retries on quota exhaustion.
4. **Maintainability**: Clean Architecture, focused modules, and exhaustive error handling.

---

## 2. Pull Request Author Expectations

Before submitting a Pull Request, authors must ensure:

- [ ] **Small & Focused**: The PR addresses one logical task or feature slice. Avoid bundling unrelated refactors or formatting changes.
- [ ] **Descriptive PR Summary**:
  - **Summary**: What does this change achieve?
  - **Motivation**: Why was this approach chosen?
  - **Testing**: Evidence of test execution (test commands run, mock scenarios added).
  - **Security Considerations**: Any impact on tokens, network listeners, or permissions.
- [ ] **Self-Review Completed**: The author has reviewed their own diff, verified line endings (LF), and confirmed that no debug logs or temporary files are included.
- [ ] **Clean CI**: All local checks pass cleanly:
  ```powershell
  cargo fmt --check
  cargo clippy --all-targets --all-features -- -D warnings
  cargo test --all-features
  pnpm lint
  pnpm build
  ```

---

## 3. Reviewer Checklist & Verification Gates

Reviewers must evaluate PRs against the following non-negotiable gates:

### Gate 1: Security & Token Isolation (Critical)
- [ ] **No Token Leaks to WebView**: Under no circumstances should OAuth access tokens, refresh tokens, or authorization codes be passed to the frontend or exposed via Tauri commands.
- [ ] **Secret Redaction**: All structs containing credentials, verifiers, or authorization codes must implement custom `fmt::Debug` that prints `[REDACTED]`.
- [ ] **Zero Cloud / AI Telemetry**: Ensure no Drive metadata (filenames, IDs, hierarchy, file sizes) or user credentials are sent to any remote API, analytics platform, or AI model.
- [ ] **Loopback Listener Hardening**:
  - Listener binds strictly to `127.0.0.1:{port}`.
  - Connection capacity is bounded (max 16 concurrent connections with FIFO reaping).
  - State parameter is validated cryptographically before token exchange.

### Gate 2: Token-Routing Invariant
- [ ] **Explicit Token Selection**:
  - Scanning and metadata inspection must use the **Source Account Token**.
  - Creating or updating `pendingOwner` permissions must use the **Source Account Token**.
  - Accepting ownership transfers must use the **Target Account Token**.
  - Post-transfer verification must use the **Target Account Token**.
- [ ] **Context-Driven Requests**: Operations must receive an explicit `AccountContext`. There must never be an ambient or default account.
- [ ] **Mandatory Email Notification**: `sendNotificationEmail` must never be set to `false`.

### Gate 3: Clean Architecture & Boundaries
- [ ] **Domain Purity**: The `domain/` layer must contain no dependencies on `tauri`, `sqlx`, `rusqlite`, `reqwest`, `keyring`, or Google SDKs.
- [ ] **Ports & Inversion of Control**: Traits/ports are declared in the `application/` layer and owned by use cases, not infrastructure adapters.
- [ ] **Thin Tauri Commands**: Tauri commands in `commands/` must only validate untrusted input, invoke the appropriate application service, and return serializable DTOs.
- [ ] **Rollback Safety**: Multi-step persistence operations (e.g., database insert followed by keychain write) must provide compensating rollback if subsequent steps fail.

### Gate 4: Durability & Concurrency
- [ ] **Single Mutation Job Lease**: Ensure that only one job can hold the mutation lease at a time.
- [ ] **Mandatory Canary Gate**: Bulk migration must be preceded by a canary run (`RUNNING_CANARY`) and an explicit user confirmation gate (`CANARY_REVIEW`). Automatic progression to bulk transfer without user approval is strictly forbidden.
- [ ] **Account Pair Immutability**: The `(source, target)` pair must be locked once scanning begins.
- [ ] **Leaf-First Ordering**: File and folder migrations must process items in `depth DESC` order (deepest files -> child folders -> root folder).
- [ ] **Rate-Limiting Discipline**:
  - `sharingRateLimitExceeded` must trigger a persisted pause (`SOURCE_RATE_LIMITED` / `WAITING_FOR_QUOTA`).
  - Fast retries on sharing limits are **strictly forbidden**.

### Gate 5: Code Quality & Rust Idioms
- [ ] **No Unwraps**: No `.unwrap()` or `.expect()` calls in production application paths.
- [ ] **Domain Newtypes**: Primitives are wrapped in strongly-typed newtypes (`AccountId`, `GooglePermissionId`).
- [ ] **Exhaustive Matching**: Enums are matched explicitly without wildcard catch-alls (`_ => ...`) for domain states.
- [ ] **Typed Error Handling**: Errors implement `thiserror::Error` or `std::error::Error` with informative messages and proper `.source()` chains.

### Gate 6: Testing Standards
- [ ] **Mock HTTP Only**: Automated tests must run against mock HTTP servers (e.g. `wiremock`) or trait mocks.
- [ ] **No Live API Calls**: Live Google API calls are forbidden in automated test runs and CI.
- [ ] **Deterministic Execution**: Tests must not rely on wall-clock timings or race conditions.

---

## 4. Review Process & Etiquette

1. **Timely Feedback**: Aim to review PRs within 24–48 hours.
2. **Actionable Comments**: Clearly explain the reason for requested changes:
   - *Bad*: "Change this."
   - *Good*: "This unwrap could panic if the OS credential store is locked. Please map it to `ConnectAccountError::Keychain` to allow graceful recovery."
3. **Distinguish Must-Fix from Suggestions**: Use prefixes like:
   - `[BLOCKER]`: Invariant violation, security bug, or test failure.
   - `[QUESTION]`: Inquiring about design intent or edge case handling.
   - `[NIT]`: Minor styling or phrasing improvement (non-blocking).
4. **Approval & Merge**: Once all blocking items are addressed and CI is green, the PR can be approved and merged into `main`.
