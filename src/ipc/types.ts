/** IPC DTOs omit secret field names so tokens cannot type-check into the WebView. */

type ForbiddenSecretField =
  | "accessToken"
  | "refreshToken"
  | "pkceVerifier"
  | "authorizationCode"
  | "clientSecret"
  | "verifier";

type AssertNoSecrets<T> = Extract<keyof T, ForbiddenSecretField> extends never
  ? T
  : never;

export const AUTH_STATUSES = [
  "CONNECTED",
  "TOKEN_REFRESHING",
  "REAUTH_REQUIRED",
  "DISCONNECTED",
  "REMOVAL_PENDING",
] as const;

export type AuthStatus = (typeof AUTH_STATUSES)[number];

export const ACCOUNT_STATUS_BADGES = [
  "CONNECTED",
  "REAUTH_REQUIRED",
  "DISCONNECTED",
] as const;

export type AccountStatusBadge = (typeof ACCOUNT_STATUS_BADGES)[number];

export type AccountDto = AssertNoSecrets<{
  id: string;
  googlePermissionId: string;
  email: string;
  displayName: string;
  label: string | null;
  authStatus: AuthStatus;
  connectedAt: string;
  lastAuthenticatedAt: string;
  updatedAt: string;
  removedAt: string | null;
}>;

export type OAuthConfigDto = AssertNoSecrets<{
  isConfigured: boolean;
  clientId: string | null;
}>;

/** Client-ID-only configure payload. Client secrets never enter the WebView. */
export type ConfigureOAuthInput = AssertNoSecrets<{
  clientId: string;
}>;

export type AccountIdInput = AssertNoSecrets<{
  accountId: string;
}>;

export type UpdateAccountLabelInput = AssertNoSecrets<{
  accountId: string;
  label: string | null;
}>;

export type DeleteAccountDataInput = AssertNoSecrets<{
  accountId: string;
  confirmation: boolean;
}>;

export const JOB_STATUSES = [
  "DRAFT",
  "SCANNING",
  "READY_FOR_REVIEW",
  "RUNNING_CANARY",
  "CANARY_REVIEW",
  "QUEUED",
  "RUNNING",
  "PAUSING",
  "PAUSED",
  "CANCELLING",
  "CANCELLED",
  "COMPLETED",
  "COMPLETED_WITH_ERRORS",
  "FAILED",
  "AUTH_REQUIRED",
  "SOURCE_RATE_LIMITED",
  "WAITING_FOR_QUOTA",
] as const;

export type JobStatus = (typeof JOB_STATUSES)[number];

export type AccountSnapshotDto = AssertNoSecrets<{
  accountId: string;
  email: string;
  displayName: string;
  permissionId: string;
}>;

export type JobRoot = AssertNoSecrets<{
  id: string;
  jobId: string;
  rootFileId: string;
  rootName: string;
  validationStatus: string;
  createdAt: string;
}>;

export type ScanSummary = AssertNoSecrets<{
  files: number;
  folders: number;
  skipped: number;
  ineligible: number;
  quotaWarning: boolean;
}>;

export type MigrationProgress = AssertNoSecrets<{
  completed: number;
  total: number;
  currentPath: string | null;
}>;

export type JobErrorEntry = AssertNoSecrets<{
  itemId: string;
  message: string;
  at: string;
}>;

export type JobDto = AssertNoSecrets<{
  id: string;
  sourceAccountId: string;
  targetAccountId: string;
  sourceSnapshot: AccountSnapshotDto;
  targetSnapshot: AccountSnapshotDto;
  status: JobStatus;
  queuePosition: number | null;
  canarySize: number;
  createdAt: string;
  startedAt: string | null;
  completedAt: string | null;
  lastError: string | null;
  roots: JobRoot[];
  scan?: ScanSummary | null;
  progress?: MigrationProgress | null;
  errors?: JobErrorEntry[];
}>;

export type RootValidation = AssertNoSecrets<{
  folderId: string;
  name: string;
}>;

export const ACCOUNT_COMMANDS = {
  listAccounts: "list_accounts",
  configureOAuth: "configure_oauth",
  getOAuthConfig: "get_oauth_config",
  connectAccount: "connect_account",
  reauthenticateAccount: "reauthenticate_account",
  updateAccountLabel: "update_account_label",
  disconnectAccount: "disconnect_account",
  removeAccount: "remove_account",
  deleteLocalAccountData: "delete_local_account_data",
} as const;

export const JOB_COMMANDS = {
  createJob: "create_job",
  updateDraftJobAccounts: "update_draft_job_accounts",
  listJobs: "list_jobs",
  getJob: "get_job",
  deleteDraftJob: "delete_draft_job",
  validateRoot: "validate_root",
  addRoot: "add_root",
  removeRoot: "remove_root",
  startScan: "start_scan",
  pauseScan: "pause_scan",
  startCanary: "start_canary",
  continueMigration: "continue_migration",
  pauseMigration: "pause_migration",
  resumeMigration: "resume_migration",
  cancelMigration: "cancel_migration",
} as const;

export const IPC_EVENTS = {
  accountRegistryChanged: "account-registry-changed",
  jobStatusChanged: "job-status-changed",
  scanProgress: "scan-progress",
  migrationProgress: "migration-progress",
} as const;
