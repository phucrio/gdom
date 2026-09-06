export const JOBS_EYEBROW = "Migration jobs";
export const JOBS_TITLE = "Jobs";
export const JOBS_TITLE_ID = "jobs-title";
export const JOB_ACCOUNT_FILTER_ID = "job-account-filter";
export const JOB_ACCOUNT_FILTER_LABEL = "Filter by source or target account";
export const JOB_ACCOUNT_FILTER_ALL = "All accounts";
export const JOBS_LOADING = "Loading jobs…";
export const JOBS_EMPTY = "No migration jobs yet. Start a new job to plan a transfer.";
export const JOBS_EMPTY_FILTERED = "No jobs reference that account.";
export const JOBS_LOAD_FAILED = "Could not load migration jobs from the local backend.";
export const JOB_OPEN_LABEL = "Open";
export const JOB_RESUME_LABEL = "Resume";
export const JOB_DELETE_DRAFT_LABEL = "Delete draft";
export const JOB_DELETE_DRAFT_TITLE = "Delete draft job";
export const JOB_DELETE_CANCEL_LABEL = "Cancel";
export const JOB_DELETE_FAILED = "Could not delete the draft job.";
export const JOB_PAIR_ARROW = " -> ";
export const JOB_LABEL_SEPARATOR = " / ";

export const JOB_LIST_GROUP_TITLES = {
  active: "Running, paused, and rate-limited",
  queued: "Queued",
  draft: "Draft",
  completed: "Completed",
} as const;

export function jobsShownLabel(count: number): string {
  return `${count} shown`;
}

export function queuedJobsNotice(count: number): string {
  const verb = count === 1 ? "job is" : "jobs are";
  return `${count} queued ${verb} waiting. Queued jobs do not start until you open them and confirm.`;
}

export function queuePositionLabel(position: number): string {
  return `Queue position ${position}`;
}

export function draftDeletedAnnouncement(pairLabel: string): string {
  return `Draft job ${pairLabel} deleted.`;
}

export function deleteDraftConfirmMessage(pairLabel: string): string {
  return `Delete the draft ${pairLabel}? Scanned jobs cannot be deleted this way, and source/target cannot be swapped after a scan starts.`;
}

export function openedJobAnnouncement(pairLabel: string): string {
  return `Opened job ${pairLabel}.`;
}

export function accountOptionLabel(label: string, email: string): string {
  return `${label} (${email})`;
}

export function jobGroupHeadingId(group: string): string {
  return `job-group-${group}`;
}
