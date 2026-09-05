import { describe, expect, it } from "vitest";

import { confirmCanaryEmail } from "./canary.ts";
import { advanceWizard, moveToStep, type WizardGate } from "./steps.ts";

const readyThroughScan: WizardGate = {
  sourceAccountId: "src",
  targetAccountId: "tgt",
  validRootCount: 1,
  preflightReady: true,
  canaryConfirmed: false,
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
    const allowed = advanceWizard("canary-review", {
      ...readyThroughScan,
      canaryConfirmed: true,
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
});
