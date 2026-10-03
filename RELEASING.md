# Releasing GDOM

This is the canonical release procedure for maintainers and agents. Honor authorization already granted in the current session; do not ask again for covered actions. A request to release a version can authorize its merge, tag and publication, while a request limited to preparing a PR or draft does not. Ask only when a necessary action falls outside the authorized scope. Release authorization does not authorize live Drive transfers, and issue #1 remains open until its acceptance evidence is complete.

The compatibility-first process follows [quiche's release guide](https://github.com/cloudflare/quiche/blob/master/RELEASING.md): assess user impact, prepare a reviewed version PR, tag the merged commit, and verify the published result. GDOM distributes desktop applications, not independently published Rust crates.

## 1. Preflight and compatibility

- Start from a clean checkout of current `main`. Record the full candidate commit SHA and previous stable version. Preserve other worktrees and local data.
- Review changes to behavior, SQLite/checkpoint compatibility, account and credential identities, configuration, supported platforms and the updater contract. An API checker or commit prefix cannot establish compatibility on its own.
- Before `1.0.0`, compatible features and fixes increment patch; incompatible changes increment minor and reset patch. At `1.0.0` and later, incompatible changes increment major, compatible features minor, and compatible fixes patch. Example: compatible additions to `0.1.0` become `0.1.1`; incompatible changes become `0.2.0`.
- Stable release tags are exactly `vX.Y.Z`, without prerelease/build suffixes. Prereleases need a separate channel design and must never be routed through the stable manifest.
- Confirm availability of all six native runner lanes and production Tauri updater-signing settings before starting a release. For v0.1.1, macOS builds intentionally omit Apple Developer ID signing and notarization; this does not waive Tauri updater signatures or native acceptance. Missing updater credentials or native acceptance evidence remain blockers.

## 2. Prepare a release PR

1. Create a release-preparation branch from `main`. Update the application version in `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json` and the `gdom` entry in `src-tauri/Cargo.lock`. Update other lockfiles if their format records the app version; pnpm currently does not. Internal crates need no synchronized bump unless changed for an independent reason.
2. Move applicable `CHANGELOG.md` entries from `Unreleased` to `## [X.Y.Z] - YYYY-MM-DD`, leaving `Unreleased` available. Draft with `git-cliff --tag vX.Y.Z [BASE..HEAD]` and the tracked `cliff.toml`; review the actual release range, then curate user-visible outcomes instead of copying raw commit titles. Clearly identify breaking changes and security fixes, required upgrade actions and known limitations. Use Features/Fixes sections only when relevant.
3. Run `node scripts/release.mjs check`, `node --test scripts/release.test.mjs` and the frontend/Rust gates documented in `AGENTS.md`. Open a PR into `main` with the compatibility decision and evidence.
4. Require the `validate` aggregate check and all underlying jobs to succeed on the exact PR head; resolve actionable review findings. Merge when covered by the current session's authorization. Do not tag the pre-merge commit.

Implementation PRs normally keep the current version and add `Unreleased` notes. Do not choose a version merely because a feature was added.

## 3. Platform matrix and signing

| Platform | Native runner | Rust target | Distribution and updater |
|---|---|---|---|
| Windows x64 | `windows-latest` | `x86_64-pc-windows-msvc` | NSIS `.exe` |
| Windows ARM64 | `windows-11-arm` | `aarch64-pc-windows-msvc` | NSIS `.exe` |
| macOS Intel | `macos-15-intel` | `x86_64-apple-darwin` | DMG with Apple-unsigned app; Tauri updater-signed `.app.tar.gz` |
| macOS Apple Silicon | `macos-15` | `aarch64-apple-darwin` | DMG with Apple-unsigned app; Tauri updater-signed `.app.tar.gz` |
| Linux x64 | `ubuntu-22.04` | `x86_64-unknown-linux-gnu` | AppImage |
| Linux ARM64 | `ubuntu-22.04-arm` | `aarch64-unknown-linux-gnu` | AppImage |

Linux builds use Ubuntu 22.04's glibc baseline. Runtime requires a graphical desktop, WebKitGTK 4.1 dependencies and an unlocked Secret Service login collection. The AppImage updater applies to AppImage installations; this is not a promise of compatibility with every distribution. Windows requires Windows 11 and WebView2. macOS minimum deployment support must be checked against the built bundle and verified hardware before making a release claim. For v0.1.1, the macOS app is not Developer ID signed or notarized; Gatekeeper may warn or block first launch, and users may need to approve it manually ([Apple guidance](https://support.apple.com/guide/mac-help/open-a-mac-app-from-an-unknown-developer-mh40616/mac)). Architecture is native application architecture; Windows ARM64 NSIS itself can run under Windows emulation.

GitHub's [runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners) documents native ARM64 lanes. Runner labels, availability and toolchain versions must be rechecked if hosted jobs cannot start. Never substitute cross-compilation success for native execution evidence.

Create a protected GitHub Actions `release` environment before adding production secrets. Restrict its deployment policy to **tags matching `v*` only**, with no branch or pull-request rule, and require a release maintainer review. Keep the `ci` environment empty. Environment protection prevents same-repository PR workflows from accessing production credentials; an unsigned input alone is not a security boundary. See [GitHub environment protection](https://docs.github.com/en/actions/how-tos/deploy/configure-and-manage-deployments/manage-environments).

Store the following Tauri updater-signing secrets and public variable in the protected `release` environment, without putting values in issues, logs, source or agent messages. Apple signing/notarization secrets are required only when `apple_signed` is true. Migrate any existing repository/organization-level copies out of PR scope; do not retain duplicate production credentials accessible to ordinary CI:

- Variable `TAURI_UPDATER_PUBLIC_KEY`: the Tauri updater public key. The build injects it through `GDOM_UPDATER_PUBLIC_KEY`; only this public value is embedded in the application.
- Secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: password-protected updater key and password. Keep an encrypted offline backup with restricted custodians. Generate with `pnpm tauri signer generate --help` following the current [Tauri updater instructions](https://v2.tauri.app/plugin/updater/); never generate production keys inside ordinary PR CI.
- Secret `GDOM_DEFAULT_CLIENT_SECRET`: bundled Google Desktop OAuth client secret. Verify sign-in readiness with dedicated accounts before distributing installers.
- Optional Apple-signed macOS builds require `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD` and `APPLE_TEAM_ID`. For v0.1.1, `apple_signed` is false, so these are not required and the Mac app is not Apple-signed/notarized. The release workflow currently sets this mode explicitly; future signed Mac releases must set `apple_signed: true` after adding these secrets. When enabled, `APPLE_PASSWORD` is an app-specific password; follow [Tauri macOS signing](https://v2.tauri.app/distribute/sign/macos/).

Tauri updater signing authenticates the updater artifact bytes. It does not sign the whole `latest.json` document and does not replace Apple code signing/notarization or Windows Authenticode. For v0.1.1, Mac `.app.tar.gz` updater payloads remain Tauri-signed, but the `.app`/DMG are not Apple-signed or notarized. Windows installers currently do not claim Authenticode trust. GitHub HTTPS and repository write access protect manifest delivery; keep release write permissions narrowly scoped.

PR CI packages unsigned test artifacts without production secrets. These are not release installers, do not have bundled sign-in readiness, and must not be attached to stable releases. The tag workflow Tauri-signs all updater payloads; Apple code signing is controlled separately and is disabled for macOS in v0.1.1. Each lane runs native Rust checks/tests; the frontend gates run once.

## 4. Tag, build and inspect the draft

1. After authorized merge, fetch `main` and the tags. Identify the exact merged release commit. Confirm its version, changelog and CI evidence. When the current release authorization covers tagging, create an annotated `vX.Y.Z` tag on that commit and push that tag only. Do not request a second confirmation for an already authorized release.
2. `release.yml` rejects unstable or mismatched versions, a tag outside `main` history, an empty/missing dated changelog section and an existing release/draft. It runs frontend gates and six native build/test lanes on the tag commit, with Tauri updater signing enabled and Apple signing disabled for v0.1.1.
3. Each lane stages canonical version/OS/architecture names and a SHA-256 receipt. The aggregator checks matching version/SHA, all six platforms, artifact hashes and Tauri updater-signature presence before creating a draft with one `latest.json`. It rejects inconsistent Apple-signing modes across the two macOS receipts. Tauri signs each updater payload; staging runs the Rust verifier against the exact public key embedded in that release and refuses a signature/key mismatch. Installed clients independently verify signatures again.
4. Inspect the draft's eight distribution/update payloads, six updater `.sig` files and `latest.json`. Confirm the manifest has exactly `windows-x86_64`, `windows-aarch64`, `darwin-x86_64`, `darwin-aarch64`, `linux-x86_64`, `linux-aarch64`, each targeting its own versioned asset URL. Confirm the Mac app/DMG are explicitly Apple-unsigned and notes match the approved changelog.
5. Record workflow URL, exact SHA, artifact hashes, the macOS unsigned/not-notarized status and native acceptance evidence. A partially uploaded draft must remain unpublished; the workflow refuses reruns over an existing draft. Diagnose first; deleting an unpublished draft and retrying requires authority for that cleanup, which may already have been granted in the session. Never use clobber/force upload.

## 5. Native acceptance and publish

For each of the six targets, retain machine/OS/architecture, starting/target versions, tested SHA and results. Use synthetic local data and dedicated test accounts; never place tokens, Drive metadata or raw account data in public evidence.

- Install and launch the package; test secure-store round trip, locked/unavailable keychain recovery, local SQLite persistence and clean shutdown. The ignored synthetic credential check can be run with `GDOM_NATIVE_KEYCHAIN_TESTS=1 cargo test --manifest-path src-tauri/Cargo.toml native_keychain_round_trip -- --ignored` in an initialized desktop credential environment.
- Test OAuth browser launch and Cancel/Close/Escape followed by retry. No Drive ownership mutation is authorized by release checks.
- Check startup/manual update, stable/version/architecture selection, notes, both confirmations, download progress, offline/retry, signature rejection, install/relaunch and retained accounts/checkpoints. Confirm installation remains deferred during active jobs and that failed checkpoint persistence blocks installation.
- On macOS for v0.1.1, install the Apple-unsigned DMG, record the Gatekeeper first-launch path and any manual approval, then test update/relaunch. Tauri updater signature verification is separate from Apple code-signing status.
- Verify keyboard/focus and minimum-window layouts on each OS. Record missing native scenarios explicitly; headless frontend QA and compilation do not establish native installer behavior.
- Use two updater-enabled versions and an isolated test endpoint/build to validate upgrades before stable publication. Existing `0.1.0` installations have no updater and require a manual bootstrap install. A public draft is not the stable updater endpoint.

Once acceptance is complete and publication is covered by the current session's authorization, publish the draft as a stable/latest GitHub Release without requesting duplicate confirmation. Then independently download the published assets, verify expected hashes/signatures, check the public `releases/latest/download/latest.json`, and perform a stable-channel update on every target. If a native target is unavailable, leave its acceptance unchecked and do not claim six-target runtime completion or close issue #1.

## 6. Recovery

Never move a published tag or overwrite its artifacts. A code/data defect requires a new version with clear remediation notes. Stop promotion while investigating incomplete drafts. Removing or changing a published release can break clients and needs an explicit incident decision.

For planned updater-key rotation, first ship a bridge version signed by the old key that embeds the new public key; verify migration before signing later releases with the new key. Clients that skip the bridge may need manual installation, so preserve recovery instructions. Loss of the old private key prevents signing a trusted bridge: provide a separately verified manual installer and explain how to preserve local data. Replacing the repository variable alone does not rotate keys in installed clients.

Keep the application identifier, platform app-data location and credential service/account identities stable. SQLite migrations must remain compatible with the supported upgrade path. Back up local state before manual recovery; do not promise automatic rollback or downgrade compatibility.

The repository release skill is tracked separately in [issue #41](https://github.com/phucrio/gdom/issues/41); it must reference this procedure instead of duplicating it.
