import { describe, expect, it } from "vitest";

import type { AccountDto, JobDto, OAuthConfigDto } from "./types.ts";
import { assertNoSecretFields, fieldNameLooksLikeSecret, secretFieldsInRecord } from "./secrets.ts";

const account: AccountDto = {
  id: "1",
  googlePermissionId: "perm-1",
  email: "ada@gmail.com",
  displayName: "Ada",
  label: "Work",
  authStatus: "CONNECTED",
  connectedAt: "2026-09-05T00:00:00Z",
  lastAuthenticatedAt: "2026-09-05T00:00:00Z",
  updatedAt: "2026-09-05T00:00:00Z",
  removedAt: null,
};

const oauthConfig: OAuthConfigDto = {
  isConfigured: true,
  clientId: "desktop-client.apps.googleusercontent.com",
};

const job: JobDto = {
  id: "job-1",
  sourceAccountId: "1",
  targetAccountId: "2",
  sourceSnapshot: {
    accountId: "1",
    email: "ada@gmail.com",
    displayName: "Ada",
    permissionId: "perm-1",
  },
  targetSnapshot: {
    accountId: "2",
    email: "grace@gmail.com",
    displayName: "Grace",
    permissionId: "perm-2",
  },
  status: "DRAFT",
  queuePosition: null,
  canarySize: 5,
  createdAt: "2026-09-05T00:00:00Z",
  startedAt: null,
  completedAt: null,
  lastError: null,
  roots: [
    {
      id: "root-1",
      jobId: "job-1",
      rootFileId: "1AbCDefGhijkLMNOPqrstuvWxyz01234",
      rootName: "Archive",
      validationStatus: "VALIDATED",
      createdAt: "2026-09-05T00:00:00Z",
    },
  ],
  scan: {
    files: 4,
    folders: 2,
    skipped: 1,
    ineligible: 0,
    quotaWarning: true,
  },
  progress: { completed: 0, total: 6, currentPath: null },
  errors: [],
};

describe("IPC DTO secret isolation", () => {
  it("does not treat account, OAuth config, or job DTO fields as secrets", () => {
    expect(secretFieldsInRecord(account)).toEqual([]);
    expect(secretFieldsInRecord(oauthConfig)).toEqual([]);
    expect(secretFieldsInRecord(job)).toEqual([]);
    expect(() => assertNoSecretFields(account)).not.toThrow();
    expect(() => assertNoSecretFields(oauthConfig)).not.toThrow();
    expect(() => assertNoSecretFields(job)).not.toThrow();
  });

  it("flags access tokens, refresh tokens, PKCE verifiers, auth codes, and client secrets", () => {
    expect(fieldNameLooksLikeSecret("accessToken")).toBe(true);
    expect(fieldNameLooksLikeSecret("refresh_token")).toBe(true);
    expect(fieldNameLooksLikeSecret("pkceVerifier")).toBe(true);
    expect(fieldNameLooksLikeSecret("authorizationCode")).toBe(true);
    expect(fieldNameLooksLikeSecret("clientSecret")).toBe(true);
    expect(fieldNameLooksLikeSecret("clientId")).toBe(false);
    expect(fieldNameLooksLikeSecret("email")).toBe(false);

    expect(() =>
      assertNoSecretFields({
        id: "1",
        refreshToken: "must-never-cross-ipc",
      }),
    ).toThrow(/refreshToken/);
    expect(() =>
      assertNoSecretFields({
        scan: { accessToken: "nested-must-never-cross" },
      }),
    ).toThrow(/accessToken/);
  });
});
