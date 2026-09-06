import type { ScanSummary } from "../ipc/types.ts";
import {
  DRY_RUN_NO_ELIGIBLE,
  DRY_RUN_QUOTA_BLOCKING,
  DRY_RUN_QUOTA_UNLIMITED,
  DRY_RUN_QUOTA_UNLIMITED_NOTICE,
  DRY_RUN_QUOTA_UNKNOWN,
  DRY_RUN_SKIPPED_NOTICE,
} from "./copy.ts";

export type DryRunWarningKind = "blocking" | "notice";

export type DryRunWarning = {
  kind: DryRunWarningKind;
  message: string;
};

const BYTE_UNITS = ["B", "KB", "MB", "GB", "TB"] as const;

export function formatBytes(bytes: number | null | undefined): string {
  if (bytes === null || bytes === undefined) {
    return DRY_RUN_QUOTA_UNLIMITED;
  }
  if (!Number.isFinite(bytes) || bytes < 0) {
    return DRY_RUN_QUOTA_UNKNOWN;
  }
  if (bytes < 1024) {
    return `${Math.round(bytes)} B`;
  }
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < BYTE_UNITS.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const digits = value >= 10 || unit === 0 ? 0 : 1;
  return `${value.toFixed(digits)} ${BYTE_UNITS[unit]}`;
}

export function formatQuotaRemaining(bytes: number | null | undefined): string {
  if (bytes === null || bytes === undefined) {
    return DRY_RUN_QUOTA_UNLIMITED;
  }
  return formatBytes(bytes);
}

export function dryRunWarnings(scan: ScanSummary, scanComplete: boolean): DryRunWarning[] {
  const warnings: DryRunWarning[] = [];
  if (scan.quotaWarning) {
    warnings.push({ kind: "blocking", message: DRY_RUN_QUOTA_BLOCKING });
  } else if (scan.estimatedQuotaBytes > 0 && scan.targetLimitBytes === null) {
    warnings.push({ kind: "notice", message: DRY_RUN_QUOTA_UNLIMITED_NOTICE });
  }
  if (scanComplete && scan.eligibleItems === 0 && scan.files + scan.folders === 0) {
    warnings.push({ kind: "blocking", message: DRY_RUN_NO_ELIGIBLE });
  }
  if (scan.skipped > 0 || scan.ineligible > 0) {
    warnings.push({ kind: "notice", message: DRY_RUN_SKIPPED_NOTICE });
  }
  return warnings;
}
