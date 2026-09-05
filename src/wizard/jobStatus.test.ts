import { describe, expect, it } from "vitest";

import { canaryAllowsBulk, isDraftJob, scanAllowsCanary } from "./jobStatus.ts";

describe("job status gates", () => {
  it("treats only DRAFT as an unlocked pair", () => {
    expect(isDraftJob("DRAFT")).toBe(true);
    expect(isDraftJob("SCANNING")).toBe(false);
    expect(isDraftJob("READY_FOR_REVIEW")).toBe(false);
  });

  it("does not unlock canary until the scan has left DRAFT and SCANNING", () => {
    expect(scanAllowsCanary("DRAFT")).toBe(false);
    expect(scanAllowsCanary("SCANNING")).toBe(false);
    expect(scanAllowsCanary("READY_FOR_REVIEW")).toBe(true);
    expect(scanAllowsCanary("CANARY_REVIEW")).toBe(true);
  });

  it("does not unlock bulk until canary review (or a later live state)", () => {
    expect(canaryAllowsBulk("READY_FOR_REVIEW")).toBe(false);
    expect(canaryAllowsBulk("RUNNING_CANARY")).toBe(false);
    expect(canaryAllowsBulk("CANARY_REVIEW")).toBe(true);
    expect(canaryAllowsBulk("RUNNING")).toBe(true);
  });
});
