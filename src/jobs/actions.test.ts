import { describe, expect, it } from "vitest";

import { JOB_STATUSES } from "../ipc/types.ts";
import { canDeleteDraft, listResumeKind, queuedJobsDoNotAutoStart } from "./actions.ts";

describe("jobs list actions", () => {
  it("deletes only draft jobs", () => {
    expect(canDeleteDraft("DRAFT")).toBe(true);
    expect(canDeleteDraft("SCANNING")).toBe(false);
    expect(canDeleteDraft("PAUSED")).toBe(false);
    expect(canDeleteDraft("COMPLETED")).toBe(false);
  });

  it("resumes halted mutation jobs from the list without treating queued as started", () => {
    expect(listResumeKind("AUTH_REQUIRED")).toBe("transfer");
    expect(listResumeKind("SOURCE_RATE_LIMITED")).toBe("transfer");
    expect(listResumeKind("WAITING_FOR_QUOTA")).toBe("transfer");
    expect(listResumeKind("PAUSED")).toBeNull();
    expect(listResumeKind("QUEUED")).toBeNull();
    expect(queuedJobsDoNotAutoStart("QUEUED")).toBe(true);
  });

  it("does not offer list resume for statuses that are already running or finished", () => {
    for (const status of JOB_STATUSES) {
      if (
        status === "AUTH_REQUIRED" ||
        status === "SOURCE_RATE_LIMITED" ||
        status === "WAITING_FOR_QUOTA"
      ) {
        continue;
      }
      expect(listResumeKind(status)).toBeNull();
    }
  });
});
