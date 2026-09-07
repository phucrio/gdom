import {
  NAV_LINK_CLASS,
  NAV_LINK_CURRENT_CLASS,
  PRIMARY_NAV_HOME,
  PRIMARY_NAV_JOBS,
  PRIMARY_NAV_LABEL,
  WORKSPACE_HASH,
} from "./copy.ts";
import { isPrimaryNavCurrent, type WorkspaceView } from "./workspace.ts";

type PrimaryNavProps = {
  view: WorkspaceView;
  onHome: () => void;
  onJobs: () => void;
};

function navClass(current: boolean): string {
  return current ? NAV_LINK_CURRENT_CLASS : NAV_LINK_CLASS;
}

export function PrimaryNav({ view, onHome, onJobs }: PrimaryNavProps) {
  const homeCurrent = isPrimaryNavCurrent("home", view);
  const jobsCurrent = isPrimaryNavCurrent("jobs", view);

  return (
    <nav className="nav" aria-label={PRIMARY_NAV_LABEL}>
      <a
        className={navClass(homeCurrent)}
        href={WORKSPACE_HASH.home}
        aria-current={homeCurrent ? "page" : undefined}
        onClick={(e) => {
          e.preventDefault();
          onHome();
        }}
      >
        {PRIMARY_NAV_HOME}
      </a>
      <a
        className={navClass(jobsCurrent)}
        href={WORKSPACE_HASH.jobs}
        aria-current={jobsCurrent ? "page" : undefined}
        onClick={(e) => {
          e.preventDefault();
          onJobs();
        }}
      >
        {PRIMARY_NAV_JOBS}
      </a>
    </nav>
  );
}
