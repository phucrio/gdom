import { describe, expect, it } from "vitest";

import {
  canaryAllowsBulk,
  canCancelTransfer,
  canPauseScan,
  canPauseTransfer,
  canResumeScan,
  canResumeTransfer,
  canStartBulk,
  canStartCanary,
  canStartScan,
  haltStatusDetail,
  isDraftJob,
  isHaltStatus,
  isScanRunning,
  jobStatusLabel,
  scanAllowsCanary,
} from "./jobStatus.ts";

describe("job status gates", () => {
  it("treats only DRAFT as an unlocked pair", () => {
    expect(isDraftJob("DRAFT")).toBe(true);
    expect(isDraftJob("SCANNING")).toBe(false);
    expect(isDraftJob("READY_FOR_REVIEW")).toBe(false);
  });

  it("does not unlock canary until the scan is ready for review", () => {
    expect(scanAllowsCanary("DRAFT")).toBe(false);
    expect(scanAllowsCanary("SCANNING")).toBe(false);
    expect(scanAllowsCanary("PAUSED")).toBe(false);
    expect(scanAllowsCanary("FAILED")).toBe(false);
    expect(scanAllowsCanary("AUTH_REQUIRED")).toBe(false);
    expect(scanAllowsCanary("READY_FOR_REVIEW")).toBe(true);
    expect(scanAllowsCanary("CANARY_REVIEW")).toBe(true);
  });

  it("does not unlock bulk until canary review (or a later live state)", () => {
    expect(canaryAllowsBulk("READY_FOR_REVIEW")).toBe(false);
    expect(canaryAllowsBulk("RUNNING_CANARY")).toBe(false);
    expect(canaryAllowsBulk("AUTH_REQUIRED")).toBe(false);
    expect(canaryAllowsBulk("CANARY_REVIEW")).toBe(true);
    expect(canaryAllowsBulk("RUNNING")).toBe(true);
  });

  it("enables scan start, pause, and resume from worker-safe statuses", () => {
    expect(canStartScan(null)).toBe(true);
    expect(canStartScan("DRAFT")).toBe(true);
    expect(canStartScan("SCANNING")).toBe(false);
    expect(canPauseScan("SCANNING")).toBe(true);
    expect(canPauseScan("READY_FOR_REVIEW")).toBe(false);
    expect(canResumeScan("PAUSED")).toBe(true);
    expect(isScanRunning("SCANNING")).toBe(true);
  });

  it("enables canary and bulk controls without holding the whole run busy", () => {
    expect(canStartCanary("READY_FOR_REVIEW")).toBe(true);
    expect(canStartCanary("RUNNING_CANARY")).toBe(false);
    expect(canPauseTransfer("RUNNING_CANARY")).toBe(true);
    expect(canPauseTransfer("RUNNING")).toBe(true);
    expect(canResumeTransfer("PAUSED")).toBe(true);
    expect(canResumeTransfer("SOURCE_RATE_LIMITED")).toBe(true);
    expect(canResumeTransfer("WAITING_FOR_QUOTA")).toBe(true);
    expect(canStartBulk("CANARY_REVIEW")).toBe(true);
    expect(canStartBulk("RUNNING_CANARY")).toBe(false);
    expect(canCancelTransfer("RUNNING")).toBe(true);
    expect(canCancelTransfer("COMPLETED")).toBe(false);
  });

  it("describes halt states in text, not color alone", () => {
    expect(isHaltStatus("AUTH_REQUIRED")).toBe(true);
    expect(isHaltStatus("SOURCE_RATE_LIMITED")).toBe(true);
    expect(isHaltStatus("WAITING_FOR_QUOTA")).toBe(true);
    expect(isHaltStatus("RUNNING")).toBe(false);
    expect(jobStatusLabel("AUTH_REQUIRED")).toContain("Re-authentication");
    expect(jobStatusLabel("SOURCE_RATE_LIMITED")).toContain("rate limited");
    expect(jobStatusLabel("WAITING_FOR_QUOTA")).toContain("quota");
    expect(haltStatusDetail("SOURCE_RATE_LIMITED", null)).toContain("not retried automatically");
    expect(haltStatusDetail("WAITING_FOR_QUOTA", "quota exceeded")).toContain("quota exceeded");
  });
});
