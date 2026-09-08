# Changelog

User-visible changes are recorded here. Version selection and release operations follow [RELEASING.md](RELEASING.md).

## [Unreleased]

### Features

- Check for signed stable updates at startup or from **Check for updates**, including before sign-in. Review version and release notes, confirm download, then confirm installation and restart.
- Defer installation while migration work is active so persisted checkpoints and account data are preserved.
- Build Windows, macOS and Linux desktop packages for x64 and ARM64. Store refresh tokens in each operating system's secure credential store.

### Fixes

- Retry loading the custom OAuth client secret after unlocking the credential store, so existing accounts can refresh tokens without reopening sign-in or restarting the app.

### Upgrade actions and known limitations

- Existing `0.1.0` installations need a manual bootstrap install to gain the updater.
- Linux requires a compatible graphical desktop and an unlocked Secret Service login collection; automatic updates apply to AppImage installations.
- Production signing settings and native install/upgrade acceptance on all six targets are required before release. Build artifacts alone do not establish release readiness.
