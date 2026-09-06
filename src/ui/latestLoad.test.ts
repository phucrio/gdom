import { describe, expect, it } from "vitest";

import { createLatestLoad } from "./latestLoad.ts";

describe("latest load guard", () => {
  it("keeps only the most recent begin() token current", () => {
    const load = createLatestLoad();
    const first = load.begin();
    const second = load.begin();
    expect(load.isCurrent(first)).toBe(false);
    expect(load.isCurrent(second)).toBe(true);
  });
});
