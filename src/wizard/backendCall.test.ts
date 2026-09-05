import { describe, expect, it } from "vitest";

import { unlocksNextGate, type BackendCall } from "./backendCall.ts";

describe("unlocksNextGate", () => {
  it("unlocks after a successful job command or a true missing-command result", () => {
    const ok: BackendCall<string> = { status: "ok", value: "job" };
    expect(unlocksNextGate(ok)).toBe(true);
    expect(unlocksNextGate({ status: "missing" })).toBe(true);
  });

  it("does not unlock the next wizard gate after a real command error", () => {
    expect(unlocksNextGate({ status: "error" })).toBe(false);
  });
});
