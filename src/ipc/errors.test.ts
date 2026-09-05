import { describe, expect, it } from "vitest";

import { formatIpcError, isCommandMissing, toIpcError } from "./errors.ts";

describe("isCommandMissing", () => {
  it("does not treat structured account-not-found errors as a missing command", () => {
    const error = toIpcError(
      { kind: "accountNotFound", message: "account not found" },
      "disconnect_account",
    );
    expect(error.kind).toBe("accountNotFound");
    expect(isCommandMissing(error)).toBe(false);
    expect(formatIpcError(error)).toBe("account not found");
  });

  it("classifies Tauri unknown-command failures as missing", () => {
    const missing = toIpcError("command create_job not found", "create_job");
    expect(missing.kind).toBe("commandMissing");
    expect(isCommandMissing(missing)).toBe(true);
    expect(formatIpcError(missing)).toMatch(/create_job command is not available/);

    const unknown = toIpcError("unknown command start_scan", "start_scan");
    expect(isCommandMissing(unknown)).toBe(true);
  });

  it("does not treat ACL denials as a missing command", () => {
    const denied = toIpcError("command start_scan not allowed by the acl", "start_scan");
    expect(isCommandMissing(denied)).toBe(false);
    expect(denied.kind).toBe("internal");
  });
});
