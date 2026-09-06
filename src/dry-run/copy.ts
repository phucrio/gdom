export const DRY_RUN_DASHBOARD_LABEL = "Dry-run preflight dashboard";
export const DRY_RUN_ITEMS_LABEL = "Discovered items";
export const DRY_RUN_EXPORT_LABEL = "Export dry-run report";
export const DRY_RUN_SOURCE_LABEL = "Source";
export const DRY_RUN_TARGET_LABEL = "Target";
export const DRY_RUN_ROOTS_LABEL = "Root folders";
export const DRY_RUN_NO_ROOTS = "No root folders.";
export const DRY_RUN_FILTER_LABEL = "Item filter";
export const DRY_RUN_EMPTY_ITEMS = "No items match this filter.";
export const DRY_RUN_ITEMS_LOADING = "Loading discovered items…";
export const DRY_RUN_ITEMS_UNAVAILABLE =
  "Item listing is not registered in this build. The dashboard still uses scan totals.";
export const DRY_RUN_EXPORT_UNAVAILABLE =
  "Dry-run export is not registered in this build.";
export const DRY_RUN_EXPORT_PATH_LABEL = "Local destination path";
export const DRY_RUN_EXPORT_PATH_HINT =
  "Enter a full local path. Use .txt for the preflight summary or .csv for item metadata. File content and tokens are never written.";
export const DRY_RUN_EXPORT_PATH_REQUIRED = "Enter a local destination path before exporting.";
export const DRY_RUN_EXPORT_BUTTON = "Export dry-run";
export const DRY_RUN_EXPORT_DISABLED = "Finish the scan before exporting the dry-run.";
export const DRY_RUN_PREV_PAGE = "Previous page";
export const DRY_RUN_NEXT_PAGE = "Next page";
export const DRY_RUN_QUOTA_UNLIMITED = "Unlimited";
export const DRY_RUN_QUOTA_UNKNOWN = "Unknown";

export const DRY_RUN_CATEGORY_LABELS = {
  eligibleFiles: "Eligible files",
  eligibleFolders: "Eligible folders",
  alreadyOwned: "Already target-owned",
  notOwnedBySource: "Not owned by source",
  sharedDrive: "Shared Drive",
  shortcuts: "Shortcuts",
  trashed: "Trashed or missing",
  otherIneligible: "Other ineligible",
} as const;

export const DRY_RUN_FILTER_LABELS = {
  all: "All",
  eligible: "Eligible",
  skipped: "Skipped",
  ineligible: "Ineligible",
} as const;

export const DRY_RUN_ITEM_COLUMNS = {
  name: "Name",
  kind: "Kind",
  state: "State",
  depth: "Depth",
  fileId: "File ID",
  quota: "Quota bytes",
} as const;

export const DRY_RUN_KIND_FOLDER = "Folder";
export const DRY_RUN_KIND_SHORTCUT = "Shortcut";
export const DRY_RUN_KIND_FILE = "File";
export const DRY_RUN_QUOTA_EMPTY = "—";

export const DRY_RUN_ITEM_STATE_LABELS = {
  DISCOVERED: "Discovered",
  ELIGIBLE: "Eligible",
  SKIPPED_ALREADY_OWNED_BY_TARGET: "Already target-owned",
  SKIPPED_NOT_OWNED_BY_SOURCE: "Not owned by source",
  SKIPPED_SHARED_DRIVE: "Shared Drive",
  SKIPPED_SHORTCUT_TARGET: "Shortcut",
  SKIPPED_TRASHED: "Trashed or missing",
  SKIPPED_INELIGIBLE: "Ineligible",
  PENDING_OWNER_REQUIRED: "Pending owner required",
  PENDING_OWNER_CREATED: "Pending owner created",
  ACCEPT_REQUIRED: "Accept required",
  ACCEPTING: "Accepting",
  TRANSFERRED: "Transferred",
  VERIFYING: "Verifying",
  VERIFIED: "Verified",
  RETRYABLE_FAILED: "Retryable failure",
  PERMANENT_FAILED: "Permanent failure",
  CANCELLED: "Cancelled",
} as const;

export const DRY_RUN_QUOTA_ESTIMATED_LABEL = "Estimated quota needed";
export const DRY_RUN_QUOTA_REMAINING_LABEL = "Target remaining";
export const DRY_RUN_QUOTA_USAGE_LABEL = "Target usage";
export const DRY_RUN_QUOTA_LIMIT_LABEL = "Target limit";

export const DRY_RUN_BLOCKING_PREFIX = "Blocking: ";
export const DRY_RUN_NOTICE_PREFIX = "Warning: ";

export const DRY_RUN_QUOTA_BLOCKING =
  "Estimated eligible bytes exceed remaining storage on the target account. Canary and bulk can pause until quota is freed.";
export const DRY_RUN_QUOTA_UNLIMITED_NOTICE =
  "Target remaining storage is unlimited or not reported. Review estimated bytes before transferring.";
export const DRY_RUN_NO_ELIGIBLE =
  "The scan found no eligible items. Canary cannot transfer ownership until eligible files or folders exist.";
export const DRY_RUN_SKIPPED_NOTICE =
  "Skipped and ineligible items are listed for review and will not be transferred.";

export function dryRunExportedAnnouncement(path: string, eligibleItems: number): string {
  return `Dry-run exported to ${path}. ${eligibleItems} eligible items. Metadata only.`;
}

export function dryRunPageStatus(page: number, pageCount: number, total: number): string {
  if (total === 0) {
    return "0 items";
  }
  return `Page ${page} of ${pageCount} · ${total} items`;
}

export function defaultDryRunFileName(jobId: string, extension: "txt" | "csv"): string {
  const safe = jobId.replace(/[^A-Za-z0-9_-]/g, "");
  const id = safe.length > 0 ? safe : "job";
  return `gdom-dry-run-${id}.${extension}`;
}
