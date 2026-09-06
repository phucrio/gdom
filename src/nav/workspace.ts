import { WORKSPACE_HASH } from "./copy.ts";

export const WORKSPACE_VIEWS = ["accounts", "jobs", "wizard"] as const;

export type WorkspaceView = (typeof WORKSPACE_VIEWS)[number];

export function workspaceViewFromHash(hash: string): WorkspaceView {
  switch (hash) {
    case WORKSPACE_HASH.jobs:
      return "jobs";
    case WORKSPACE_HASH.wizard:
    case WORKSPACE_HASH.newJob:
      return "wizard";
    case WORKSPACE_HASH.accounts:
    case WORKSPACE_HASH.accountsAlias:
    case "":
    case "#":
      return "accounts";
    default:
      return "accounts";
  }
}

export function hashForView(view: WorkspaceView): string {
  switch (view) {
    case "accounts":
      return WORKSPACE_HASH.accounts;
    case "jobs":
      return WORKSPACE_HASH.jobs;
    case "wizard":
      return WORKSPACE_HASH.wizard;
  }
}

export type PrimaryNavId = "accounts" | "jobs" | "new-job";

/** Reopened jobs stay under Jobs; New job is only the empty wizard. */
export function shouldClearOpenedJob(hash: string, view: WorkspaceView): boolean {
  return hash === WORKSPACE_HASH.newJob || view === "accounts" || view === "jobs";
}

export function isPrimaryNavCurrent(
  item: PrimaryNavId,
  view: WorkspaceView,
  openedExistingJob: boolean,
): boolean {
  switch (item) {
    case "accounts":
      return view === "accounts";
    case "jobs":
      return view === "jobs" || (view === "wizard" && openedExistingJob);
    case "new-job":
      return view === "wizard" && !openedExistingJob;
  }
}
