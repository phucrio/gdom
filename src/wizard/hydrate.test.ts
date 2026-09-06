import { describe, expect, it } from "vitest";

import type { JobDto } from "../ipc/types.ts";
import { hydrateWizardFromJob } from "./hydrate.ts";

const paused: JobDto = {
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
  status: "PAUSED",
  queuePosition: null,
  canarySize: 5,
  createdAt: "2026-09-06T00:00:00Z",
  startedAt: "2026-09-06T01:00:00Z",
  completedAt: null,
  lastError: "auth",
  roots: [
    {
      id: "root-1",
      jobId: "99",
      rootFileId: "folder-abc",
      rootName: "Archive",
      validationStatus: "VALIDATED",
      createdAt: "2026-09-06T00:00:00Z",
    },
  ],
};

describe("hydrate wizard from job", () => {
  it("loads persisted source and target IDs, not a registry selection", () => {
    const hydrated = hydrateWizardFromJob(paused);
    expect(hydrated.sourceAccountId).toBe("10");
    expect(hydrated.targetAccountId).toBe("20");
    expect(hydrated.roots).toEqual([{ folderId: "folder-abc", input: "Archive" }]);
    expect(hydrated.step).toBe("scan-preflight");
  });

  it("does not treat scan inventory progress as a transfer pause", () => {
    const hydrated = hydrateWizardFromJob({
      ...paused,
      progress: { completed: 0, total: 6, currentPath: null },
      phase: "scan",
    });
    expect(hydrated.step).toBe("scan-preflight");
  });

  it("opens a transfer-paused canary on canary review", () => {
    const hydrated = hydrateWizardFromJob({ ...paused, phase: "canary" });
    expect(hydrated.step).toBe("canary-review");
  });

  it("opens queued bulk on live migration so confirm can start", () => {
    const hydrated = hydrateWizardFromJob({ ...paused, status: "QUEUED", phase: "bulk" });
    expect(hydrated.step).toBe("live-migration");
  });

  it("opens a halted mutation job on live migration so resume is explicit in the wizard", () => {
    const hydrated = hydrateWizardFromJob({ ...paused, status: "AUTH_REQUIRED", phase: "bulk" });
    expect(hydrated.sourceAccountId).toBe(paused.sourceAccountId);
    expect(hydrated.targetAccountId).toBe(paused.targetAccountId);
    expect(hydrated.step).toBe("live-migration");
  });
});
