import { describe, expect, it } from "vitest";

import type {
  AccountDto,
  AccountReferencesDto,
  DryRunExport,
  JobDto,
  JobItemDto,
  JobItemsPage,
  OAuthConfigDto,
} from "./types.ts";
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
  usingCustomOverride: false,
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
    totalItems: 7,
    eligibleItems: 6,
    alreadyOwnedByTarget: 1,
    notOwnedBySource: 0,
    sharedDrive: 0,
    shortcuts: 0,
    trashed: 0,
    otherIneligible: 0,
    estimatedQuotaBytes: 2048,
    targetUsageBytes: 100,
    targetLimitBytes: 10000,
    targetRemainingBytes: 9900,
  },
  progress: { completed: 0, total: 6, currentPath: null },
  errors: [],
};

const references: AccountReferencesDto = {
  accountId: "1",
  jobs: [{ jobId: "job-1", status: "DRAFT", role: "source" }],
};

const jobItem: JobItemDto = {
  id: "item-1",
  jobId: "job-1",
  fileId: "file-1",
  name: "Notes",
  mimeType: "text/plain",
  depth: 1,
  originalParentIds: ["root-1"],
  state: "ELIGIBLE",
  quotaBytesUsed: 12,
};

const itemsPage: JobItemsPage = {
  items: [jobItem],
  page: 1,
  pageSize: 50,
  total: 1,
};

const dryRunExport: DryRunExport = {
  path: "C:/Reports/gdom-dry-run-job-1.txt",
  eligibleItems: 6,
  quotaWarning: false,
};

describe("IPC DTO secret isolation", () => {
  it("does not treat account, OAuth config, or job DTO fields as secrets", () => {
    expect(secretFieldsInRecord(account)).toEqual([]);
    expect(secretFieldsInRecord(oauthConfig)).toEqual([]);
    expect(secretFieldsInRecord(job)).toEqual([]);
    expect(secretFieldsInRecord(references)).toEqual([]);
    expect(secretFieldsInRecord(jobItem)).toEqual([]);
    expect(secretFieldsInRecord(itemsPage)).toEqual([]);
    expect(secretFieldsInRecord(dryRunExport)).toEqual([]);
    expect(() => assertNoSecretFields(account)).not.toThrow();
    expect(() => assertNoSecretFields(oauthConfig)).not.toThrow();
    expect(() => assertNoSecretFields(job)).not.toThrow();
    expect(() => assertNoSecretFields(references)).not.toThrow();
    expect(() => assertNoSecretFields(jobItem)).not.toThrow();
    expect(() => assertNoSecretFields(itemsPage)).not.toThrow();
    expect(() => assertNoSecretFields(dryRunExport)).not.toThrow();
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
