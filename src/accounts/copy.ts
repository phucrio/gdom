export const ACCOUNTS_LOAD_FAILED = "Could not load the account registry from the local backend.";

export const CONNECT_TITLE = "Connect a Google account";
export const CONNECT_SIGN_IN = "Sign in with Google";
export const CONNECT_SIGN_IN_BUSY = "Waiting for browser…";
export const CONNECT_CANCEL = "Cancel";
export const CONNECT_BACK = "Back";
export const CONNECT_CONFIG_LOADING = "Checking OAuth configuration…";
export const CONNECT_READY_DEFAULT =
  "GDOM will open Google sign-in in your system browser. No client ID is required.";
export const CONNECT_READY_CUSTOM = "Using a custom OAuth client ID from Advanced options.";
export const CONNECT_ADVANCED_SUMMARY = "Advanced: custom OAuth client ID";
export const CONNECT_ADVANCED_HELP =
  "Optional. Paste a Google Cloud OAuth desktop client ID to use your own Cloud project. GDOM never asks for or displays a client secret.";
export const CONNECT_CLIENT_ID_LABEL = "OAuth client ID";
export const CONNECT_SAVE_CUSTOM = "Save custom client ID";
export const CONNECT_RESET_DEFAULT = "Use GDOM default";
export const CONNECT_CLIENT_ID_REQUIRED = "Enter a Google Cloud OAuth client ID before saving.";
export const CONNECT_CONFIG_LOAD_FAILED = "Could not read OAuth configuration.";
export const CONNECT_FAILED = "Could not connect the account.";
export const CONNECT_BROWSER_ANNOUNCEMENT = "Opening Google sign-in in the system browser.";
export const CONNECT_SUCCESS_ANNOUNCEMENT = "Account connected.";
export const CONNECT_CUSTOM_SAVED = "Custom OAuth client ID saved.";
export const CONNECT_RESET_ANNOUNCEMENT = "OAuth client ID reset to the GDOM default.";

export function referencingJobsLabel(count: number): string {
  return count === 1 ? "1 referencing job" : `${count} referencing jobs`;
}
