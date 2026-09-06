import {
  NAV_LINK_CLASS,
  NAV_LINK_CURRENT_CLASS,
  PRIMARY_NAV_ACCOUNTS,
  PRIMARY_NAV_JOBS,
  PRIMARY_NAV_LABEL,
  PRIMARY_NAV_NEW_JOB,
  WORKSPACE_HASH,
} from "./copy.ts";
import { isPrimaryNavCurrent, type WorkspaceView } from "./workspace.ts";

type PrimaryNavProps = {
  view: WorkspaceView;
  openedExistingJob: boolean;
  onAccounts: () => void;
  onJobs: () => void;
  onNewJob: () => void;
};

function navClass(current: boolean): string {
  return current ? NAV_LINK_CURRENT_CLASS : NAV_LINK_CLASS;
}

export function PrimaryNav({
  view,
  openedExistingJob,
  onAccounts,
  onJobs,
  onNewJob,
}: PrimaryNavProps) {
  const accountsCurrent = isPrimaryNavCurrent("accounts", view, openedExistingJob);
  const jobsCurrent = isPrimaryNavCurrent("jobs", view, openedExistingJob);
  const newJobCurrent = isPrimaryNavCurrent("new-job", view, openedExistingJob);

  return (
    <nav className="nav" aria-label={PRIMARY_NAV_LABEL}>
      <a
        className={navClass(accountsCurrent)}
        href={WORKSPACE_HASH.accounts}
        aria-current={accountsCurrent ? "page" : undefined}
        onClick={() => onAccounts()}
      >
        {PRIMARY_NAV_ACCOUNTS}
      </a>
      <a
        className={navClass(jobsCurrent)}
        href={WORKSPACE_HASH.jobs}
        aria-current={jobsCurrent ? "page" : undefined}
        onClick={() => onJobs()}
      >
        {PRIMARY_NAV_JOBS}
      </a>
      <a
        className={navClass(newJobCurrent)}
        href={WORKSPACE_HASH.wizard}
        aria-current={newJobCurrent ? "page" : undefined}
        onClick={(event) => {
          event.preventDefault();
          onNewJob();
        }}
      >
        {PRIMARY_NAV_NEW_JOB}
      </a>
    </nav>
  );
}
