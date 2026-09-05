import { describe, expect, it } from "vitest";

import type { AccountDto } from "../ipc/types.ts";
import { accountDisplayLabel, accountStatusBadge, connectedAccountCount } from "./status.ts";

function account(overrides: Partial<AccountDto>): AccountDto {
  return {
    id: "1",
    googlePermissionId: "perm-1",
    email: "ada@gmail.com",
    displayName: "Ada",
    label: null,
    authStatus: "CONNECTED",
    connectedAt: "2026-09-05T00:00:00Z",
    lastAuthenticatedAt: "2026-09-05T00:00:00Z",
    updatedAt: "2026-09-05T00:00:00Z",
    removedAt: null,
    ...overrides,
  };
}

describe("accountStatusBadge", () => {
  it("returns CONNECTED, REAUTH_REQUIRED, and DISCONNECTED as visible text labels", () => {
    expect(accountStatusBadge("CONNECTED")).toBe("CONNECTED");
    expect(accountStatusBadge("REAUTH_REQUIRED")).toBe("REAUTH_REQUIRED");
    expect(accountStatusBadge("DISCONNECTED")).toBe("DISCONNECTED");
  });

  it("keeps additional auth states as text rather than color-only status", () => {
    expect(accountStatusBadge("TOKEN_REFRESHING")).toBe("TOKEN_REFRESHING");
    expect(accountStatusBadge("REMOVAL_PENDING")).toBe("REMOVAL_PENDING");
  });
});

describe("accountDisplayLabel", () => {
  it("prefers the local label, then display name, then email", () => {
    expect(accountDisplayLabel(account({ label: "Archive" }))).toBe("Archive");
    expect(accountDisplayLabel(account({ label: "  ", displayName: "Ada Lovelace" }))).toBe(
      "Ada Lovelace",
    );
    expect(accountDisplayLabel(account({ label: null, displayName: "  " }))).toBe("ada@gmail.com");
  });
});

describe("connectedAccountCount", () => {
  it("counts only CONNECTED accounts", () => {
    const accounts = [
      account({ id: "1", authStatus: "CONNECTED" }),
      account({ id: "2", authStatus: "REAUTH_REQUIRED" }),
      account({ id: "3", authStatus: "DISCONNECTED" }),
    ];
    expect(connectedAccountCount(accounts)).toBe(1);
  });
});
