import { useState } from "react";

import type { BackendPort } from "../ipc/port.ts";
import type { AccountDto } from "../ipc/types.ts";
import { Dialog } from "../ui/Dialog.tsx";
import { ConnectDialog } from "./ConnectDialog.tsx";
import { accountDisplayLabel, accountStatusBadge, connectedAccountCount } from "./status.ts";

type AccountRegistryProps = {
  backend: BackendPort;
  accounts: AccountDto[];
  loading: boolean;
  loadError: string | null;
  onRefresh: () => void;
  onAnnounce: (message: string) => void;
};

type PendingAction =
  | { type: "label"; account: AccountDto; value: string }
  | { type: "disconnect"; account: AccountDto }
  | { type: "remove"; account: AccountDto; deleteLocalData: boolean };

export function AccountRegistry({
  backend,
  accounts,
  loading,
  loadError,
  onRefresh,
  onAnnounce,
}: AccountRegistryProps) {
  const [connectOpen, setConnectOpen] = useState(false);
  const [pending, setPending] = useState<PendingAction | null>(null);
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);

  async function runAction(label: string, work: () => Promise<void>) {
    setBusy(true);
    setActionError(null);
    try {
      await work();
      onAnnounce(label);
      setPending(null);
      onRefresh();
    } catch (caught) {
      const message = caught instanceof Error ? caught.message : "The account action failed.";
      setActionError(message);
      onAnnounce(message);
    } finally {
      setBusy(false);
    }
  }

  return (
    <section id="account-registry" className="registry" aria-labelledby="registry-title" tabIndex={-1}>
      <div className="section-heading">
        <div>
          <p className="eyebrow">Account registry</p>
          <h2 id="registry-title">Connected accounts</h2>
        </div>
        <div className="heading-actions">
          <span className="badge">{connectedAccountCount(accounts)} connected</span>
          <button type="button" className="primary-button" onClick={() => setConnectOpen(true)}>
            Connect
          </button>
        </div>
      </div>

      {loading && <p role="status">Loading accounts…</p>}
      {loadError !== null && (
        <p className="error" role="alert">
          {loadError}
        </p>
      )}

      {!loading && accounts.length === 0 && loadError === null && (
        <p className="empty">
          No Google accounts connected. Connect a personal Gmail account to start a migration.
        </p>
      )}

      {accounts.length > 0 && (
        <ul className="account-list">
          {accounts.map((account) => {
            const badge = accountStatusBadge(account.authStatus);
            return (
              <li key={account.id} className="account-card">
                <div className="account-identity">
                  <strong>{accountDisplayLabel(account)}</strong>
                  <span className="account-email">{account.email}</span>
                </div>
                <span className={`status-badge status-${account.authStatus.toLowerCase()}`}>
                  {badge}
                </span>
                <div className="account-actions">
                  <button
                    type="button"
                    onClick={() =>
                      runAction("Reauthentication opened in the system browser.", () =>
                        backend.reauthenticateAccount(account.id).then(() => undefined),
                      )
                    }
                    disabled={busy}
                  >
                    Reauthenticate
                  </button>
                  <button
                    type="button"
                    onClick={() =>
                      setPending({ type: "label", account, value: account.label ?? "" })
                    }
                    disabled={busy}
                  >
                    Edit label
                  </button>
                  <button
                    type="button"
                    onClick={() => setPending({ type: "disconnect", account })}
                    disabled={busy || account.authStatus === "DISCONNECTED"}
                  >
                    Disconnect
                  </button>
                  <button
                    type="button"
                    className="danger-button"
                    onClick={() =>
                      setPending({ type: "remove", account, deleteLocalData: false })
                    }
                    disabled={busy}
                  >
                    Remove
                  </button>
                </div>
              </li>
            );
          })}
        </ul>
      )}

      {actionError !== null && pending === null && (
        <p className="error" role="alert">
          {actionError}
        </p>
      )}

      {connectOpen && (
        <ConnectDialog
          backend={backend}
          onClose={() => setConnectOpen(false)}
          onConnected={onRefresh}
          onAnnounce={onAnnounce}
        />
      )}

      {pending?.type === "label" && (
        <Dialog title="Edit label" onClose={() => setPending(null)}>
          <form
            className="dialog-body"
            onSubmit={(event) => {
              event.preventDefault();
              const nextLabel = pending.value.trim();
              void runAction("Account label updated.", () =>
                backend
                  .updateAccountLabel(pending.account.id, nextLabel.length === 0 ? null : nextLabel)
                  .then(() => undefined),
              );
            }}
          >
            <div className="field">
              <label htmlFor="account-label">Local label for {pending.account.email}</label>
              <input
                id="account-label"
                maxLength={100}
                value={pending.value}
                onChange={(event) =>
                  setPending({ ...pending, value: event.target.value })
                }
              />
            </div>
            {actionError !== null && (
              <p className="error" role="alert">
                {actionError}
              </p>
            )}
            <div className="dialog-actions">
              <button type="button" className="ghost-button" onClick={() => setPending(null)}>
                Cancel
              </button>
              <button type="submit" className="primary-button" disabled={busy}>
                Save label
              </button>
            </div>
          </form>
        </Dialog>
      )}

      {pending?.type === "disconnect" && (
        <Dialog title="Disconnect account" onClose={() => setPending(null)}>
          <div className="dialog-body">
            <p>
              Disconnect {pending.account.email}? The refresh token is removed from the keychain.
              The local account record remains so historical jobs can keep their snapshots.
            </p>
            {actionError !== null && (
              <p className="error" role="alert">
                {actionError}
              </p>
            )}
            <div className="dialog-actions">
              <button type="button" className="ghost-button" onClick={() => setPending(null)}>
                Cancel
              </button>
              <button
                type="button"
                className="primary-button"
                disabled={busy}
                onClick={() =>
                  void runAction("Account disconnected.", () =>
                    backend.disconnectAccount(pending.account.id),
                  )
                }
              >
                Disconnect
              </button>
            </div>
          </div>
        </Dialog>
      )}

      {pending?.type === "remove" && (
        <Dialog title="Remove account" onClose={() => setPending(null)}>
          <div className="dialog-body">
            <p>
              Remove {pending.account.email} from the registry? Active jobs that still reference
              this account will block removal.
            </p>
            <label className="checkbox-row">
              <input
                type="checkbox"
                checked={pending.deleteLocalData}
                onChange={(event) =>
                  setPending({ ...pending, deleteLocalData: event.target.checked })
                }
              />
              Also delete local account data
            </label>
            {actionError !== null && (
              <p className="error" role="alert">
                {actionError}
              </p>
            )}
            <div className="dialog-actions">
              <button type="button" className="ghost-button" onClick={() => setPending(null)}>
                Cancel
              </button>
              <button
                type="button"
                className="danger-button"
                disabled={busy}
                onClick={() =>
                  void runAction("Account removed.", () =>
                    pending.deleteLocalData
                      ? backend.deleteLocalAccountData(pending.account.id, true)
                      : backend.removeAccount(pending.account.id),
                  )
                }
              >
                Remove
              </button>
            </div>
          </div>
        </Dialog>
      )}
    </section>
  );
}
