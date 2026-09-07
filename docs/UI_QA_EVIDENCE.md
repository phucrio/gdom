# UI QA evidence

This file records release-refinement checks using synthetic account and file data only. It must not contain Drive metadata from user accounts.

## Automated browser scenarios

`pnpm ui:qa` uses the local Vite fixture and bundled Playwright Chromium. It checks recipient selection before mutation, progress updates, pagination, recoverable errors, canary confirmation, pause/resume, account filtering, reconnect, and viewport containment at 375, 640, 768, 900, and 1280 px. It also asserts that the progress panel is not fixed, so it cannot cover workspace content.

The command writes screenshots to `GDOM_QA_OUTPUT` or the local temporary `gdom-progress-qa` directory. They are evidence artifacts and are intentionally not committed.

## Manual desktop gate

Before release, inspect the synthetic fixture in Chromium and the Tauri WebView at 1120 by 720, 720 by 520, and 1280 by 900. Verify 200% zoom, keyboard-only operation, visible focus, reduced motion, the recipient review before Start transfer, a cancel confirmation, a single history Close action, long Vietnamese names, and no overlay obscuring an actionable control.

Native WebView2 packaging verification remains a release gate because it requires the compile-time desktop client secret and is not exercised by pull-request validation.
