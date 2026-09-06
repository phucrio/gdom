export const ACCOUNTS_LOAD_FAILED = "Could not load the account registry from the local backend.";

export const CONNECT_TITLE = "Connect a Google account";
export const CONNECT_SIGN_IN = "Sign in with Google";
export const CONNECT_SIGN_IN_BUSY = "Waiting for browser…";
export const CONNECT_CANCEL = "Cancel";
export const CONNECT_BACK = "Back";
export const CONNECT_CONFIG_LOADING = "Checking OAuth configuration…";
export const CONNECT_READY_DEFAULT =
  "GDOM will open Google sign-in in your system browser.";
export const CONNECT_READY_CUSTOM = "Using a custom Desktop OAuth client imported from Advanced options.";
export const CONNECT_SECRET_REQUIRED =
  "Google Desktop clients require a client secret. Import the JSON from Google Cloud Console (Desktop app). GDOM stores it in Windows Credential Manager and never shows it or keeps it in source.";
export const CONNECT_ADVANCED_SUMMARY = "Advanced: your own Desktop OAuth client";
export const CONNECT_ADVANCED_HELP =
  "Create an OAuth client of type Desktop app, download the JSON, and import it. The client secret stays in Windows Credential Manager. It never enters this window or the git tree. You can also set GDOM_GOOGLE_CLIENT_ID and GDOM_GOOGLE_CLIENT_SECRET.";
export const CONNECT_IMPORT_JSON = "Import Desktop client JSON";
export const CONNECT_RESET_DEFAULT = "Use GDOM default";
export const CONNECT_CONFIG_LOAD_FAILED = "Could not read OAuth configuration.";
export const CONNECT_FAILED = "Could not connect the account.";
export const CONNECT_BROWSER_ANNOUNCEMENT = "Opening Google sign-in in the system browser.";
export const CONNECT_SUCCESS_ANNOUNCEMENT = "Account connected.";
export const CONNECT_IMPORT_SAVED = "Desktop client credentials imported.";
export const CONNECT_RESET_ANNOUNCEMENT = "OAuth client reset to the GDOM default.";

export function referencingJobsLabel(count: number): string {
  return count === 1 ? "1 referencing job" : `${count} referencing jobs`;
}
