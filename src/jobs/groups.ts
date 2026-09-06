import type { JobDto, JobStatus } from "../ipc/types.ts";
import { JOB_LIST_GROUP_TITLES as GROUP_TITLES } from "./copy.ts";

export const JOB_LIST_GROUPS = ["active", "queued", "draft", "completed"] as const;

export type JobListGroup = (typeof JOB_LIST_GROUPS)[number];

export const JOB_LIST_GROUP_TITLES: Record<JobListGroup, string> = GROUP_TITLES;

export function jobListGroup(status: JobStatus): JobListGroup {
  switch (status) {
    case "DRAFT":
      return "draft";
    case "QUEUED":
      return "queued";
    case "CANCELLED":
    case "COMPLETED":
    case "COMPLETED_WITH_ERRORS":
    case "FAILED":
      return "completed";
    case "SCANNING":
    case "READY_FOR_REVIEW":
    case "RUNNING_CANARY":
    case "CANARY_REVIEW":
    case "RUNNING":
    case "PAUSING":
    case "PAUSED":
    case "CANCELLING":
    case "AUTH_REQUIRED":
    case "SOURCE_RATE_LIMITED":
    case "WAITING_FOR_QUOTA":
      return "active";
  }
}

export function groupJobs(jobs: readonly JobDto[]): Record<JobListGroup, JobDto[]> {
  const grouped: Record<JobListGroup, JobDto[]> = {
    active: [],
    queued: [],
    draft: [],
    completed: [],
  };
  for (const job of jobs) {
    grouped[jobListGroup(job.status)].push(job);
  }
  return grouped;
}

export function jobReferencesAccount(job: JobDto, accountId: string): boolean {
  return job.sourceAccountId === accountId || job.targetAccountId === accountId;
}

export function filterJobsByAccount(jobs: readonly JobDto[], accountId: string | null): JobDto[] {
  if (accountId === null || accountId.length === 0) {
    return [...jobs];
  }
  return jobs.filter((job) => jobReferencesAccount(job, accountId));
}

export function jobCountByAccount(jobs: readonly JobDto[]): Record<string, number> {
  const counts: Record<string, number> = {};
  for (const job of jobs) {
    counts[job.sourceAccountId] = (counts[job.sourceAccountId] ?? 0) + 1;
    if (job.targetAccountId !== job.sourceAccountId) {
      counts[job.targetAccountId] = (counts[job.targetAccountId] ?? 0) + 1;
    }
  }
  return counts;
}
