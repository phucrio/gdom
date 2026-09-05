import { useCallback, useEffect, useRef, useState } from "react";

import { AccountRegistry } from "./accounts/AccountRegistry.tsx";
import type { BackendPort } from "./ipc/port.ts";
import { IPC_EVENTS, type AccountDto } from "./ipc/types.ts";
import { LegalDialogs } from "./legal/LegalDialogs.tsx";
import { MigrationWizard } from "./wizard/MigrationWizard.tsx";
import "./App.css";

type AppProps = {
  backend: BackendPort;
};

export function App({ backend }: AppProps) {
  const [accounts, setAccounts] = useState<AccountDto[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [announcement, setAnnouncement] = useState("GDOM is ready. No Google account is connected.");
  const [legal, setLegal] = useState<"privacy" | "limited-use" | null>(null);

  const announce = useCallback((message: string) => {
    setAnnouncement(message);
  }, []);

  const refreshGeneration = useRef(0);

  const refreshAccounts = useCallback(() => {
    const generation = refreshGeneration.current + 1;
    refreshGeneration.current = generation;
    setLoading(true);
    backend
      .listAccounts()
      .then((next) => {
        if (refreshGeneration.current !== generation) {
          return;
        }
        setAccounts(next);
        setLoadError(null);
        setLoading(false);
      })
      .catch((caught: unknown) => {
        if (refreshGeneration.current !== generation) {
          return;
        }
        setLoadError(
          caught instanceof Error
            ? caught.message
            : "Could not load the account registry from the local backend.",
        );
        setLoading(false);
      });
  }, [backend]);

  useEffect(() => {
    refreshAccounts();
  }, [refreshAccounts]);

  useEffect(() => {
    const subscriptions: Array<Promise<() => void>> = [
      backend.subscribe(IPC_EVENTS.accountRegistryChanged, refreshAccounts),
    ];

    return () => {
      void Promise.all(subscriptions).then((unlistens) => {
        for (const unlisten of unlistens) {
          unlisten();
        }
      });
    };
  }, [backend, refreshAccounts]);

  return (
    <div className="app-shell">
      <header className="topbar">
        <div className="brand">
          <span className="brand-mark" aria-hidden="true">
            G
          </span>
          GDOM
        </div>
        <nav className="nav" aria-label="Primary">
          <a className="nav-link" href="#account-registry">
            Accounts
          </a>
          <a className="nav-link" href="#migration-wizard">
            New job
          </a>
        </nav>
        <div className="legal-links">
          <button type="button" className="link-button" onClick={() => setLegal("privacy")}>
            Privacy Policy
          </button>
          <button type="button" className="link-button" onClick={() => setLegal("limited-use")}>
            Limited Use Disclosure
          </button>
        </div>
      </header>

      <div className="live-region" aria-live="polite" aria-atomic="true" role="status">
        {announcement}
      </div>

      <main className="workspace">
        <AccountRegistry
          backend={backend}
          accounts={accounts}
          loading={loading}
          loadError={loadError}
          onRefresh={refreshAccounts}
          onAnnounce={announce}
        />
        <MigrationWizard backend={backend} accounts={accounts} onAnnounce={announce} />
      </main>

      <LegalDialogs open={legal} onClose={() => setLegal(null)} />
    </div>
  );
}
