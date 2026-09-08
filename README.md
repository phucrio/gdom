# GDOM — Google Drive Owner Migrator

Local-first desktop software for planning and executing Google Drive ownership transfers between connected personal Gmail accounts.

GDOM targets Windows 11, macOS and Linux on x64 and ARM64. It connects personal Gmail accounts (`@gmail.com` / `@googlemail.com`) through the system browser, stores refresh tokens in Windows Credential Manager, macOS Keychain or Linux Secret Service, and keeps Drive metadata and checkpoints in local SQLite. Source and target are chosen per job from an Account Registry; accounts have no permanent role.

## Current status

Backend waves 1–5 are in place: OAuth PKCE, account lifecycle, job persistence, recursive scan, dry-run preflight, idempotent pending-owner/accept/verify, canary, and a single global mutation lease.

Scan, canary, and bulk run on background workers so Tauri commands return immediately. The new-job wizard can pause and resume those workers and shows halt reasons as text. The Jobs list reopens persisted jobs after restart; resume is always an explicit click and uses the job's stored account pair. Dry-run review shows skip categories, quota, a paginated item list, and export; canary stays behind an explicit Continue after scan. Live Drive mutation is not enabled in CI. Do not transfer production data; any live canary needs dedicated test accounts and explicit confirmation.

Tracking: [issue #19](https://github.com/phucrio/gdom/issues/19).

## Product guardrails

- Accounts are a registry; source and target are selected per migration job.
- A job has exactly one distinct source and target; the pair becomes immutable when scanning starts.
- Only one job may issue ownership mutations at a time.
- Consumer-account transfers require a source `pendingOwner` request and a target acceptance; every request must use the account-specific OAuth token.
- Google sign-in uses a Desktop OAuth client (RFC 8252). The client ID is embedded; the client secret is never committed. Local testing sets `GDOM_GOOGLE_CLIENT_SECRET`; signed release builds inject `GDOM_DEFAULT_CLIENT_SECRET` from the protected `release` environment secret.
- OAuth tokens remain in the Rust backend; refresh tokens live in the operating system credential store.
- Backend logs are written to the local app-data `logs/gdom.log` file, rotated at 10 MB and kept to five files. Tokens and secrets are redacted before a line is stored.
- OAuth requests full Drive access because GDOM must list and transfer arbitrary existing items; the consent flow must justify this immediately before opening the system browser.
- Dry run and a mandatory canary precede bulk transfer. In the Drive browser, **Start transfer** authorizes the selected operation: a fully verified canary proceeds to the remaining items automatically; canary problems stop for review. There is no automatic rollback.
- Live Drive mutation requires explicit user confirmation. Automated tests use mock HTTP by default. Ordinary CI never calls live Drive.

## Architecture decisions

Durable product and architecture decisions are recorded in [docs/DECISIONS.md](docs/DECISIONS.md). Local implementation plans are intentionally excluded from Git.

## Local development

Install the Linux WebKit/RSVG prerequisites from the [Tauri guide](https://v2.tauri.app/start/prerequisites/), then run:

```powershell
pnpm install
$env:GDOM_GOOGLE_CLIENT_SECRET = "GOCSPX-your-desktop-client-secret"
pnpm tauri dev
```

Google's Desktop token endpoint requires that secret. Do not commit it. Enable the Google Drive API on the Cloud project and add your Gmail as an OAuth test user. If a leftover custom client ID is stored from an older build, use **Advanced → Use GDOM default**.

CI builds six unsigned test packages. Version tags build signed draft releases with `GDOM_DEFAULT_CLIENT_SECRET` and updater signing settings from GitHub Actions. Follow [RELEASING.md](RELEASING.md) for SemVer, changelog, platform prerequisites, signing and publication.

For frontend-only work:

```sh
pnpm dev
```

The build matrix covers Windows and macOS installers plus Linux AppImages, each on x64 and ARM64. Linux needs a compatible graphical desktop and unlocked Secret Service login collection; Ubuntu 22.04 is the build baseline. Native install and upgrade acceptance is tracked separately from compilation and remains required before claiming release readiness. The repository enforces LF for all tracked text files, so do not override `.gitattributes` with machine-specific line-ending conversions.

## License

GDOM is licensed under the [GNU General Public License v3.0 or later](LICENSE) (`GPL-3.0-or-later`).
