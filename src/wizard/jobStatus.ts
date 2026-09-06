import type { JobStatus } from "../ipc/types.ts";

export function isDraftJob(status: JobStatus): boolean {
  return status === "DRAFT";
}

export function isScanRunning(status: JobStatus): boolean {
  return status === "SCANNING";
}

export function isCanaryRunning(status: JobStatus): boolean {
  return status === "RUNNING_CANARY";
}

export function isBulkRunning(status: JobStatus): boolean {
  return status === "RUNNING" || status === "PAUSING" || status === "CANCELLING";
}

export function isTransferRunning(status: JobStatus): boolean {
  return isCanaryRunning(status) || isBulkRunning(status);
}

export function isPaused(status: JobStatus): boolean {
  return status === "PAUSED";
}

export function isHaltStatus(status: JobStatus): boolean {
  switch (status) {
    case "AUTH_REQUIRED":
    case "SOURCE_RATE_LIMITED":
    case "WAITING_FOR_QUOTA":
      return true;
    case "DRAFT":
    case "SCANNING":
    case "READY_FOR_REVIEW":
    case "RUNNING_CANARY":
    case "CANARY_REVIEW":
    case "QUEUED":
    case "RUNNING":
    case "PAUSING":
    case "PAUSED":
    case "CANCELLING":
    case "CANCELLED":
    case "COMPLETED":
    case "COMPLETED_WITH_ERRORS":
    case "FAILED":
      return false;
  }
}

export function isTerminalJob(status: JobStatus): boolean {
  switch (status) {
    case "CANCELLED":
    case "COMPLETED":
    case "COMPLETED_WITH_ERRORS":
    case "FAILED":
      return true;
    case "DRAFT":
    case "SCANNING":
    case "READY_FOR_REVIEW":
    case "RUNNING_CANARY":
    case "CANARY_REVIEW":
    case "QUEUED":
    case "RUNNING":
    case "PAUSING":
    case "PAUSED":
    case "CANCELLING":
    case "AUTH_REQUIRED":
    case "SOURCE_RATE_LIMITED":
    case "WAITING_FOR_QUOTA":
      return false;
  }
}

/** Scan finished far enough that the dry-run can be reviewed and canary may start. */
export function scanAllowsCanary(status: JobStatus): boolean {
  switch (status) {
    case "READY_FOR_REVIEW":
    case "RUNNING_CANARY":
    case "CANARY_REVIEW":
    case "QUEUED":
    case "RUNNING":
    case "PAUSING":
    case "CANCELLING":
    case "CANCELLED":
    case "COMPLETED":
    case "COMPLETED_WITH_ERRORS":
    case "SOURCE_RATE_LIMITED":
    case "WAITING_FOR_QUOTA":
      return true;
    case "DRAFT":
    case "SCANNING":
    case "PAUSED":
    case "FAILED":
    case "AUTH_REQUIRED":
      return false;
  }
}

/** Canary finished (or a later live state). Bulk must not skip RUNNING_CANARY. */
export function canaryAllowsBulk(status: JobStatus): boolean {
  switch (status) {
    case "CANARY_REVIEW":
    case "RUNNING":
    case "PAUSING":
    case "PAUSED":
    case "CANCELLING":
    case "CANCELLED":
    case "COMPLETED":
    case "COMPLETED_WITH_ERRORS":
    case "FAILED":
    case "SOURCE_RATE_LIMITED":
    case "WAITING_FOR_QUOTA":
      return true;
    case "DRAFT":
    case "SCANNING":
    case "READY_FOR_REVIEW":
    case "RUNNING_CANARY":
    case "QUEUED":
    case "AUTH_REQUIRED":
      return false;
  }
}

export function canStartScan(status: JobStatus | null): boolean {
  return status === null || status === "DRAFT";
}

export function canPauseScan(status: JobStatus | null): boolean {
  return status === "SCANNING";
}

export function canResumeScan(status: JobStatus | null): boolean {
  return status === "PAUSED";
}

export function canStartCanary(status: JobStatus | null): boolean {
  return status === "READY_FOR_REVIEW" || status === "QUEUED";
}

export function canPauseTransfer(status: JobStatus | null): boolean {
  return status === "RUNNING_CANARY" || status === "RUNNING" || status === "PAUSING";
}

export function canResumeTransfer(status: JobStatus | null): boolean {
  switch (status) {
    case "PAUSED":
    case "SOURCE_RATE_LIMITED":
    case "WAITING_FOR_QUOTA":
    case "AUTH_REQUIRED":
      return true;
    default:
      return false;
  }
}

export function canStartBulk(status: JobStatus | null): boolean {
  return status === "CANARY_REVIEW" || status === "QUEUED";
}

export function canCancelTransfer(status: JobStatus | null): boolean {
  if (status === null || isDraftJob(status) || isTerminalJob(status)) {
    return false;
  }
  return status !== "SCANNING";
}

export function jobStatusLabel(status: JobStatus): string {
  switch (status) {
    case "DRAFT":
      return "Draft";
    case "SCANNING":
      return "Scanning";
    case "READY_FOR_REVIEW":
      return "Ready for review";
    case "RUNNING_CANARY":
      return "Canary running";
    case "CANARY_REVIEW":
      return "Canary complete — review before bulk";
    case "QUEUED":
      return "Queued behind another mutation job";
    case "RUNNING":
      return "Live migration running";
    case "PAUSING":
      return "Pausing";
    case "PAUSED":
      return "Paused";
    case "CANCELLING":
      return "Cancelling";
    case "CANCELLED":
      return "Cancelled";
    case "COMPLETED":
      return "Completed";
    case "COMPLETED_WITH_ERRORS":
      return "Completed with errors";
    case "FAILED":
      return "Failed";
    case "AUTH_REQUIRED":
      return "Re-authentication required";
    case "SOURCE_RATE_LIMITED":
      return "Source sharing rate limited — resume after the limit clears";
    case "WAITING_FOR_QUOTA":
      return "Waiting for target storage quota — resume after quota is available";
  }
}

export function haltStatusDetail(status: JobStatus, lastError: string | null): string | null {
  switch (status) {
    case "AUTH_REQUIRED":
      return lastError !== null && lastError.length > 0
        ? `Re-authentication required. ${lastError}`
        : "An account needs to be re-authenticated in the Account Registry before this job can resume.";
    case "SOURCE_RATE_LIMITED":
      return lastError !== null && lastError.length > 0
        ? `Sharing rate limit reached. ${lastError} Resume after the limit clears. This is not retried automatically.`
        : "Google Drive sharing rate limit reached. Resume after the limit clears. This is not retried automatically.";
    case "WAITING_FOR_QUOTA":
      return lastError !== null && lastError.length > 0
        ? `Target storage quota exceeded. ${lastError} Free space, then resume.`
        : "Target storage quota exceeded. Free space on the target account, then resume.";
    default:
      return null;
  }
}
