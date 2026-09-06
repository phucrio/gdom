export const ACCOUNTS_LOAD_FAILED = "Could not load the account registry from the local backend.";

export const CONNECT_TITLE = "Connect a Google account";
export const CONNECT_SIGN_IN = "Sign in with Google";
export const CONNECT_SIGN_IN_BUSY = "Waiting for browser…";
export const CONNECT_CANCEL = "Cancel";
export const CONNECT_BACK = "Back";
export const CONNECT_CONFIG_LOADING = "Checking OAuth configuration…";
export const CONNECT_READY_DEFAULT =
  "GDOM will open Google sign-in in your system browser.";
export const CONNECT_READY_CUSTOM =
  "A leftover custom OAuth client is stored locally. Reset to the GDOM default unless you still need it.";
export const CONNECT_SECRET_REQUIRED =
  "Local sign-in needs GDOM_GOOGLE_CLIENT_SECRET in your shell before launching the app. Release builds inject GDOM_DEFAULT_CLIENT_SECRET from CI. The secret is never committed or shown here.";
export const CONNECT_ADVANCED_SUMMARY = "Advanced: OAuth client source";
export const CONNECT_ADVANCED_HELP =
  "For local testing, set GDOM_GOOGLE_CLIENT_SECRET then run pnpm tauri dev. Enable the Google Drive API on the Cloud project and add your Gmail as an OAuth test user. Release builds receive GDOM_DEFAULT_CLIENT_SECRET at compile time from the GitHub repository secret.";
export const CONNECT_RESET_DEFAULT = "Use GDOM default";
export const CONNECT_CONFIG_LOAD_FAILED = "Could not read OAuth configuration.";
export const CONNECT_FAILED = "Could not connect the account.";
export const CONNECT_BROWSER_ANNOUNCEMENT = "Opening Google sign-in in the system browser.";
export const CONNECT_SUCCESS_ANNOUNCEMENT = "Account connected.";
export const CONNECT_RESET_ANNOUNCEMENT = "OAuth client reset to the GDOM default.";

export function referencingJobsLabel(count: number): string {
  return count === 1 ? "1 referencing job" : `${count} referencing jobs`;
}
