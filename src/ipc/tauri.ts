import { invoke, type InvokeArgs } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import { formatIpcError, toIpcError } from "./errors.ts";
import type { BackendPort } from "./port.ts";
import { assertNoSecretFields } from "./secrets.ts";
import {
  ACCOUNT_COMMANDS,
  JOB_COMMANDS,
  type AccountDto,
  type JobDto,
  type OAuthConfigDto,
  type RootValidation,
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
    getOAuthConfig: () => invokeCommand<OAuthConfigDto>(ACCOUNT_COMMANDS.getOAuthConfig),
    configureOAuth: (clientId) =>
      invokeCommand<void>(ACCOUNT_COMMANDS.configureOAuth, {
        input: { clientId },
      }),
    connectAccount: () => invokeCommand<AccountDto>(ACCOUNT_COMMANDS.connectAccount),
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

    createJob: (sourceAccountId, targetAccountId) =>
      invokeCommand<JobDto>(JOB_COMMANDS.createJob, {
        input: { sourceAccountId, targetAccountId },
      }),
    updateDraftJobAccounts: (jobId, sourceAccountId, targetAccountId) =>
      invokeCommand<JobDto>(JOB_COMMANDS.updateDraftJobAccounts, {
        input: { jobId, sourceAccountId, targetAccountId },
      }),
    getJob: (jobId) => invokeCommand<JobDto>(JOB_COMMANDS.getJob, { input: { jobId } }),
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
