import { useEffect, useRef, useState } from "react";
import type { BackendPort } from "../ipc/port.ts";
import type { AccountDto, DriveFileItemDto, JobDto } from "../ipc/types.ts";
import { accountDisplayLabel } from "../accounts/status.ts";
import { Dialog } from "../ui/Dialog.tsx";

export type OwnerPickerProps = {
  isOpen: boolean;
  sourceAccount: AccountDto;
  accounts: AccountDto[];
  selectedItems: DriveFileItemDto[];
  backend: BackendPort;
  onClose: () => void;
  onMigrationStarted: (job: JobDto) => void;
  onAddAccount: () => void;
  onAnnounce: (message: string) => void;
};

export function OwnerPicker({
  isOpen,
  sourceAccount,
  accounts,
  selectedItems,
  backend,
  onClose,
  onMigrationStarted,
  onAddAccount,
  onAnnounce,
}: OwnerPickerProps) {
  const hasFolder = selectedItems.some((i) => i.isFolder);
  const [recursive, setRecursive] = useState(true);
  const [selectedTargetId, setSelectedTargetId] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const submittingRef = useRef(false);

  useEffect(() => {
    if (isOpen) {
      setRecursive(true);
      setBusy(false);
      setError(null);
      setSelectedTargetId(null);
      submittingRef.current = false;
    }
  }, [isOpen]);

  if (!isOpen) return null;

  const targetAccounts = accounts.filter(
    (acc) => acc.id !== sourceAccount.id && acc.authStatus !== "DISCONNECTED",
  );

  const itemCount = selectedItems.length;
  const sourceName = accountDisplayLabel(sourceAccount);

  const selectedTarget = targetAccounts.find((account) => account.id === selectedTargetId) ?? null;

  async function handleStartTransfer() {
    const target = selectedTarget;
    if (target === null || submittingRef.current || busy) return;
    if (target.authStatus === "REAUTH_REQUIRED") {
      setError(`Account ${target.email} requires re-authentication before it can receive ownership.`);
      return;
    }

    submittingRef.current = true;
    setBusy(true);
    setError(null);
    onAnnounce(`Initiating ownership transfer to ${target.email}…`);

    try {
      const rootFileIds = selectedItems.map((item) => item.id);
      const job = await backend.startTransferOperation({
        sourceAccountId: sourceAccount.id,
        targetAccountId: target.id,
        rootFileIds,
        recursive: hasFolder ? recursive : false,
      });

      onAnnounce(`Transfer operation started for ${itemCount} items.`);
      onMigrationStarted(job);
      onClose();
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : "Failed to start transfer operation.";
      setError(msg);
      onAnnounce(msg);
      submittingRef.current = false;
      setBusy(false);
    }
  }

  function handleAddAccount() {
    onClose();
    onAddAccount();
  }

  return (
    <Dialog title="Transfer ownership" onClose={onClose}>
      <div className="dialog-body owner-picker">
        <div className="owner-picker-header">
          <p className="owner-picker-summary">
            <strong>Source:</strong> {sourceName} ({sourceAccount.email})
          </p>
          <p className="owner-picker-selection">
            <strong>Transferring:</strong>{" "}
            {itemCount === 1
              ? `"${selectedItems[0]?.name}"`
              : `${itemCount} selected items`}
          </p>
        </div>

        {hasFolder && (
          <div className="owner-picker-recursive">
            <label className="checkbox-label" htmlFor="recursive-toggle">
              <input
                id="recursive-toggle"
                type="checkbox"
                checked={recursive}
                onChange={(e) => setRecursive(e.target.checked)}
                disabled={busy}
              />
              <span>Include all files and subfolders</span>
            </label>
          </div>
        )}

        <div className="owner-picker-notice" role="note">
          <p>
            Choose a recipient, review the transfer, then start it. GDOM checks a small sample first and automatically transfers the remaining items if the sample succeeds. Completed transfers are not automatically reversed.
          </p>
        </div>

        {error && (
          <p className="error" role="alert">
            {error}
          </p>
        )}

        <fieldset className="owner-picker-targets">
          <legend className="targets-title">Select recipient account</legend>
          {targetAccounts.length === 0 ? (
            <div className="no-targets">
              <p>No other connected Google accounts available.</p>
              <button
                type="button"
                className="secondary-button"
                onClick={handleAddAccount}
                disabled={busy}
              >
                + Add another account
              </button>
            </div>
          ) : (
            <div className="target-accounts-list" role="radiogroup" aria-label="Recipient accounts">
              {targetAccounts.map((target) => {
                const targetName = accountDisplayLabel(target);
                const isReauth = target.authStatus === "REAUTH_REQUIRED";
                return (
                  <label
                    key={target.id}
                    className={`target-account-card ${isReauth ? "disabled" : ""} ${selectedTargetId === target.id ? "selected" : ""}`}
                  >
                    <input type="radio" name="transfer-target" value={target.id} checked={selectedTargetId === target.id} onChange={() => { setSelectedTargetId(target.id); setError(null); }} disabled={busy || isReauth} aria-label={`Select ${targetName} (${target.email})`} />
                    {target.avatarUrl ? (
                      <img
                        src={target.avatarUrl}
                        alt=""
                        className="target-account-avatar avatar-img"
                        referrerPolicy="no-referrer"
                      />
                    ) : (
                      <div className="target-account-avatar" aria-hidden="true">
                        {targetName.substring(0, 2).toUpperCase()}
                      </div>
                    )}
                    <div className="target-account-info">
                      <span className="target-account-name">{targetName}</span>
                      <span className="target-account-email">{target.email}</span>
                      {isReauth && (
                        <span className="badge badge-warning badge-sm">
                          Reconnect required
                        </span>
                      )}
                    </div>
                    <span className="target-select-hint" aria-hidden="true">{selectedTargetId === target.id ? "Selected" : "Select"}</span>
                  </label>
                );
              })}

              <button
                type="button"
                className="add-target-button"
                onClick={handleAddAccount}
                disabled={busy}
              >
                + Add another account
              </button>
            </div>
          )}
        </fieldset>

        {selectedTarget !== null && (
          <div className="transfer-review" role="status">
            <strong>Ready to start</strong>
            <span>{sourceAccount.email} → {selectedTarget.email}</span>
            <span>{itemCount} item{itemCount === 1 ? "" : "s"}{hasFolder ? (recursive ? ", including subfolders" : ", selected folders only") : ""}</span>
          </div>
        )}

        <div className="dialog-actions">
          <button
            type="button"
            className="secondary-button"
            onClick={onClose}
            disabled={busy}
          >
            Cancel
          </button>
          <button type="button" className="primary-button" onClick={() => void handleStartTransfer()} disabled={busy || selectedTarget === null}>
            {busy ? "Starting transfer…" : "Start transfer"}
          </button>
        </div>
      </div>
    </Dialog>
  );
}
