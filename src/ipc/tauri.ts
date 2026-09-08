import { invoke, type InvokeArgs } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import { formatIpcError, toIpcError } from "./errors.ts";
import type { BackendPort } from "./port.ts";
import { assertNoSecretFields } from "./secrets.ts";
import {
  ACCOUNT_COMMANDS,
  DRIVE_COMMANDS,
  JOB_COMMANDS,
  type AccountDto,
  type AccountReferencesDto,
  type DriveFileListDto,
  type DryRunExport,
  type FinalReportExport,
  type JobDto,
  type JobItemsPage,
  type ListJobsFilter,
  type OAuthConfigDto,
  type RootValidation,
  type StorageQuotaDto,
} from "./types.ts";

async function invokeCommand<T>(command: string, args?: InvokeArgs): Promise<T> {
  try {
    const result = await invoke<T>(command, args);
    if (typeof result === "object" && result !== null) {
      assertNoSecretFields(result);
      if (Array.isArray(result)) {
        for (const item of result) {
          if (typeof item === "object" && item !== null) {
            assertNoSecretFields(item);
          }
        }
      }
    }
    return result;
  } catch (error) {
    const ipcError = toIpcError(error, command);
    const wrapped = new Error(formatIpcError(ipcError));
    Object.defineProperty(wrapped, "cause", { value: error, enumerable: false });
    throw wrapped;
  }
}

export function createTauriBackend(): BackendPort {
  return {
    listAccounts: () => invokeCommand<AccountDto[]>(ACCOUNT_COMMANDS.listAccounts),
    getAccountStorage: (accountId) => invokeCommand<StorageQuotaDto>(DRIVE_COMMANDS.getAccountStorage, { input: { accountId } }),
    getOAuthConfig: () => invokeCommand<OAuthConfigDto>(ACCOUNT_COMMANDS.getOAuthConfig),
    resetOAuthConfig: () => invokeCommand<OAuthConfigDto>(ACCOUNT_COMMANDS.resetOAuthConfig),
    beginAccountConnection: (attemptId) => invokeCommand<void>(ACCOUNT_COMMANDS.beginAccountConnection, { attemptId }),
    cancelAccountConnection: (attemptId) => invokeCommand<void>(ACCOUNT_COMMANDS.cancelAccountConnection, { attemptId }),
    connectAccount: (attemptId) => invokeCommand<AccountDto>(ACCOUNT_COMMANDS.connectAccount, { attemptId }),
    reauthenticateAccount: (accountId) =>
      invokeCommand<AccountDto>(ACCOUNT_COMMANDS.reauthenticateAccount, {
        input: { accountId },
      }),
    updateAccountLabel: (accountId, label) =>
      invokeCommand<AccountDto>(ACCOUNT_COMMANDS.updateAccountLabel, {
        input: { accountId, label },
      }),
    disconnectAccount: (accountId) =>
      invokeCommand<void>(ACCOUNT_COMMANDS.disconnectAccount, {
        input: { accountId },
      }),
    removeAccount: (accountId) =>
      invokeCommand<void>(ACCOUNT_COMMANDS.removeAccount, {
        input: { accountId },
      }),
    deleteLocalAccountData: (accountId, confirmation) =>
      invokeCommand<void>(ACCOUNT_COMMANDS.deleteLocalAccountData, {
        input: { accountId, confirmation },
      }),

    openDriveItem: (input) => invokeCommand<void>(DRIVE_COMMANDS.openDriveItem, { input }),
    listDriveFiles: (input) =>
      invokeCommand<DriveFileListDto>(DRIVE_COMMANDS.listDriveFiles, { input }),
    renameDriveItem: (input) =>
      invokeCommand<void>(DRIVE_COMMANDS.renameDriveItem, { input }),
    trashDriveItem: (input) =>
      invokeCommand<void>(DRIVE_COMMANDS.trashDriveItem, { input }),
    startTransferOperation: (input) =>
      invokeCommand<JobDto>(DRIVE_COMMANDS.startTransferOperation, { input }),

    createJob: (sourceAccountId, targetAccountId) =>
      invokeCommand<JobDto>(JOB_COMMANDS.createJob, {
        input: { sourceAccountId, targetAccountId },
      }),
    updateDraftJobAccounts: (jobId, sourceAccountId, targetAccountId) =>
      invokeCommand<JobDto>(JOB_COMMANDS.updateDraftJobAccounts, {
        input: { jobId, sourceAccountId, targetAccountId },
      }),
    listJobs: (filter?: ListJobsFilter) =>
      invokeCommand<JobDto[]>(JOB_COMMANDS.listJobs, { filter: filter ?? null }),
    getJob: (jobId) => invokeCommand<JobDto>(JOB_COMMANDS.getJob, { input: { jobId } }),
    deleteDraftJob: (jobId) =>
      invokeCommand<void>(JOB_COMMANDS.deleteDraftJob, { input: { jobId } }),
    getAccountReferences: (accountId) =>
      invokeCommand<AccountReferencesDto>(ACCOUNT_COMMANDS.getAccountReferences, {
        input: { accountId },
      }),
    validateRoot: (jobId, input) =>
      invokeCommand<RootValidation>(JOB_COMMANDS.validateRoot, {
        input: { jobId, input },
      }),
    addRoot: (jobId, input) =>
      invokeCommand<JobDto>(JOB_COMMANDS.addRoot, { input: { jobId, input } }),
    removeRoot: (jobId, rootId) =>
      invokeCommand<JobDto>(JOB_COMMANDS.removeRoot, { input: { jobId, rootId } }),
    startScan: (jobId) =>
      invokeCommand<JobDto>(JOB_COMMANDS.startScan, { input: { jobId } }),
    pauseScan: (jobId) =>
      invokeCommand<JobDto>(JOB_COMMANDS.pauseScan, { input: { jobId } }),
    listJobItems: (jobId, filter, page) =>
      invokeCommand<JobItemsPage>(JOB_COMMANDS.listJobItems, {
        input: { jobId, filter: filter ?? null, page: page ?? 1 },
      }),
    reorderQueuedJob: (jobId, position) =>
      invokeCommand<JobDto>(JOB_COMMANDS.reorderQueuedJob, { input: { jobId, position } }),
    removeQueuedJob: (jobId) =>
      invokeCommand<JobDto>(JOB_COMMANDS.removeQueuedJob, { input: { jobId } }),
    exportFinalReport: (jobId, destination) =>
      invokeCommand<FinalReportExport>(JOB_COMMANDS.exportFinalReport, { input: { jobId, destination } }),
    exportDryRun: (jobId, destination) =>
      invokeCommand<DryRunExport>(JOB_COMMANDS.exportDryRun, {
        input: { jobId, destination },
      }),
    startCanary: (jobId, confirmationEmail) =>
      invokeCommand<JobDto>(JOB_COMMANDS.startCanary, {
        input: { jobId, confirmation: confirmationEmail },
      }),
    continueMigration: (jobId) =>
      invokeCommand<JobDto>(JOB_COMMANDS.continueMigration, { input: { jobId } }),
    pauseMigration: (jobId) =>
      invokeCommand<JobDto>(JOB_COMMANDS.pauseMigration, { input: { jobId } }),
    resumeMigration: (jobId) =>
      invokeCommand<JobDto>(JOB_COMMANDS.resumeMigration, { input: { jobId } }),
    cancelMigration: (jobId) =>
      invokeCommand<JobDto>(JOB_COMMANDS.cancelMigration, { input: { jobId } }),

    subscribe: async (event, listener) => {
      try {
        const unlisten = await listen(event, () => {
          listener();
        });
        return unlisten;
      } catch {
        return () => {
          /* not running inside Tauri */
        };
      }
    },
  };
}
