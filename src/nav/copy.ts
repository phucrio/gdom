export const BRAND_NAME = "GDOM";

export const PRIMARY_NAV_LABEL = "Primary";
export const PRIMARY_NAV_ACCOUNTS = "Accounts";
export const PRIMARY_NAV_JOBS = "Jobs";
export const PRIMARY_NAV_NEW_JOB = "New job";

export const WORKSPACE_HASH = {
  accounts: "#account-registry",
  accountsAlias: "#accounts",
  jobs: "#jobs",
  wizard: "#migration-wizard",
  newJob: "#new-job",
} as const;

export const WORKSPACE_SECTION_ID = {
  accounts: "account-registry",
  jobs: "jobs",
  wizard: "migration-wizard",
} as const;

export const READY_ANNOUNCEMENT = "GDOM is ready. No Google account is connected.";
export const NEW_JOB_ANNOUNCEMENT = "New job wizard.";
export const ACCOUNT_JOBS_FILTER_ANNOUNCEMENT = "Showing jobs that reference this account.";
export const OPEN_JOB_FAILED = "Could not open the job.";
export const RESUME_JOB_FAILED = "Could not resume the job.";
export const RESUME_JOB_ANNOUNCEMENT = "Migration resumed from the persisted job pair.";

export const NAV_LINK_CLASS = "nav-link";
export const NAV_LINK_CURRENT_CLASS = "nav-link current";
export const NEW_WIZARD_KEY = "new";
