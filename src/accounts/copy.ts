export const ACCOUNTS_LOAD_FAILED = "Could not load the account registry from the local backend.";

export const CONNECT_TITLE = "Connect a Google account";
export const CONNECT_SIGN_IN = "Sign in with Google";
export const CONNECT_SIGN_IN_BUSY = "Waiting for browser…";
export const CONNECT_CANCEL = "Cancel";
export const CONNECT_BACK = "Back";
export const CONNECT_CONFIG_LOADING = "Preparing Google sign-in…";
export const CONNECT_READY_DEFAULT =
  "GDOM will open Google sign-in in your system browser.";
export const CONNECT_READY_CUSTOM =
  "You are using custom sign-in settings. You can restore the GDOM defaults if you have trouble connecting.";
export const CONNECT_SECRET_REQUIRED =
  "Google sign-in is unavailable in this installation. Install the latest GDOM release or contact the person who provided this copy.";
export const CONNECT_RESET_DEFAULT = "Restore default sign-in settings";
export const CONNECT_CONFIG_LOAD_FAILED = "Could not prepare Google sign-in. Close this window and try again.";
export const CONNECT_FAILED = "Could not connect the account.";
export const CONNECT_BROWSER_ANNOUNCEMENT = "Opening Google sign-in in the system browser.";
export const CONNECT_SUCCESS_ANNOUNCEMENT = "Account connected.";
export const CONNECT_RESET_ANNOUNCEMENT = "Default sign-in settings restored.";

export function referencingJobsLabel(count: number): string {
  return count === 1 ? "1 referencing job" : `${count} referencing jobs`;
}
