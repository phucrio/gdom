import type { JobItemDto } from "../ipc/types.ts";
import {
  DRY_RUN_ITEM_STATE_LABELS,
  DRY_RUN_KIND_FILE,
  DRY_RUN_KIND_FOLDER,
  DRY_RUN_KIND_SHORTCUT,
  DRY_RUN_QUOTA_EMPTY,
} from "./copy.ts";

export const GOOGLE_FOLDER_MIME = "application/vnd.google-apps.folder";
export const GOOGLE_SHORTCUT_MIME = "application/vnd.google-apps.shortcut";

export function itemKindLabel(mimeType: string): string {
  if (mimeType === GOOGLE_FOLDER_MIME) {
    return DRY_RUN_KIND_FOLDER;
  }
  if (mimeType === GOOGLE_SHORTCUT_MIME) {
    return DRY_RUN_KIND_SHORTCUT;
  }
  return DRY_RUN_KIND_FILE;
}

function isKnownItemState(state: string): state is keyof typeof DRY_RUN_ITEM_STATE_LABELS {
  return Object.prototype.hasOwnProperty.call(DRY_RUN_ITEM_STATE_LABELS, state);
}

export function itemStateLabel(state: string): string {
  if (isKnownItemState(state)) {
    return DRY_RUN_ITEM_STATE_LABELS[state];
  }
  return state.replace(/_/g, " ").toLowerCase();
}

export function itemQuotaLabel(item: JobItemDto): string {
  if (item.quotaBytesUsed === null) {
    return DRY_RUN_QUOTA_EMPTY;
  }
  return String(item.quotaBytesUsed);
}

export function emptyItemsPage(): {
  items: JobItemDto[];
  page: number;
  pageSize: number;
  total: number;
} {
  return { items: [], page: 1, pageSize: 50, total: 0 };
}
