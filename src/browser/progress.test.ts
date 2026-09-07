import { describe, expect, it } from "vitest";
import { progressCounts } from "./progress.ts";
import type { JobDto } from "../ipc/types.ts";

const job: JobDto = {
  id: "job", sourceAccountId: "source", targetAccountId: "target",
  sourceSnapshot: { accountId: "source", email: "source@gmail.com", displayName: "Source", permissionId: "source" },
  targetSnapshot: { accountId: "target", email: "target@gmail.com", displayName: "Target", permissionId: "target" },
  status: "COMPLETED_WITH_ERRORS", queuePosition: null, canarySize: 5,
  createdAt: "2026-09-07", startedAt: null, completedAt: null, lastError: null, roots: [],
};
describe("persisted progress counts", () => {
  it("counts failed and skipped items as processed without counting error history", () => {
    const result = progressCounts({ ...job,
      progress: { completed: 5, failed: 1, skipped: 2, total: 8, currentPath: null },
      errors: [{ itemId: "old", message: "old retry failure", at: "earlier" }],
    });
    expect(result).toEqual({ succeeded: 5, failed: 1, skipped: 2, total: 8, processed: 8, percent: 100 });
  });
});
