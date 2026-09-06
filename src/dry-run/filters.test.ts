import { describe, expect, it } from "vitest";

import {
  backendItemFilter,
  categoryFilter,
  clampItemPage,
  isJobItemFilter,
  itemPageCount,
} from "./filters.ts";

describe("dry-run item filters", () => {
  it("sends eligible skipped and ineligible to the backend, not all", () => {
    expect(backendItemFilter("all")).toBeNull();
    expect(backendItemFilter("eligible")).toBe("eligible");
    expect(backendItemFilter("skipped")).toBe("skipped");
    expect(backendItemFilter("ineligible")).toBe("ineligible");
  });

  it("maps dashboard categories onto the three review filters", () => {
    expect(categoryFilter("eligibleFiles")).toBe("eligible");
    expect(categoryFilter("alreadyOwned")).toBe("skipped");
    expect(categoryFilter("shortcuts")).toBe("skipped");
    expect(categoryFilter("notOwnedBySource")).toBe("ineligible");
    expect(categoryFilter("sharedDrive")).toBe("ineligible");
    expect(categoryFilter("trashed")).toBe("ineligible");
    expect(isJobItemFilter("eligible")).toBe(true);
    expect(isJobItemFilter("shortcut")).toBe(false);
  });

  it("clamps pagination without inventing a zero page", () => {
    expect(itemPageCount(0, 50)).toBe(1);
    expect(itemPageCount(50, 50)).toBe(1);
    expect(itemPageCount(51, 50)).toBe(2);
    expect(clampItemPage(0, 3)).toBe(1);
    expect(clampItemPage(9, 3)).toBe(3);
  });
});
