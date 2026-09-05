import { describe, expect, it } from "vitest";

import { confirmCanaryEmail } from "./canary.ts";

describe("confirmCanaryEmail", () => {
  it("refuses continue until the re-entered email equals the target snapshot", () => {
    expect(confirmCanaryEmail("other@gmail.com", "target@gmail.com")).toBe(false);
    expect(confirmCanaryEmail("", "target@gmail.com")).toBe(false);
    expect(confirmCanaryEmail("target@gmail.com", "")).toBe(false);
  });

  it("accepts a matching target email, ignoring case and surrounding space", () => {
    expect(confirmCanaryEmail("target@gmail.com", "target@gmail.com")).toBe(true);
    expect(confirmCanaryEmail("  Target@Gmail.com  ", "target@gmail.com")).toBe(true);
  });
});
