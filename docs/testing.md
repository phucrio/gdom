# Dedicated Gmail canary

Automated tests use mock HTTP only. The ignored `live_drive_test::dedicated_gmail_canary` test is an operator-run integration harness, not part of CI. Its environment gate runs before loading a manifest, opening a database, reading Windows Credential Manager, or making a request. It refuses CI even when opted in.

## Prepare locally

1. Use a dedicated GDOM profile with exactly three personal Gmail test accounts A, B and C. If the normal profile contains other accounts or jobs, use a separate Windows user profile; do not edit or copy production account rows into a test database. Follow OAuth setup in `AGENTS.md`. Connect A twice through the app and record that exactly one active account row remains. Close GDOM before running the harness so another process cannot mutate ownership outside its lease.
2. In Google Drive, as A, create two separate disposable trees: one for B and one for C. Each tree contains a root folder, one subfolder and one harmless text file. Keep each tree between three and five total items. Create these outside GDOM to exercise full Drive scope. Do not use real documents, shortcuts, shared-drive items, or shared fixture IDs.
3. Copy the example below to a local directory outside the checkout. Replace IDs and addresses locally. Obtain account IDs from the dedicated profile's account list/database using local tooling; never send those rows to an AI assistant. Create the output directory first. The database and output directory must also be outside the checkout.

```json
{
  "database": "C:/Users/TEST_USER/AppData/Roaming/me.phucrio.gdom/gdom.db",
  "output_directory": "C:/Users/TEST_USER/gdom-canary/evidence",
  "accounts": [
    { "label": "A", "account_id": "1", "email": "REPLACE_A@gmail.com" },
    { "label": "B", "account_id": "2", "email": "REPLACE_B@gmail.com" },
    { "label": "C", "account_id": "3", "email": "REPLACE_C@gmail.com" }
  ],
  "fixtures": [
    { "target": "B", "root_id": "REPLACE_ROOT_B", "item_ids": ["REPLACE_ROOT_B", "REPLACE_SUBFOLDER_B", "REPLACE_FILE_B"] },
    { "target": "C", "root_id": "REPLACE_ROOT_C", "item_ids": ["REPLACE_ROOT_C", "REPLACE_SUBFOLDER_C", "REPLACE_FILE_C"] }
  ]
}
```

Use the application identifier from `src-tauri/tauri.conf.json` and the actual application data path for that Windows profile. No tokens or OAuth secrets belong in this manifest.

## Read-only Drive preflight

```powershell
$env:GDOM_LIVE_DRIVE_TESTS = '1'
$env:GDOM_LIVE_MANIFEST = 'C:/Users/TEST_USER/gdom-canary/manifest.json'
Remove-Item Env:GDOM_LIVE_TARGET -ErrorAction SilentlyContinue
Remove-Item Env:GDOM_LIVE_CONFIRM_ROOT -ErrorAction SilentlyContinue
cargo test --manifest-path src-tauri/Cargo.toml --lib live_drive_test::dedicated_gmail_canary -- --ignored --exact
```

Live evidence requires a clean committed checkout, including no untracked files; commit or move local source changes before running. The source revision is checked again immediately before mutations and before writing successful evidence.

This verifies all identities against Google and scans both trees using A. It creates local scan jobs and local `preflight-B.json` / `preflight-C.json`; it performs no Drive mutations. Every scanned ID must match the manifest exactly. Review the local manifest and both preflight results before authorizing a transfer. The example alone is not evidence that any fixture exists.

## Explicit transfer, one pair at a time

After the operator explicitly approves the exact B root and account pair, set:

```powershell
$env:GDOM_LIVE_TARGET = 'B'
$env:GDOM_LIVE_CONFIRM_ROOT = 'EXACT_ROOT_B_ID_FROM_MANIFEST'
cargo test --manifest-path src-tauri/Cargo.toml --lib live_drive_test::dedicated_gmail_canary -- --ignored --exact
```

The harness rechecks identity, ownership and scan contents before enabling mutations. Its adapter rejects mutation IDs outside the selected fixture, pending-owner calls using any token except A, and accept calls using any token except the selected target. It reuses the normal service, token refresh, Windows keychain and Google adapter. The adapter retains `sendNotificationEmail=true` for pending-owner changes; the recipient must separately confirm email receipt.

Inspect final TXT/CSV reports and verify B owns every item with unchanged parents. Only then separately approve C, replace both variables with C and its exact root, and run again. There is no automatic second job. An incomplete run is not a pass: preserve the local database and reports, inspect the job, and resolve it before retrying. Do not rerun blindly or reset item state to force a pass.

## Evidence and cleanup

Commit only a manually reviewed, sanitized summary containing the exact source commit SHA, package version, run timestamp, pair labels, item counts, identity/scan/owner/parents/token-routing results, duplicate-connection check and recipient email-receipt confirmation. `evidence-B.json` and `evidence-C.json` deliberately leave the last two manual checks as `OPERATOR_REQUIRED`; do not replace them without observation. Local final reports contain real IDs and account information and must not be uploaded or committed.

After reviewing reports, clean up only the exact fixture IDs from the manifest, using B for its tree and C for its tree. Use Drive manually; verify IDs before trashing. Never search/delete by a broad name, remove unrelated files, or automatically reverse ownership.

Issue #26 stays open until both live runs and all manual evidence are complete. Having three accounts ready for SSO does not establish fixtures, authorize a live transfer, or complete this checklist.

## Mock validation

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib live_drive_test
```

This runs gate, manifest, authorization and rejected-routing checks against a loopback-only client. The live test remains ignored.

On failure the test prints only a safe stage instruction. Once the output directory has been validated, it writes `failure.json` there with a static stage/category and next action; raw errors, account IDs, paths and tokens are never printed or copied into that diagnostic. Failures before output validation explicitly report that no local failure report was written.
