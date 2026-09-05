export const LIMITED_USE_SENTENCE =
  "GDOM's use of information received from Google APIs will adhere to the Google API Services User Data Policy, including the Limited Use requirements.";

export const FULL_DRIVE_SCOPE_JUSTIFICATION =
  "GDOM requests full Google Drive access so it can list and transfer ownership of arbitrary existing items in the folders you select. The narrower per-file scope cannot provide the complete Drive listing this workflow needs. GDOM does not download, upload, copy, or analyze file content.";

export const SYSTEM_BROWSER_OAUTH_EXPLANATION =
  "Google sign-in opens in your system browser, not inside this window. GDOM never shows or stores access tokens, refresh tokens, authorization codes, or PKCE verifiers in the interface. Refresh tokens stay in the Windows Credential Manager; access tokens stay in backend memory.";

export const PRIVACY_POLICY_TITLE = "Privacy Policy";

export const PRIVACY_POLICY_SECTIONS = [
  {
    heading: "Data GDOM accesses",
    body: "GDOM accesses the Google Drive account identity, selected root and descendant metadata, permissions, and ownership state needed to scan and transfer ownership. It does not download, upload, copy, or analyze file content. GDOM requests the full Google Drive scope so it can list and transfer arbitrary existing items selected for migration.",
  },
  {
    heading: "Local storage",
    body: "Refresh tokens are stored only in the operating-system keychain. Account registry records, job checkpoints, and the minimum Drive metadata required to resume a job are stored in the local SQLite database on this device.",
  },
  {
    heading: "No remote processing",
    body: "GDOM has no backend that receives OAuth tokens or Drive metadata. It does not send Drive metadata, file content, credentials, or derived data to analytics or AI systems.",
  },
  {
    heading: "Deletion",
    body: "You can delete local account data. This removes the selected account credential from the OS keychain and its local account and job metadata after confirmation. It does not revoke Google authorization unless you explicitly choose a separate revoke action.",
  },
  {
    heading: "Google API Limited Use",
    body: LIMITED_USE_SENTENCE,
  },
] as const;

export const LIMITED_USE_TITLE = "Limited Use Disclosure";

export const LIMITED_USE_BODY = [
  LIMITED_USE_SENTENCE,
  "Full Drive access is required to list and transfer arbitrary existing items. GDOM reads Drive identity, folder and file metadata, permissions, and ownership state for items in selected trees. That data remains on this device. No Drive data is sent to analytics or AI.",
] as const;

export const QUOTA_WARNING =
  "Google Drive sharing rate limits can pause a transfer. If a sharing quota is exceeded, GDOM waits rather than retrying quickly.";
