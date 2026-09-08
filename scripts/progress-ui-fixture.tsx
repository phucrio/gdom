import { createRoot } from "react-dom/client";
import { useState } from "react";
import { App } from "../src/App.tsx";
import type { BackendPort } from "../src/ipc/port.ts";
import type { AccountDto, DriveFileItemDto, JobDto, JobItemDto } from "../src/ipc/types.ts";

const account = (id: string): AccountDto => ({ id, email: `${id}@gmail.com`, displayName: id,
  googlePermissionId: id, label: null, authStatus: "CONNECTED", connectedAt: "2026-09-07",
  lastAuthenticatedAt: "2026-09-07", updatedAt: "2026-09-07", removedAt: null });
const source = account("source");
const target = account("target");
const snapshot = (value: AccountDto) => ({ accountId: value.id, email: value.email,
  displayName: value.displayName, permissionId: value.googlePermissionId });
let current: JobDto = { id: "job-1", sourceAccountId: source.id, targetAccountId: target.id,
  sourceSnapshot: snapshot(source), targetSnapshot: snapshot(target), status: "RUNNING", phase: "bulk",
  queuePosition: null, canarySize: 5, createdAt: "2026-09-07T00:00:00Z", startedAt: null,
  completedAt: null, lastError: null, roots: [],
  progress: { completed: 0, total: 2, failed: 0, skipped: 0, currentPath: null } };
let queueJobs: JobDto[] | null = null;
let failQueue = false;
let failExport = false;
let queueGate: Promise<void> | null = null;
let releaseQueue: (() => void) | null = null;
let items: JobItemDto[] = [0, 1].map((index) => ({ id: `item-${index}`, jobId: current.id,
  fileId: `file-${index}`, name: index === 0 ? "Report.pdf" : "Folder", mimeType: index === 0 ? "application/pdf" : "application/vnd.google-apps.folder",
  depth: 0, originalParentIds: [], state: "ELIGIBLE", quotaBytesUsed: null }));
const driveItem: DriveFileItemDto = {
  id: "drive-item-1", name: "release-notes.md", mimeType: "text/markdown", isFolder: false,
  folderId: null, size: 128, modifiedTime: "2026-09-07T00:00:00Z", owners: [], webViewLink: null,
  canTransferOwnership: true, isOwner: true, shortcutTargetId: null,
};
const listeners = new Map<string, Set<() => void>>();
let failAccounts = false;
let delayJob = false;
let failJob = false;
let failStartTransfer = false;
let jobRequests = 0;
let releaseJob: (() => void) | null = null;
const commands: string[] = [];
function emit() { for (const callbacks of listeners.values()) for (const callback of callbacks) callback(); }
function unsupported(): never { throw new Error("Unsupported synthetic fixture command"); }
const transition = async (command: string, status: JobDto["status"]) => {
  commands.push(command); current = { ...current, status }; emit(); return current;
};
const backend: BackendPort = {
  getAccountStorage: async () => ({ usageBytes: 107374182400, limitBytes: 5497558138880 }),
  beginAccountConnection: unsupported,
  cancelAccountConnection: unsupported,
  listAccounts: async () => { if (failAccounts) throw new Error("Registry unavailable"); return [source, target]; },
  listJobs: async () => queueJobs ?? [current, { ...current, id: "other-job", sourceAccountId: "other", targetAccountId: "else", status: "COMPLETED" }],
  getJob: async (id) => {
    jobRequests += 1;
    const shouldFail = failJob;
    const result = { ...current, id, targetSnapshot: id === "other-job" ? { ...current.targetSnapshot, email: "another@gmail.com" } : current.targetSnapshot };
    if (delayJob) { delayJob = false; await new Promise<void>((resolve) => { releaseJob = resolve; }); }
    if (shouldFail) throw new Error("Progress unavailable");
    return result;
  },
  listJobItems: async (_id, _filter, page = 1) => ({ items: items.slice(page - 1, page), page, pageSize: 1, total: items.length }),
  openDriveItem: unsupported,
  subscribe: async (event, callback) => { const callbacks = listeners.get(event) ?? new Set(); callbacks.add(callback); listeners.set(event, callbacks); return () => { callbacks.delete(callback); }; },
  listDriveFiles: async () => ({ items: [driveItem], nextPageToken: null }),
  pauseMigration: () => transition("pauseMigration", "PAUSED"),
  pauseScan: () => transition("pauseScan", "PAUSED"),
  startScan: () => transition("startScan", "SCANNING"),
  resumeMigration: () => transition("resumeMigration", "RUNNING"),
  continueMigration: () => transition("continueMigration", "RUNNING"),
  startCanary: () => transition("startCanary", "RUNNING_CANARY"),
  cancelMigration: () => transition("cancelMigration", "CANCELLED"),
  reauthenticateAccount: async () => { commands.push("reauthenticateAccount"); source.authStatus = "CONNECTED"; emit(); return source; },
  getOAuthConfig: unsupported, resetOAuthConfig: unsupported, connectAccount: unsupported,
  updateAccountLabel: unsupported, disconnectAccount: unsupported, removeAccount: unsupported,
  deleteLocalAccountData: unsupported, renameDriveItem: unsupported, trashDriveItem: unsupported,
  startTransferOperation: async (input) => { if (failStartTransfer) throw new Error("Transfer start unavailable"); commands.push(`startTransferOperation:${input.sourceAccountId}:${input.targetAccountId}:${input.rootFileIds.join(",")}:${input.recursive}`); return current; }, createJob: unsupported, updateDraftJobAccounts: unsupported,
  deleteDraftJob: unsupported, getAccountReferences: unsupported, validateRoot: unsupported,
  reorderQueuedJob: async (id, position) => {
    if (queueGate) await queueGate;
    if (failQueue) throw new Error("Queue update failed");
    const jobs = queueJobs ?? [];
    const selected = jobs.find((job) => job.id === id);
    if (!selected) throw new Error("Missing queue job");
    const queued = jobs.filter((job) => job.status === "QUEUED" && job.id !== id)
      .sort((left, right) => (left.queuePosition ?? 0) - (right.queuePosition ?? 0));
    queued.splice(position - 1, 0, selected);
    queued.forEach((job, index) => { job.queuePosition = index + 1; });
    commands.push(`reorder:${id}:${position}`); emit(); return selected;
  },
  removeQueuedJob: async (id) => {
    if (failQueue) throw new Error("Queue update failed");
    const selected = queueJobs?.find((job) => job.id === id);
    if (!selected) throw new Error("Missing queue job");
    selected.status = "PAUSED"; selected.queuePosition = null;
    queueJobs?.filter((job) => job.status === "QUEUED").forEach((job, index) => { job.queuePosition = index + 1; });
    commands.push(`remove:${id}`); emit(); return selected;
  },
  exportFinalReport: async (id, destination) => {
    if (failExport) throw new Error("Destination is not writable");
    commands.push(`export:${id}:${destination}`);
    return { path: destination, status: "COMPLETED_WITH_ERRORS", counts: {total: 6, verified: 2, failed: 1, cancelled: 1, skipped: 1, unfinished: 1} };
  },
  addRoot: unsupported, removeRoot: unsupported, exportDryRun: unsupported,
};
function Fixture() {
  const [generation, setGeneration] = useState(0);
  Object.assign(window, { progressQa: {
    commands,
    holdQueue() { queueGate = new Promise<void>((resolve) => { releaseQueue = resolve; }); },
    releaseQueue() { releaseQueue?.(); queueGate = null; releaseQueue = null; },
    failQueue(value: boolean) { failQueue = value; },
    failExport(value: boolean) { failExport = value; },
    queueScenario() {
      queueJobs = [1, 2, 3].map((position) => ({ ...current, id: `queue-${position}`, status: "QUEUED", queuePosition: position * 2,
        roots: [{ id: `root-${position}`, jobId: `queue-${position}`, rootFileId: `file-${position}`, rootName: `Queued fixture ${position}`, validationStatus: "VALID", createdAt: "2026-09-07T00:00:00Z" }] }));
      queueJobs.push({ ...current, id: "finished", status: "COMPLETED_WITH_ERRORS" });
      emit();
    },
    get jobRequests() { return jobRequests; },
    get jobHeld() { return releaseJob !== null; },
    failJob(failure: boolean) { failJob = failure; },
    failStartTransfer(failure: boolean) { failStartTransfer = failure; },
    setStatus(status: JobDto["status"], phase: JobDto["phase"] = "bulk") { current = { ...current, status, phase };
      if (status === "CANARY_REVIEW") {
        current.progress = { completed: 1, failed: 1, skipped: 0, total: 3, currentPath: null };
        items.push({ id: "remaining", jobId: current.id, fileId: "remaining", name: "Remaining.pdf", mimeType: "application/pdf", depth: 0, originalParentIds: [], state: "ELIGIBLE", quotaBytesUsed: null });
      }
      emit(); },
    complete() { items = items.map((item, index) => ({ ...item, state: index === 0 ? "VERIFIED" : "PERMANENT_FAILED" })); current = { ...current, status: "COMPLETED_WITH_ERRORS", progress: { completed: 1, failed: 1, skipped: 0, total: 2, currentPath: null } }; emit(); },
    registryFailure(failure: boolean) { failAccounts = failure; setGeneration((value) => value + 1); },
    disconnect() { source.authStatus = "DISCONNECTED"; emit(); },
    holdJob() { delayJob = true; },
    releaseJob() { releaseJob?.(); releaseJob = null; },
  } });
  return <App key={generation} backend={backend} />;
}
const root = document.getElementById("root");
if (root) createRoot(root).render(<Fixture />);
