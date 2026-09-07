import type { JobDto } from "../ipc/types.ts";

export function progressCounts(job: JobDto) {
  const succeeded = job.progress?.completed ?? 0;
  const failed = job.progress?.failed ?? 0;
  const skipped = job.progress?.skipped ?? job.scan?.skipped ?? 0;
  const total = job.progress?.total ?? job.scan?.totalItems ?? 0;
  const processed = succeeded + failed + skipped;
  return { succeeded, failed, skipped, total, processed,
    percent: total > 0 ? Math.min(100, Math.floor(processed / total * 100)) : 0 };
}

export function canResumeJob(job: JobDto): boolean {
  return ["PAUSED", "QUEUED", "AUTH_REQUIRED", "SOURCE_RATE_LIMITED", "WAITING_FOR_QUOTA"].includes(job.status);
}
