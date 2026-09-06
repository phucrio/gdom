import { describe, expect, it } from "vitest";

import { DRY_RUN_NO_ELIGIBLE, DRY_RUN_QUOTA_BLOCKING, DRY_RUN_QUOTA_UNLIMITED } from "./copy.ts";
import { emptyScanSummary } from "./preflight.ts";
import { dryRunWarnings, formatBytes, formatQuotaRemaining } from "./quota.ts";

describe("dry-run quota warnings", () => {
  it("formats remaining bytes and unlimited quota in text", () => {
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(2048)).toBe("2.0 KB");
    expect(formatQuotaRemaining(null)).toBe(DRY_RUN_QUOTA_UNLIMITED);
  });

  it("marks storage overrun and empty eligible sets as blocking, skips as notices", () => {
    const blocking = dryRunWarnings(
      {
        ...emptyScanSummary(),
        files: 2,
        folders: 1,
        eligibleItems: 3,
        estimatedQuotaBytes: 80,
        targetLimitBytes: 100,
        targetRemainingBytes: 50,
        quotaWarning: true,
      },
      true,
    );
    expect(blocking.some((warning) => warning.kind === "blocking" && warning.message === DRY_RUN_QUOTA_BLOCKING)).toBe(
      true,
    );

    const emptyEligible = dryRunWarnings(emptyScanSummary(), true);
    expect(emptyEligible.some((warning) => warning.message === DRY_RUN_NO_ELIGIBLE)).toBe(true);

    const skipped = dryRunWarnings(
      { ...emptyScanSummary(), files: 1, folders: 0, eligibleItems: 1, skipped: 2 },
      true,
    );
    expect(skipped.some((warning) => warning.kind === "notice")).toBe(true);
    expect(skipped.some((warning) => warning.message === DRY_RUN_QUOTA_BLOCKING)).toBe(false);
  });
});
