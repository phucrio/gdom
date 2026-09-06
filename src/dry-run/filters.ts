import {
  JOB_ITEM_FILTERS,
  type JobItemFilter,
} from "../ipc/types.ts";
import { DRY_RUN_FILTER_LABELS } from "./copy.ts";

export { JOB_ITEM_FILTERS, type JobItemFilter };

export function isJobItemFilter(value: string): value is JobItemFilter {
  return (JOB_ITEM_FILTERS as readonly string[]).includes(value);
}

export function backendItemFilter(filter: JobItemFilter): string | null {
  return filter === "all" ? null : filter;
}

export function itemFilterLabel(filter: JobItemFilter): string {
  return DRY_RUN_FILTER_LABELS[filter];
}

export function itemPageCount(total: number, pageSize: number): number {
  if (total <= 0 || pageSize <= 0) {
    return 1;
  }
  return Math.max(1, Math.ceil(total / pageSize));
}

export function clampItemPage(page: number, pageCount: number): number {
  if (page < 1) {
    return 1;
  }
  if (page > pageCount) {
    return pageCount;
  }
  return page;
}

export function categoryFilter(categoryId: string): JobItemFilter {
  switch (categoryId) {
    case "eligibleFiles":
    case "eligibleFolders":
      return "eligible";
    case "alreadyOwned":
    case "shortcuts":
      return "skipped";
    case "notOwnedBySource":
    case "sharedDrive":
    case "trashed":
    case "otherIneligible":
      return "ineligible";
    default:
      return "all";
  }
}
