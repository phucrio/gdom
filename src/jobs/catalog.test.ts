import { describe, expect, it } from "vitest";

import type { JobDto } from "../ipc/types.ts";
import { openPersistedJob, resumePersistedJob } from "./catalog.ts";

function job(status: JobDto["status"]): JobDto {
  return {
    id: "99",
    sourceAccountId: "10",
    targetAccountId: "20",
    sourceSnapshot: {
      accountId: "10",
      email: "source@gmail.com",
      displayName: "Source",
      permissionId: "perm-s",
    },
    targetSnapshot: {
      accountId: "20",
      email: "target@gmail.com",
      displayName: "Target",
      permissionId: "perm-t",
    },
    status,
    queuePosition: null,
    canarySize: 5,
    createdAt: "2026-09-06T00:00:00Z",
    startedAt: null,
    completedAt: null,
    lastError: null,
    roots: [],
  };
}

describe("persisted job catalog", () => {
  it("opens by job id and does not resume", async () => {
    const calls: string[] = [];
    const opened = await openPersistedJob(
      {
        getJob: async (id) => {
          calls.push(`get:${id}`);
          return job("PAUSED");
        },
        resumeMigration: async () => {
          calls.push("resume");
          return job("RUNNING");
        },
      },
      "99",
    );
    expect(opened.sourceAccountId).toBe("10");
    expect(opened.targetAccountId).toBe("20");
    expect(calls).toEqual(["get:99"]);
  });

  it("resumes halted mutation jobs from the persisted id, not a registry selection", async () => {
    const calls: string[] = [];
    const resumed = await resumePersistedJob(
      {
        getJob: async (id) => {
          calls.push(`get:${id}`);
          return { ...job("RUNNING"), id };
        },
        resumeMigration: async (id) => {
          calls.push(`resume:${id}`);
          return job("RUNNING");
        },
      },
      job("AUTH_REQUIRED"),
    );
    expect(resumed.sourceAccountId).toBe("10");
    expect(calls).toEqual(["resume:99", "get:99"]);
  });

  it("does not start queued jobs", async () => {
    const calls: string[] = [];
    await resumePersistedJob(
      {
        getJob: async (id) => {
          calls.push(`get:${id}`);
          return job("QUEUED");
        },
        resumeMigration: async () => {
          calls.push("resume");
          return job("RUNNING");
        },
      },
      job("QUEUED"),
    );
    expect(calls).toEqual(["get:99"]);
  });
});
