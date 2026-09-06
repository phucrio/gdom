import type { JobDto } from "../ipc/types.ts";
import { listResumeKind } from "./actions.ts";
import { openedJobAnnouncement as formatOpenedJobAnnouncement } from "./copy.ts";
import { jobPairLabel } from "./pairLabel.ts";

export type JobCatalogPort = {
  getJob(jobId: string): Promise<JobDto>;
  resumeMigration(jobId: string): Promise<JobDto>;
};

/** Loads the persisted job. Resume never uses the account currently selected in the registry. */
export async function openPersistedJob(backend: JobCatalogPort, jobId: string): Promise<JobDto> {
  return backend.getJob(jobId);
}

/**
 * Explicit resume only. Opening a job must not call this.
 * Queued and paused-scan jobs are opened, not auto-started.
 */
export async function resumePersistedJob(backend: JobCatalogPort, job: JobDto): Promise<JobDto> {
  if (listResumeKind(job.status) !== "transfer") {
    return openPersistedJob(backend, job.id);
  }
  await backend.resumeMigration(job.id);
  return openPersistedJob(backend, job.id);
}

export function openedJobAnnouncement(job: JobDto): string {
  return formatOpenedJobAnnouncement(jobPairLabel(job));
}
