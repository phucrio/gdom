import type { JobStatus } from "../ipc/types.ts";
import { isDraftJob } from "./status.ts";

export type ListResumeKind = "transfer" | null;

/** Halted mutation jobs can resume from the list. PAUSED stays in the wizard so scan vs transfer is not guessed. */
export function listResumeKind(status: JobStatus): ListResumeKind {
  switch (status) {
    case "AUTH_REQUIRED":
    case "SOURCE_RATE_LIMITED":
    case "WAITING_FOR_QUOTA":
      return "transfer";
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
      return null;
  }
}

export function canDeleteDraft(status: JobStatus): boolean {
  return isDraftJob(status);
}

export function queuedJobsDoNotAutoStart(status: JobStatus): boolean {
  return status === "QUEUED";
}
