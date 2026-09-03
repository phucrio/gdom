# Initial decisions

## Scope

GDOM starts as a Tauri 2 desktop app with React/TypeScript in the webview and Rust in the trusted backend. It runs on Windows 11 and serves only the owner's personal Gmail accounts. Google Workspace, Shared Drives, service accounts, content copying, telemetry, and automatic rollback are out of scope.

Tracked text uses LF through `.gitattributes` and `.editorconfig`, independent of each developer's Windows or Linux Git configuration. Windows batch files are the sole CRLF exception. CI runs the validation lane on Windows.

Development and production use separate Google Cloud projects. The personal production app will use an External OAuth consent screen in the In production state. OAuth authorization always uses the system browser and loopback callback, never the Tauri WebView.

## Ownership contract

For consumer Google accounts, source and target must complete a two-step transfer:

1. Source creates or updates the target permission with `type=user`, `role=writer`, and `pendingOwner=true`.
2. Target updates that permission with `role=owner` and `transferOwnership=true`.

The backend will always use the source token for scanning and the pending-owner request, then the target token for acceptance and verification. Ownership-transfer notification emails are mandatory and cannot be suppressed.

## Architecture boundary

The domain layer will not depend on Tauri, SQLite, or Google. Application services depend on small ports; Google Drive, SQLite, keychain, and Tauri commands are adapters. The composition root remains in `src-tauri/src/lib.rs`. No cloud backend may receive OAuth tokens or Drive metadata.

The initial Rust source tree uses five responsibility boundaries:

- `domain`: account, job, item, and migration invariants.
- `application`: use-case orchestration and policy-owned ports.
- `infrastructure`: Google Drive, SQLite, and keychain adapters.
- `runtime`: checkpointed workers, retries, and single-job scheduling.
- `commands`: thin Tauri adapters.

Feature files are added only when their behavior is implemented; the scaffold does not pre-create one file per planned service.

## Scope and privacy gate

Wave 0 will evaluate `drive.file` against the required root selection, recursive scan, and ownership-transfer flow. The app does not request full `drive` until the spike documents why it is necessary. If requested, the consent flow gives an in-app justification immediately before authorization.

Drive metadata, file content, credentials, and derived data never go to analytics or AI. A published Privacy Policy and visible Limited Use disclosure are production-release blockers. The local-data deletion flow removes selected keychain credentials and SQLite account/job metadata after confirmation; it does not revoke the Google grant unless separately requested.

## Rate limits and recovery

The default transfer concurrency is one. `sharingRateLimitExceeded` is a persisted pause (`SOURCE_RATE_LIMITED` or `WAITING_FOR_QUOTA`), never a fast retry. A migration may resume in a future app session only after loading its persisted checkpoint and reconciling the remote item state.

## Accessibility

The UI targets WCAG 2.2 AA: 4.5:1 normal text contrast, 3:1 large text and component/focus contrast, and no status expressed by color alone.

## Delivery order

1. Create typed account and job contracts with mock HTTP tests.
2. Add account registry persistence and OAuth PKCE without exposing tokens to the frontend.
3. Add scanner, dry-run, and preflight checks.
4. Add idempotent pending-owner, acceptance, and verification phases with a mandatory canary.
5. Add single-job scheduling, checkpoint recovery, reports, and UI workflows.

## Validation rule

Real OAuth/Drive integration tests are ignored unless an explicit environment variable enables them. No live transfer occurs during development without the user's direct confirmation.
