import { createRoot } from "react-dom/client";

import { DriveFileBrowser } from "../src/browser/DriveFileBrowser.tsx";
import type { BackendPort } from "../src/ipc/port.ts";
import type { AccountDto, DriveFileItemDto, JobDto } from "../src/ipc/types.ts";
import "../src/App.css";

const account: AccountDto = {
  id: "account-1",
  googlePermissionId: "permission-1",
  email: "owner@example.test",
  displayName: "Owner",
  label: null,
  authStatus: "CONNECTED",
  connectedAt: "2026-01-01T00:00:00.000Z",
  lastAuthenticatedAt: "2026-01-01T00:00:00.000Z",
  updatedAt: "2026-01-01T00:00:00.000Z",
  removedAt: null,
};

const file: DriveFileItemDto = {
  id: "file-1",
  name: "report-after-reauth.txt",
  mimeType: "text/plain",
  isFolder: false,
  folderId: null,
  folderResourceKey: null,
  resourceKey: null,
  size: 12,
  modifiedTime: null,
  owners: [],
  webViewLink: null,
  canTransferOwnership: true,
  isOwner: true,
  shortcutTargetId: null,
};

const state: Window["driveAuthQa"] = {
  listCalls: 0,
  reauthenticationCalls: 0,
  accountRefreshCalls: 0,
  authorizationRequired: true,
  cancelFirstReauthentication: true,
  commands: [],
};
window.driveAuthQa = state;

const backend = {
  listDriveFiles: async () => {
    state.listCalls += 1;
    if (state.authorizationRequired) {
      throw new Error("account requires re-authentication");
    }
    return { items: [file], nextPageToken: null };
  },
  reauthenticateAccount: async (accountId: string) => {
    state.reauthenticationCalls += 1;
    state.commands.push(`reauthenticate:${accountId}`);
    if (state.cancelFirstReauthentication) {
      state.cancelFirstReauthentication = false;
      throw new Error("OAuth authorization was cancelled.");
    }
    state.authorizationRequired = false;
    return { ...account, authStatus: "CONNECTED" as const };
  },
} as unknown as BackendPort;

async function refreshAccountStatus(accountId: string): Promise<AccountDto["authStatus"] | null> {
  state.accountRefreshCalls += 1;
  if (accountId !== account.id) {
    return null;
  }
  return state.authorizationRequired ? "REAUTH_REQUIRED" : "CONNECTED";
}

function recordAnnouncement(message: string) {
  const announcement = document.getElementById("drive-auth-announcement");
  if (announcement === null) {
    throw new Error("Drive auth QA announcement region is missing.");
  }
  announcement.textContent = message;
}

function rejectUnexpectedMigration(job: JobDto): never {
  throw new Error(`Migration should not start during auth recovery: ${job.id}`);
}

function rejectUnexpectedAddAccount(): never {
  throw new Error("Account addition should not start during auth recovery.");
}

const root = document.getElementById("root");
if (!root) {
  throw new Error("Drive auth QA root element is missing.");
}

createRoot(root).render(
  <DriveFileBrowser
    account={account}
    accounts={[account]}
    backend={backend}
    onAnnounce={recordAnnouncement}
    onMigrationStarted={rejectUnexpectedMigration}
    onAddAccount={rejectUnexpectedAddAccount}
    onRefreshAccountStatus={refreshAccountStatus}
  />,
);
