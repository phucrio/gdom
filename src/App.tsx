import { useEffect, useState } from "react";

import { AvatarMenu } from "./accounts/AvatarMenu.tsx";
import { ConnectDialog } from "./accounts/ConnectDialog.tsx";
import { useAccountRegistry } from "./accounts/useAccountRegistry.ts";
import gdomIcon from "./assets/gdom-icon.svg?no-inline";
import { LandingScreen } from "./auth/LandingScreen.tsx";
import { DriveFileBrowser } from "./browser/DriveFileBrowser.tsx";
import { GlobalProgressPanel } from "./browser/GlobalProgressPanel.tsx";
import type { BackendPort } from "./ipc/port.ts";
import type { JobDto } from "./ipc/types.ts";
import { JobsList } from "./jobs/JobsList.tsx";
import { useJobCatalog } from "./jobs/useJobCatalog.ts";
import { LIMITED_USE_TITLE, PRIVACY_POLICY_TITLE } from "./legal/copy.ts";
import { LegalDialogs } from "./legal/LegalDialogs.tsx";
import { BRAND_NAME, READY_ANNOUNCEMENT } from "./nav/copy.ts";
import { PrimaryNav } from "./nav/PrimaryNav.tsx";
import { useWorkspaceNav } from "./nav/useWorkspaceNav.ts";
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
  const [activeAccountId, setActiveAccountId] = useState<string | null>(null);
  const [accountFilter, setAccountFilter] = useState<string | null>(null);
  const [activeJobId, setActiveJobId] = useState<string | null>(null);
  const [connectDialogOpen, setConnectDialogOpen] = useState(false);

  // Sync active account: defaults to first connected or available account
  useEffect(() => {
    if (accounts.accounts.length === 0) {
      setActiveAccountId(null);
      return;
    }
    // If no active account selected, or active account disconnected/removed, select first available
    const activeExists = accounts.accounts.some((a) => a.id === activeAccountId);
    if (!activeAccountId || !activeExists) {
      const firstConnected =
        accounts.accounts.find((a) => a.authStatus === "CONNECTED") ?? accounts.accounts[0];
      if (firstConnected) {
        setActiveAccountId(firstConnected.id);
      }
    }
  }, [accounts.accounts, activeAccountId]);

  // Track latest in-progress job globally if none manually dismissed
  useEffect(() => {
    if (!activeJobId && jobs.jobs.length > 0) {
      const inFlight = jobs.jobs.find(
        (j) =>
          j.status === "SCANNING" ||
          j.status === "RUNNING_CANARY" ||
          j.status === "RUNNING" ||
          j.status === "QUEUED" ||
          j.status === "PAUSED" ||
          j.status === "CANARY_REVIEW" ||
          j.status === "AUTH_REQUIRED" ||
          j.status === "SOURCE_RATE_LIMITED" ||
          j.status === "WAITING_FOR_QUOTA",
      );
      if (inFlight) {
        setActiveJobId(inFlight.id);
      }
    }
  }, [activeJobId, jobs.jobs]);

  function announce(message: string) {
    setAnnouncement(message);
  }

  // Determine active account DTO
  const activeAccount =
    accounts.accounts.find((a) => a.id === activeAccountId) ?? accounts.accounts[0] ?? null;

  if (accounts.loadError && accounts.accounts.length === 0) {
    return <main className="app-shell">
      <p className="error" role="alert">{accounts.loadError}</p>
      <button type="button" onClick={accounts.refresh}>Retry loading accounts</button>
    </main>;
  }

  // Render Landing Screen if accounts registry loaded and no accounts connected
  if (!accounts.loading && accounts.accounts.length === 0) {
    return (
      <div className="app-shell landing-shell">
        <LandingScreen
          backend={backend}
          onAnnounce={announce}
          onConnected={accounts.refresh}
          onOpenLegal={setLegal}
        />
        <LegalDialogs open={legal} onClose={() => setLegal(null)} />
      </div>
    );
  }

  return (
    <div className="app-shell">
      <header className="topbar">
        <div className="brand">
          <span className="brand-mark" aria-hidden="true">
            <img src={gdomIcon} width={24} height={24} alt="" />
          </span>
          <span className="brand-title">{BRAND_NAME}</span>
        </div>

        <PrimaryNav
          view={view}
          onHome={() => goTo("home")}
          onJobs={() => goTo("jobs")}
        />

        <div className="topbar-right">
          <div className="legal-links">
            <button
              type="button"
              className="link-button"
              onClick={() => setLegal("privacy")}
              aria-label="Privacy Policy"
            >
              {PRIVACY_POLICY_TITLE}
            </button>
            <span aria-hidden="true">·</span>
            <button
              type="button"
              className="link-button"
              onClick={() => setLegal("limited-use")}
              aria-label="Limited Use Disclosure"
            >
              {LIMITED_USE_TITLE}
            </button>
          </div>

          <AvatarMenu
            activeAccount={activeAccount}
            accounts={accounts.accounts}
            backend={backend}
            onSelectAccount={(accId) => {
              setActiveAccountId(accId);
              goTo("home");
            }}
            onAddAccount={() => setConnectDialogOpen(true)}
            onAnnounce={announce}
            onRefresh={accounts.refresh}
          />
        </div>
      </header>

      <div className="live-region" aria-live="polite" aria-atomic="true" role="status">
        {announcement}
      </div>

      <main id="main-content" className="workspace">
        {accounts.loadError && <p className="error" role="alert">{accounts.loadError}</p>}
        {view === "home" && activeAccount && (
          <DriveFileBrowser
            key={activeAccount.id}
            account={activeAccount}
            accounts={accounts.accounts}
            backend={backend}
            onAnnounce={announce}
            onMigrationStarted={(job: JobDto) => {
              setActiveJobId(job.id);
              jobs.refresh();
            }}
            onAddAccount={() => setConnectDialogOpen(true)}
          />
        )}

        {view === "jobs" && (
          <JobsList
            accounts={accounts.accounts}
            jobs={jobs.jobs}
            loading={jobs.loading}
            loadError={jobs.loadError}
            accountFilter={accountFilter}
            onAccountFilter={setAccountFilter}
            onAnnounce={announce}
            onRefresh={jobs.refresh}
            onSelectJobForProgress={(jobId) => {
              setActiveJobId(jobId);
            }}
          />
        )}
      </main>

      <aside className="progress-region" aria-label="Active migration">
        <GlobalProgressPanel
          jobId={activeJobId}
          backend={backend}
          onAnnounce={announce}
          onRefreshJobs={jobs.refresh}
          onDismiss={() => setActiveJobId(null)}
        />
      </aside>

      {/* Connect / Add Account Dialog */}
      {connectDialogOpen && (
        <ConnectDialog
          backend={backend}
          onClose={() => setConnectDialogOpen(false)}
          onConnected={() => {
            setConnectDialogOpen(false);
            accounts.refresh();
          }}
          onAnnounce={announce}
        />
      )}

      {/* Legal Dialogs */}
      <LegalDialogs open={legal} onClose={() => setLegal(null)} />
    </div>
  );
}
