import { describe, expect, it } from "vitest";

import type { JobDto } from "../ipc/types.ts";
import { confirmCanaryEmail } from "./canary.ts";
import { advanceWizard, moveToStep, wizardStepForJob, type WizardGate } from "./steps.ts";

const readyThroughScan: WizardGate = {
  sourceAccountId: "src",
  targetAccountId: "tgt",
  validRootCount: 1,
  preflightReady: true,
  canaryConfirmed: false,
  canaryFinished: false,
  pairLocked: false,
};

describe("wizard step order", () => {
  it("rejects the same account pair at the first step", () => {
    const result = advanceWizard("select-accounts", {
      sourceAccountId: "same",
      targetAccountId: "same",
      validRootCount: 0,
      preflightReady: false,
      canaryConfirmed: false,
      canaryFinished: false,
      pairLocked: false,
    });
    expect(result).toEqual({ ok: false, reason: "same-source-and-target" });
  });

  it("accepts a distinct pair and advances to add-roots", () => {
    const result = advanceWizard("select-accounts", {
      sourceAccountId: "src",
      targetAccountId: "tgt",
      validRootCount: 0,
      preflightReady: false,
      canaryConfirmed: false,
      canaryFinished: false,
      pairLocked: false,
    });
    expect(result).toEqual({ ok: true, step: "add-roots" });
  });

  it("cannot skip the canary gate from scan-preflight to live-migration", () => {
    const skipped = moveToStep("scan-preflight", "live-migration", {
      ...readyThroughScan,
      canaryConfirmed: true,
    });
    expect(skipped).toEqual({ ok: false, reason: "cannot-skip-step" });
  });

  it("refuses to leave canary review until the target email is confirmed", () => {
    const blocked = advanceWizard("canary-review", readyThroughScan);
    expect(blocked).toEqual({ ok: false, reason: "canary-not-confirmed" });
    expect(confirmCanaryEmail("wrong@gmail.com", "target@gmail.com")).toBe(false);
  });

  it("allows live migration only after the canary email matches", () => {
    expect(confirmCanaryEmail("target@gmail.com", "target@gmail.com")).toBe(true);
    const emailOnly = advanceWizard("canary-review", {
      ...readyThroughScan,
      canaryConfirmed: true,
    });
    expect(emailOnly).toEqual({ ok: false, reason: "canary-incomplete" });
    const allowed = advanceWizard("canary-review", {
      ...readyThroughScan,
      canaryConfirmed: true,
      canaryFinished: true,
    });
    expect(allowed).toEqual({ ok: true, step: "live-migration" });
  });

  it("allows returning to select-accounts after scan without skipping canary forward", () => {
    const back = moveToStep("scan-preflight", "select-accounts", {
      ...readyThroughScan,
      pairLocked: true,
    });
    expect(back).toEqual({ ok: true, step: "select-accounts" });
    const skipped = moveToStep("scan-preflight", "live-migration", {
      ...readyThroughScan,
      pairLocked: true,
      canaryConfirmed: true,
    });
    expect(skipped).toEqual({ ok: false, reason: "cannot-skip-step" });
  });

  it("reopens a persisted job on the step that matches its status", () => {
    const base: JobDto = {
      id: "1",
      sourceAccountId: "1",
      targetAccountId: "2",
      sourceSnapshot: {
        accountId: "1",
        email: "a@gmail.com",
        displayName: "A",
        permissionId: "p1",
      },
      targetSnapshot: {
        accountId: "2",
        email: "b@gmail.com",
        displayName: "B",
        permissionId: "p2",
      },
      status: "DRAFT",
      queuePosition: null,
      canarySize: 5,
      createdAt: "2026-09-06T00:00:00Z",
      startedAt: null,
      completedAt: null,
      lastError: null,
      roots: [],
    };
    expect(wizardStepForJob(base)).toBe("select-accounts");
    expect(wizardStepForJob({ ...base, status: "READY_FOR_REVIEW" })).toBe("scan-preflight");
    expect(wizardStepForJob({ ...base, status: "CANARY_REVIEW" })).toBe("canary-review");
    expect(wizardStepForJob({ ...base, status: "AUTH_REQUIRED" })).toBe("live-migration");
    expect(wizardStepForJob({ ...base, status: "QUEUED", phase: "canary" })).toBe("canary-review");
    expect(wizardStepForJob({ ...base, status: "QUEUED", phase: "bulk" })).toBe("live-migration");
    expect(wizardStepForJob({ ...base, status: "PAUSED", phase: "scan" })).toBe("scan-preflight");
    expect(
      wizardStepForJob({
        ...base,
        status: "PAUSED",
        phase: "scan",
        progress: { completed: 0, total: 4, currentPath: null },
      }),
    ).toBe("scan-preflight");
  });

  it("blocks the live-migration step tab until canary has finished", () => {
    const skipped = moveToStep("canary-review", "live-migration", {
      ...readyThroughScan,
      canaryConfirmed: true,
      canaryFinished: false,
    });
    expect(skipped).toEqual({ ok: false, reason: "canary-incomplete" });
  });
});
