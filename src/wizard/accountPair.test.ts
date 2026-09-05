import { describe, expect, it } from "vitest";

import { selectAccountPair } from "./accountPair.ts";

describe("selectAccountPair", () => {
  it("rejects the same account as source and target", () => {
    expect(selectAccountPair("acct-1", "acct-1")).toEqual({
      ok: false,
      reason: "same-source-and-target",
    });
  });

  it("accepts a distinct source and target pair", () => {
    expect(selectAccountPair("acct-source", "acct-target")).toEqual({
      ok: true,
      sourceAccountId: "acct-source",
      targetAccountId: "acct-target",
    });
  });

  it("rejects a missing source or target", () => {
    expect(selectAccountPair(null, "acct-target")).toEqual({
      ok: false,
      reason: "missing-source",
    });
    expect(selectAccountPair("acct-source", null)).toEqual({
      ok: false,
      reason: "missing-target",
    });
  });
});
