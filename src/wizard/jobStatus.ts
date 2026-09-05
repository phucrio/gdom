import type { JobStatus } from "../ipc/types.ts";

export function isDraftJob(status: JobStatus): boolean {
  return status === "DRAFT";
}

export function scanAllowsCanary(status: JobStatus): boolean {
  return status !== "DRAFT" && status !== "SCANNING";
}

export function canaryAllowsBulk(status: JobStatus): boolean {
  switch (status) {
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
    case "SOURCE_RATE_LIMITED":
    case "WAITING_FOR_QUOTA":
      return true;
    case "DRAFT":
    case "SCANNING":
    case "READY_FOR_REVIEW":
    case "RUNNING_CANARY":
    case "AUTH_REQUIRED":
      return false;
  }
}
