import { describe, expect, it } from "vitest";

import { WORKSPACE_HASH } from "./copy.ts";
import { hashForView, isPrimaryNavCurrent, shouldClearOpenedJob, workspaceViewFromHash } from "./workspace.ts";

describe("workspace navigation", () => {
  it("maps primary nav hashes to accounts, jobs, and new job", () => {
    expect(workspaceViewFromHash(WORKSPACE_HASH.accounts)).toBe("accounts");
    expect(workspaceViewFromHash(WORKSPACE_HASH.jobs)).toBe("jobs");
    expect(workspaceViewFromHash(WORKSPACE_HASH.wizard)).toBe("wizard");
    expect(workspaceViewFromHash(WORKSPACE_HASH.newJob)).toBe("wizard");
    expect(workspaceViewFromHash("")).toBe("accounts");
  });

  it("writes hashes the primary nav can land on", () => {
    expect(hashForView("accounts")).toBe(WORKSPACE_HASH.accounts);
    expect(hashForView("jobs")).toBe(WORKSPACE_HASH.jobs);
    expect(hashForView("wizard")).toBe(WORKSPACE_HASH.wizard);
  });

  it("clears a reopened job on Accounts, Jobs, or New job hashes", () => {
    expect(shouldClearOpenedJob(WORKSPACE_HASH.newJob, "wizard")).toBe(true);
    expect(shouldClearOpenedJob(WORKSPACE_HASH.accounts, "accounts")).toBe(true);
    expect(shouldClearOpenedJob(WORKSPACE_HASH.jobs, "jobs")).toBe(true);
    expect(shouldClearOpenedJob(WORKSPACE_HASH.wizard, "wizard")).toBe(false);
  });

  it("keeps a reopened job under Jobs, not New job", () => {
    expect(isPrimaryNavCurrent("jobs", "wizard", true)).toBe(true);
    expect(isPrimaryNavCurrent("new-job", "wizard", true)).toBe(false);
    expect(isPrimaryNavCurrent("new-job", "wizard", false)).toBe(true);
    expect(isPrimaryNavCurrent("jobs", "accounts", true)).toBe(false);
  });
});
