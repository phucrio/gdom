# Changelog

User-visible changes are recorded here. Version selection and release operations follow [RELEASING.md](RELEASING.md).

## [Unreleased]

## [0.1.1] - 2026-10-03

### Features

- Connect multiple personal Google accounts with Google sign-in, retain account identity across restarts, and store refresh tokens in each operating system's secure credential store.
- Browse Drive files and folders, inspect owners, navigate shortcuts, use item actions, and review per-account storage usage.
- Review recursive scans with dry runs before migration. Run canary-gated ownership transfers; queue, pause, and resume jobs from persisted checkpoints, with one active mutation job and exportable final reports.
- Provide native desktop packages for Windows, macOS, and Linux on x64 and ARM64. Check signed stable updates and release notes, confirm installation, and defer updates while migration work is active.
- Save sanitized per-item Drive errors locally and expose details in migration progress and exported reports.

### Fixes

- Recover custom OAuth secrets after credential-store unlock and reauthenticate rejected Drive tokens; refresh the account registry and reload the current folder with its latest sort order.
- Preserve scan scope, transfer progress, checkpoints, and job state across retries. Persist worker halts before releasing the mutation lease and clear stale authentication warnings after successful resume.
- Preserve selected-account context, resource keys, and shortcut navigation while browsing Drive.
- Keep logs redacted and Windows log rotation bounded.

### Security

- Harden OAuth loopback callback handling against malformed or stalled requests and handler exhaustion while preserving valid Google sign-in outcomes.

### Upgrade actions and known limitations

- Existing `0.1.0` installations need a manual bootstrap install to gain the updater.
- Linux requires a compatible graphical desktop and an unlocked Secret Service login collection; automatic updates apply to AppImage installations.
- The macOS app is not Apple Developer ID signed or notarized in v0.1.1. Gatekeeper may warn or block first launch; users may need to approve it manually. Tauri updater artifacts remain signed.
