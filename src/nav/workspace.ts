import { WORKSPACE_HASH } from "./copy.ts";

export const WORKSPACE_VIEWS = ["home", "jobs"] as const;

export type WorkspaceView = (typeof WORKSPACE_VIEWS)[number];

export function workspaceViewFromHash(hash: string): WorkspaceView {
  switch (hash) {
    case WORKSPACE_HASH.jobs:
      return "jobs";
    case WORKSPACE_HASH.home:
    case "":
    case "#":
    default:
      return "home";
  }
}

export function hashForView(view: WorkspaceView): string {
  switch (view) {
    case "home":
      return WORKSPACE_HASH.home;
    case "jobs":
      return WORKSPACE_HASH.jobs;
  }
}

export type PrimaryNavId = "home" | "jobs";

export function isPrimaryNavCurrent(
  item: PrimaryNavId,
  view: WorkspaceView,
): boolean {
  return item === view;
}
