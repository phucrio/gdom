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
  avatarUrl?: string | null;
}>;

export type OAuthConfigDto = AssertNoSecrets<{
  isConfigured: boolean;
  clientId: string | null;
  usingCustomOverride: boolean;
  canSignIn: boolean;
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

export const JOB_RUN_PHASES = ["scan", "canary", "bulk"] as const;

export type JobRunPhase = (typeof JOB_RUN_PHASES)[number];

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
  totalItems: number;
  eligibleItems: number;
  alreadyOwnedByTarget: number;
  notOwnedBySource: number;
  sharedDrive: number;
  shortcuts: number;
  trashed: number;
  otherIneligible: number;
  estimatedQuotaBytes: number;
  targetUsageBytes: number;
  targetLimitBytes: number | null;
  targetRemainingBytes: number | null;
}>;

export const JOB_ITEM_FILTERS = ["all", "eligible", "skipped", "ineligible"] as const;

export type JobItemFilter = (typeof JOB_ITEM_FILTERS)[number];

export type JobItemDto = AssertNoSecrets<{
  id: string;
  jobId: string;
  fileId: string;
  name: string;
  mimeType: string;
  depth: number;
  originalParentIds: string[];
  state: string;
  quotaBytesUsed: number | null;
}>;

export type JobItemsPage = AssertNoSecrets<{
  items: JobItemDto[];
  page: number;
  pageSize: number;
  total: number;
}>;

export type DryRunExport = AssertNoSecrets<{
  path: string;
  eligibleItems: number;
  quotaWarning: boolean;
}>;

export type MigrationProgress = AssertNoSecrets<{
  completed: number;
  failed?: number;
  skipped?: number;
  total: number;
  currentPath: string | null;
}>;

export type JobErrorEntry = AssertNoSecrets<{
  itemId: string;
  message: string;
  at: string;
}>;

export type ListJobsFilter = AssertNoSecrets<{
  status?: JobStatus;
  accountId?: string;
}>;

export type AccountJobRole = "source" | "target";

export type AccountJobReferenceDto = AssertNoSecrets<{
  jobId: string;
  status: JobStatus;
  role: AccountJobRole;
}>;

export type AccountReferencesDto = AssertNoSecrets<{
  accountId: string;
  jobs: AccountJobReferenceDto[];
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
  phase?: JobRunPhase;
  scan?: ScanSummary | null;
  progress?: MigrationProgress | null;
  errors?: JobErrorEntry[];
}>;

export type RootValidation = AssertNoSecrets<{
  folderId: string;
  name: string;
}>;

export const ACCOUNT_COMMANDS = {
  beginAccountConnection: "begin_account_connection",
  cancelAccountConnection: "cancel_account_connection",
  listAccounts: "list_accounts",
  configureOAuth: "configure_oauth",
  getOAuthConfig: "get_oauth_config",
  resetOAuthConfig: "reset_oauth_config",
  connectAccount: "connect_account",
  reauthenticateAccount: "reauthenticate_account",
  updateAccountLabel: "update_account_label",
  disconnectAccount: "disconnect_account",
  removeAccount: "remove_account",
  deleteLocalAccountData: "delete_local_account_data",
  getAccountReferences: "get_account_references",
} as const;

export const DRIVE_COMMANDS = {
  openDriveItem: "open_drive_item",
  listDriveFiles: "list_drive_files",
  renameDriveItem: "rename_drive_item",
  trashDriveItem: "trash_drive_item",
  startTransferOperation: "start_transfer_operation",
} as const;

export type DriveFileOwnerDto = AssertNoSecrets<{
  avatarUrl?: string | null;
  permissionId: string;
  emailAddress: string | null;
}>;

export type DriveFileItemDto = AssertNoSecrets<{
  id: string;
  name: string;
  mimeType: string;
  isFolder: boolean;
  folderId: string | null;
  folderResourceKey?: string | null;
  resourceKey?: string | null;
  size: number | null;
  modifiedTime: string | null;
  owners: DriveFileOwnerDto[];
  webViewLink: string | null;
  canTransferOwnership: boolean;
  isOwner: boolean;
  shortcutTargetId: string | null;
}>;

export type DriveFileListDto = AssertNoSecrets<{
  items: DriveFileItemDto[];
  nextPageToken: string | null;
}>;

export type ListDriveFilesInput = AssertNoSecrets<{
  accountId: string;
  folderId?: string | null;
  folderResourceKey?: string | null;
  pageToken?: string | null;
  pageSize?: number | null;
  orderBy?: string | null;
}>;

export type OpenDriveItemInput = AssertNoSecrets<{
  accountId: string;
  fileId: string;
  resourceKey?: string | null;
}>;

export type RenameDriveItemInput = AssertNoSecrets<{
  accountId: string;
  fileId: string;
  newName: string;
}>;

export type TrashDriveItemInput = AssertNoSecrets<{
  accountId: string;
  fileId: string;
}>;

export type StartTransferOperationInput = AssertNoSecrets<{
  sourceAccountId: string;
  targetAccountId: string;
  rootFileIds: string[];
  recursive: boolean;
}>;

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
  listJobItems: "list_job_items",
  exportDryRun: "export_dry_run",
  startCanary: "start_canary",
  continueMigration: "continue_migration",
  pauseMigration: "pause_migration",
  resumeMigration: "resume_migration",
  cancelMigration: "cancel_migration",
  retryFailedItems: "retry_failed_items",
  queueJob: "queue_job",
} as const;

export const IPC_EVENTS = {
  accountRegistryChanged: "account-registry-changed",
  jobStatusChanged: "job-status-changed",
  jobListChanged: "job-list-changed",
  scanProgress: "scan-progress",
  migrationProgress: "migration-progress",
  itemStateChanged: "item-state-changed",
  canaryCompleted: "canary-completed",
  migrationCompleted: "migration-completed",
} as const;
