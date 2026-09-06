import { useEffect, useRef, useState } from "react";

import { AccountRegistry } from "./accounts/AccountRegistry.tsx";
import { useAccountRegistry } from "./accounts/useAccountRegistry.ts";
import type { BackendPort } from "./ipc/port.ts";
import type { JobDto } from "./ipc/types.ts";
import { openedJobAnnouncement, openPersistedJob, resumePersistedJob } from "./jobs/catalog.ts";
import { jobCountByAccount } from "./jobs/groups.ts";
import { JobsList } from "./jobs/JobsList.tsx";
import { useJobCatalog } from "./jobs/useJobCatalog.ts";
import { LIMITED_USE_TITLE, PRIVACY_POLICY_TITLE } from "./legal/copy.ts";
import { LegalDialogs } from "./legal/LegalDialogs.tsx";
import {
  ACCOUNT_JOBS_FILTER_ANNOUNCEMENT,
  BRAND_MARK,
  BRAND_NAME,
  NEW_JOB_ANNOUNCEMENT,
  NEW_WIZARD_KEY,
  OPEN_JOB_FAILED,
  READY_ANNOUNCEMENT,
  RESUME_JOB_ANNOUNCEMENT,
  RESUME_JOB_FAILED,
} from "./nav/copy.ts";
import { PrimaryNav } from "./nav/PrimaryNav.tsx";
import { useWorkspaceNav } from "./nav/useWorkspaceNav.ts";
import { shouldClearOpenedJob } from "./nav/workspace.ts";
import { createLatestLoad } from "./ui/latestLoad.ts";
import { MigrationWizard } from "./wizard/MigrationWizard.tsx";
import "./App.css";

type AppProps = {
  backend: BackendPort;
};

export function App({ backend }: AppProps) {
  const accounts = useAccountRegistry(backend);
  const jobs = useJobCatalog(backend);
  const { view, goTo } = useWorkspaceNav();
  const [announcement, setAnnouncement] = useState(READY_ANNOUNCEMENT);
  const [legal, setLegal] = useState<"privacy" | "limited-use" | null>(null);
  const [accountFilter, setAccountFilter] = useState<string | null>(null);
  const [openedJob, setOpenedJob] = useState<JobDto | null>(null);
  const jobActionLoad = useRef(createLatestLoad());

  function announce(message: string) {
    setAnnouncement(message);
  }

  useEffect(() => {
    if (shouldClearOpenedJob(window.location.hash, view)) {
      setOpenedJob(null);
    }
  }, [view]);

  async function openJob(jobId: string) {
    const token = jobActionLoad.current.begin();
    try {
      const job = await openPersistedJob(backend, jobId);
      if (!jobActionLoad.current.isCurrent(token)) {
        return;
      }
      setOpenedJob(job);
      goTo("wizard");
      announce(openedJobAnnouncement(job));
    } catch (caught) {
      if (!jobActionLoad.current.isCurrent(token)) {
        return;
      }
      const message = caught instanceof Error ? caught.message : OPEN_JOB_FAILED;
      jobs.setLoadError(message);
      announce(message);
    }
  }

  async function resumeJob(job: JobDto) {
    const token = jobActionLoad.current.begin();
    try {
      const next = await resumePersistedJob(backend, job);
      if (!jobActionLoad.current.isCurrent(token)) {
        return;
      }
      setOpenedJob(next);
      goTo("wizard");
      announce(RESUME_JOB_ANNOUNCEMENT);
    } catch (caught) {
      if (!jobActionLoad.current.isCurrent(token)) {
        return;
      }
      const message = caught instanceof Error ? caught.message : RESUME_JOB_FAILED;
      jobs.setLoadError(message);
      announce(message);
    }
  }

  return (
    <div className="app-shell">
      <header className="topbar">
        <div className="brand">
          <span className="brand-mark" aria-hidden="true">
            {BRAND_MARK}
          </span>
          {BRAND_NAME}
        </div>
        <PrimaryNav
          view={view}
          openedExistingJob={openedJob !== null}
          onAccounts={() => {
            setOpenedJob(null);
            goTo("accounts");
          }}
          onJobs={() => {
            setOpenedJob(null);
            goTo("jobs");
          }}
          onNewJob={() => {
            setOpenedJob(null);
            goTo("wizard");
            announce(NEW_JOB_ANNOUNCEMENT);
          }}
        />
        <div className="legal-links">
          <button type="button" className="link-button" onClick={() => setLegal("privacy")}>
            {PRIVACY_POLICY_TITLE}
          </button>
          <button type="button" className="link-button" onClick={() => setLegal("limited-use")}>
            {LIMITED_USE_TITLE}
          </button>
        </div>
      </header>

      <div className="live-region" aria-live="polite" aria-atomic="true" role="status">
        {announcement}
      </div>

      <main className="workspace workspace-single">
        {view === "accounts" && (
          <AccountRegistry
            backend={backend}
            accounts={accounts.accounts}
            loading={accounts.loading}
            loadError={accounts.loadError}
            jobCounts={jobCountByAccount(jobs.jobs)}
            onShowJobs={(accountId) => {
              setAccountFilter(accountId);
              goTo("jobs");
              announce(ACCOUNT_JOBS_FILTER_ANNOUNCEMENT);
            }}
            onRefresh={accounts.refresh}
            onAnnounce={announce}
          />
        )}
        {view === "jobs" && (
          <JobsList
            backend={backend}
            accounts={accounts.accounts}
            jobs={jobs.jobs}
            loading={jobs.loading}
            loadError={jobs.loadError}
            accountFilter={accountFilter}
            onAccountFilter={setAccountFilter}
            onOpenJob={(jobId) => void openJob(jobId)}
            onResumeJob={(job) => void resumeJob(job)}
            onAnnounce={announce}
            onRefresh={jobs.refresh}
          />
        )}
        {view === "wizard" && (
          <MigrationWizard
            key={openedJob?.id ?? NEW_WIZARD_KEY}
            backend={backend}
            accounts={accounts.accounts}
            onAnnounce={announce}
            openedJob={openedJob}
          />
        )}
      </main>

      <LegalDialogs open={legal} onClose={() => setLegal(null)} />
    </div>
  );
}
