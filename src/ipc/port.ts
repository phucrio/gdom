import type {
  AccountDto,
  AccountReferencesDto,
  DriveFileListDto,
  DryRunExport,
  JobDto,
  JobItemsPage,
  ListDriveFilesInput,
  ListJobsFilter,
  OAuthConfigDto,
  RenameDriveItemInput,
  RootValidation,
  StartTransferOperationInput,
  TrashDriveItemInput,
} from "./types.ts";

/** UI-owned backend contract. Transport and secrets stay behind the adapter. */
export type BackendPort = {
  listAccounts(): Promise<AccountDto[]>;
  getOAuthConfig(): Promise<OAuthConfigDto>;
  resetOAuthConfig(): Promise<OAuthConfigDto>;
  beginAccountConnection(attemptId: string): Promise<void>;
  cancelAccountConnection(attemptId: string): Promise<void>;
  connectAccount(attemptId: string): Promise<AccountDto>;
  reauthenticateAccount(accountId: string): Promise<AccountDto>;
  updateAccountLabel(accountId: string, label: string | null): Promise<AccountDto>;
  disconnectAccount(accountId: string): Promise<void>;
  removeAccount(accountId: string): Promise<void>;
  deleteLocalAccountData(accountId: string, confirmation: true): Promise<void>;

  openDriveItem(fileId: string): Promise<void>;
  listDriveFiles(input: ListDriveFilesInput): Promise<DriveFileListDto>;
  renameDriveItem(input: RenameDriveItemInput): Promise<void>;
  trashDriveItem(input: TrashDriveItemInput): Promise<void>;
  startTransferOperation(input: StartTransferOperationInput): Promise<JobDto>;

  createJob(sourceAccountId: string, targetAccountId: string): Promise<JobDto>;
  updateDraftJobAccounts(
    jobId: string,
    sourceAccountId: string,
    targetAccountId: string,
  ): Promise<JobDto>;
  listJobs(filter?: ListJobsFilter): Promise<JobDto[]>;
  getJob(jobId: string): Promise<JobDto>;
  deleteDraftJob(jobId: string): Promise<void>;
  getAccountReferences(accountId: string): Promise<AccountReferencesDto>;
  validateRoot(jobId: string, input: string): Promise<RootValidation>;
  addRoot(jobId: string, input: string): Promise<JobDto>;
  removeRoot(jobId: string, rootId: string): Promise<JobDto>;
  startScan(jobId: string): Promise<JobDto>;
  pauseScan(jobId: string): Promise<JobDto>;
  listJobItems(jobId: string, filter?: string | null, page?: number): Promise<JobItemsPage>;
  exportDryRun(jobId: string, destination: string): Promise<DryRunExport>;
  startCanary(jobId: string, confirmationEmail: string): Promise<JobDto>;
  continueMigration(jobId: string): Promise<JobDto>;
  pauseMigration(jobId: string): Promise<JobDto>;
  resumeMigration(jobId: string): Promise<JobDto>;
  cancelMigration(jobId: string): Promise<JobDto>;

  subscribe(event: string, listener: () => void): Promise<() => void>;
};
