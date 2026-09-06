import { describe, expect, it } from "vitest";

import type { AccountDto, JobDto } from "../ipc/types.ts";
import { JOB_LABEL_SEPARATOR, JOB_PAIR_ARROW } from "./copy.ts";
import { jobPairLabel, jobSnapshotLabel } from "./pairLabel.ts";

const job: JobDto = {
  id: "10",
  sourceAccountId: "1",
  targetAccountId: "2",
  sourceSnapshot: {
    accountId: "1",
    email: "ada@gmail.com",
    displayName: "Ada",
    permissionId: "perm-1",
  },
  targetSnapshot: {
    accountId: "2",
    email: "grace@gmail.com",
    displayName: "Grace",
    permissionId: "perm-2",
  },
  status: "PAUSED",
  queuePosition: null,
  canarySize: 5,
  createdAt: "2026-09-06T00:00:00Z",
  startedAt: "2026-09-06T01:00:00Z",
  completedAt: null,
  lastError: null,
  roots: [],
};

const accounts: AccountDto[] = [
  {
    id: "1",
    googlePermissionId: "perm-1",
    email: "ada@gmail.com",
    displayName: "Ada Lovelace",
    label: "Personal A",
    authStatus: "CONNECTED",
    connectedAt: "2026-09-05T00:00:00Z",
    lastAuthenticatedAt: "2026-09-05T00:00:00Z",
    updatedAt: "2026-09-05T00:00:00Z",
    removedAt: null,
  },
];

describe("job pair label", () => {
  it("renders snapshot identity even when the registry selection is empty", () => {
    expect(jobPairLabel(job, [])).toBe(
      `Ada${JOB_LABEL_SEPARATOR}ada@gmail.com${JOB_PAIR_ARROW}Grace${JOB_LABEL_SEPARATOR}grace@gmail.com`,
    );
  });

  it("prefers the live local label but still uses snapshot emails", () => {
    expect(jobPairLabel(job, accounts)).toBe(
      `Personal A${JOB_LABEL_SEPARATOR}ada@gmail.com${JOB_PAIR_ARROW}Grace${JOB_LABEL_SEPARATOR}grace@gmail.com`,
    );
    expect(jobSnapshotLabel(job.sourceSnapshot, accounts)).toBe(
      `Personal A${JOB_LABEL_SEPARATOR}ada@gmail.com`,
    );
  });
});
