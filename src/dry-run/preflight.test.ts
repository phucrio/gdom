import { describe, expect, it } from "vitest";

import type { JobDto } from "../ipc/types.ts";
import { DRY_RUN_CATEGORY_LABELS, DRY_RUN_NO_ROOTS } from "./copy.ts";
import { dryRunPair, dryRunRootsLabel, emptyScanSummary, preflightCategories } from "./preflight.ts";

const job: JobDto = {
  id: "22",
  sourceAccountId: "1",
  targetAccountId: "2",
  sourceSnapshot: {
    accountId: "1",
    email: "source@gmail.com",
    displayName: "Source",
    permissionId: "perm-s",
  },
  targetSnapshot: {
    accountId: "2",
    email: "target@gmail.com",
    displayName: "Target",
    permissionId: "perm-t",
  },
  status: "READY_FOR_REVIEW",
  queuePosition: null,
  canarySize: 5,
  createdAt: "2026-09-07T00:00:00Z",
  startedAt: null,
  completedAt: null,
  lastError: null,
  roots: [
    {
      id: "root-1",
      jobId: "22",
      rootFileId: "folder-1",
      rootName: "Archive",
      validationStatus: "VALIDATED",
      createdAt: "2026-09-07T00:00:00Z",
    },
  ],
};

describe("dry-run preflight dashboard", () => {
  it("uses job snapshots for source and target, not a registry selection", () => {
    const pair = dryRunPair(job, []);
    expect(pair?.sourceLabel).toContain("source@gmail.com");
    expect(pair?.targetLabel).toContain("target@gmail.com");
    expect(dryRunRootsLabel(job)).toBe("Archive");
    expect(dryRunRootsLabel(null)).toBe(DRY_RUN_NO_ROOTS);
  });

  it("exposes skip categories instead of four aggregate counts only", () => {
    const scan = {
      ...emptyScanSummary(),
      files: 4,
      folders: 2,
      skipped: 5,
      ineligible: 3,
      alreadyOwnedByTarget: 1,
      notOwnedBySource: 1,
      sharedDrive: 1,
      shortcuts: 1,
      trashed: 1,
      otherIneligible: 0,
    };
    const categories = preflightCategories(scan);
    const labels = categories.map((row) => row.label);
    expect(labels).toContain(DRY_RUN_CATEGORY_LABELS.eligibleFiles);
    expect(labels).toContain(DRY_RUN_CATEGORY_LABELS.alreadyOwned);
    expect(labels).toContain(DRY_RUN_CATEGORY_LABELS.sharedDrive);
    expect(labels).toContain(DRY_RUN_CATEGORY_LABELS.shortcuts);
    expect(labels).toContain(DRY_RUN_CATEGORY_LABELS.trashed);
    expect(categories.find((row) => row.id === "shortcuts")?.count).toBe(1);
  });
});
