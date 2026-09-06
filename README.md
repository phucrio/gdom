# GDOM — Google Drive Owner Migrator

Local-first desktop software for planning and executing Google Drive ownership transfers between connected personal Gmail accounts.

GDOM runs on Windows 11. It connects personal Gmail accounts (`@gmail.com` / `@googlemail.com`) through the system browser, stores refresh tokens in Windows Credential Manager, and keeps Drive metadata and checkpoints in local SQLite. Source and target are chosen per job from an Account Registry; accounts have no permanent role.

## Current status

Backend waves 1–5 are in place: OAuth PKCE, account lifecycle, job persistence, recursive scan, dry-run preflight, idempotent pending-owner/accept/verify, canary, and a single global mutation lease.

Scan, canary, and bulk run on background workers so Tauri commands return immediately. Remaining desktop E2E gaps are a Jobs list, live wizard pause/resume wiring, and a full dry-run item review. Live Drive mutation is not enabled in CI. Do not transfer production data; any live canary needs dedicated test accounts and explicit confirmation.

Tracking: [issue #19](https://github.com/phucrio/gdom/issues/19).

## Product guardrails

- Accounts are a registry; source and target are selected per migration job.
- A job has exactly one distinct source and target; the pair becomes immutable when scanning starts.
- Only one job may issue ownership mutations at a time.
- Consumer-account transfers require a source `pendingOwner` request and a target acceptance; every request must use the account-specific OAuth token.
- OAuth tokens remain in the Rust backend; refresh tokens live in Windows Credential Manager.
- OAuth requests full Drive access because GDOM must list and transfer arbitrary existing items; the consent flow must justify this immediately before opening the system browser.
- Dry run and a mandatory canary precede bulk transfer. There is no automatic rollback.
- Live Drive mutation requires explicit user confirmation. Automated tests use mock HTTP by default. Ordinary CI never calls live Drive.

## Architecture decisions

Durable product and architecture decisions are recorded in [docs/DECISIONS.md](docs/DECISIONS.md). Local implementation plans are intentionally excluded from Git.

## Local development

Install the Linux WebKit/RSVG prerequisites from the [Tauri guide](https://v2.tauri.app/start/prerequisites/), then run:

```sh
pnpm install
pnpm tauri dev
```

For frontend-only work:

```sh
pnpm dev
```

Windows 11 is the supported runtime. Linux and macOS secure-store adapters are deferred beyond the MVP. The repository enforces LF for all tracked text files, so do not override `.gitattributes` with machine-specific line-ending conversions.

## License

GDOM is licensed under the [GNU General Public License v3.0 or later](LICENSE) (`GPL-3.0-or-later`).
