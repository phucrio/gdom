import { describe, expect, it } from "vitest";

import type { JobDto, JobStatus } from "../ipc/types.ts";
import {
  filterJobsByAccount,
  groupJobs,
  jobCountByAccount,
  jobListGroup,
} from "./groups.ts";

function job(status: JobStatus, source = "1", target = "2"): JobDto {
  return {
    id: `${status}-${source}-${target}`,
    sourceAccountId: source,
    targetAccountId: target,
    sourceSnapshot: {
      accountId: source,
      email: `${source}@gmail.com`,
      displayName: "Source",
      permissionId: "perm-s",
    },
    targetSnapshot: {
      accountId: target,
      email: `${target}@gmail.com`,
      displayName: "Target",
      permissionId: "perm-t",
    },
    status,
    queuePosition: status === "QUEUED" ? 1 : null,
    canarySize: 5,
    createdAt: "2026-09-06T00:00:00Z",
    startedAt: null,
    completedAt: null,
    lastError: null,
    roots: [],
  };
}

describe("job list groups", () => {
  it("puts draft, queued, active, and completed jobs in separate lists", () => {
    expect(jobListGroup("DRAFT")).toBe("draft");
    expect(jobListGroup("QUEUED")).toBe("queued");
    expect(jobListGroup("PAUSED")).toBe("active");
    expect(jobListGroup("AUTH_REQUIRED")).toBe("active");
    expect(jobListGroup("SOURCE_RATE_LIMITED")).toBe("active");
    expect(jobListGroup("COMPLETED")).toBe("completed");

    const grouped = groupJobs([
      job("DRAFT"),
      job("QUEUED"),
      job("PAUSED"),
      job("COMPLETED_WITH_ERRORS"),
    ]);
    expect(grouped.draft).toHaveLength(1);
    expect(grouped.queued).toHaveLength(1);
    expect(grouped.active).toHaveLength(1);
    expect(grouped.completed).toHaveLength(1);
  });

  it("filters by source or target account and counts referencing jobs", () => {
    const jobs = [job("DRAFT", "1", "2"), job("QUEUED", "3", "1"), job("PAUSED", "4", "5")];
    expect(filterJobsByAccount(jobs, "1").map((item) => item.id)).toEqual([
      "DRAFT-1-2",
      "QUEUED-3-1",
    ]);
    expect(jobCountByAccount(jobs)["1"]).toBe(2);
    expect(jobCountByAccount(jobs)["5"]).toBe(1);
  });
});
