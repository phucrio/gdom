import type { AccountDto, JobDto, ScanSummary } from "../ipc/types.ts";
import { jobSnapshotLabel } from "../jobs/pairLabel.ts";
import { DRY_RUN_CATEGORY_LABELS, DRY_RUN_NO_ROOTS } from "./copy.ts";
import { categoryFilter, type JobItemFilter } from "./filters.ts";

export type PreflightCategoryId = keyof typeof DRY_RUN_CATEGORY_LABELS;

export type PreflightCategory = {
  id: PreflightCategoryId;
  label: string;
  count: number;
  filter: JobItemFilter;
};

export type DryRunPair = {
  sourceLabel: string;
  sourceEmail: string;
  targetLabel: string;
  targetEmail: string;
};

export function emptyScanSummary(): ScanSummary {
  return {
    files: 0,
    folders: 0,
    skipped: 0,
    ineligible: 0,
    quotaWarning: false,
    totalItems: 0,
    eligibleItems: 0,
    alreadyOwnedByTarget: 0,
    notOwnedBySource: 0,
    sharedDrive: 0,
    shortcuts: 0,
    trashed: 0,
    otherIneligible: 0,
    estimatedQuotaBytes: 0,
    targetUsageBytes: 0,
    targetLimitBytes: null,
    targetRemainingBytes: null,
  };
}

export function dryRunPair(job: JobDto | null, accounts: readonly AccountDto[]): DryRunPair | null {
  if (job === null) {
    return null;
  }
  return {
    sourceLabel: jobSnapshotLabel(job.sourceSnapshot, accounts),
    sourceEmail: job.sourceSnapshot.email,
    targetLabel: jobSnapshotLabel(job.targetSnapshot, accounts),
    targetEmail: job.targetSnapshot.email,
  };
}

export function dryRunRootNames(job: JobDto | null): string[] {
  if (job === null) {
    return [];
  }
  return job.roots.map((root) => (root.rootName.trim().length > 0 ? root.rootName : root.rootFileId));
}

export function dryRunRootsLabel(job: JobDto | null): string {
  const names = dryRunRootNames(job);
  return names.length === 0 ? DRY_RUN_NO_ROOTS : names.join(", ");
}

export function preflightCategories(scan: ScanSummary): PreflightCategory[] {
  const rows: Array<[PreflightCategoryId, number]> = [
    ["eligibleFiles", scan.files],
    ["eligibleFolders", scan.folders],
    ["alreadyOwned", scan.alreadyOwnedByTarget],
    ["notOwnedBySource", scan.notOwnedBySource],
    ["sharedDrive", scan.sharedDrive],
    ["shortcuts", scan.shortcuts],
    ["trashed", scan.trashed],
    ["otherIneligible", scan.otherIneligible],
  ];
  return rows.map(([id, count]) => ({
    id,
    label: DRY_RUN_CATEGORY_LABELS[id],
    count,
    filter: categoryFilter(id),
  }));
}
