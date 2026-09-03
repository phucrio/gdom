# GDOM — Google Drive Owner Migrator

Local-first desktop software for planning and executing Google Drive ownership transfers between connected personal Gmail accounts.

The application is currently an initialized Tauri 2 + React foundation with a Rust 2024 backend. It does not connect to Google, persist credentials, or mutate Drive data yet.

## Product guardrails

- Accounts are a registry; source and target are selected per migration job.
- A job has exactly one distinct source and target; the pair becomes immutable when scanning starts.
- Only one job may issue ownership mutations at a time.
- Consumer-account transfers require a source `pendingOwner` request and a target acceptance; every request must use the account-specific OAuth token.
- OAuth tokens remain in the Rust backend; refresh tokens will live in the OS keychain.
- Live Drive mutation requires explicit user confirmation. Tests will use mock HTTP by default.

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

Windows 11 is the supported runtime. The repository enforces LF for all tracked text files, so do not override `.gitattributes` with machine-specific line-ending conversions.

## License

GDOM is licensed under the [GNU General Public License v3.0 or later](LICENSE) (`GPL-3.0-or-later`).
