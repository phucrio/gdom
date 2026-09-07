import { describe, expect, it } from "vitest";

import { WORKSPACE_HASH } from "./copy.ts";
import { hashForView, isPrimaryNavCurrent, workspaceViewFromHash } from "./workspace.ts";

describe("workspace navigation", () => {
  it("maps primary nav hashes to home and jobs", () => {
    expect(workspaceViewFromHash(WORKSPACE_HASH.home)).toBe("home");
    expect(workspaceViewFromHash(WORKSPACE_HASH.jobs)).toBe("jobs");
    expect(workspaceViewFromHash("")).toBe("home");
    expect(workspaceViewFromHash("#random")).toBe("home");
  });

  it("writes hashes the primary nav can land on", () => {
    expect(hashForView("home")).toBe(WORKSPACE_HASH.home);
    expect(hashForView("jobs")).toBe(WORKSPACE_HASH.jobs);
  });

  it("evaluates active primary navigation tab", () => {
    expect(isPrimaryNavCurrent("home", "home")).toBe(true);
    expect(isPrimaryNavCurrent("home", "jobs")).toBe(false);
    expect(isPrimaryNavCurrent("jobs", "jobs")).toBe(true);
    expect(isPrimaryNavCurrent("jobs", "home")).toBe(false);
  });
});
