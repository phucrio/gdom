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
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const submittingRef = useRef(false);

  useEffect(() => {
    if (isOpen) {
      setRecursive(true);
      setBusy(false);
      setError(null);
      submittingRef.current = false;
    }
  }, [isOpen]);

  if (!isOpen) return null;

  const targetAccounts = accounts.filter(
    (acc) => acc.id !== sourceAccount.id && acc.authStatus !== "DISCONNECTED",
  );

  const itemCount = selectedItems.length;
  const sourceName = accountDisplayLabel(sourceAccount);

  async function handleSelectTarget(target: AccountDto) {
    if (submittingRef.current || busy) return;
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

  return (
    <Dialog title="Transfer ownership" onClose={onClose}>
      <div className="owner-picker" role="dialog" aria-labelledby="owner-picker-title">
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
            Chọn tài khoản bên dưới để bắt đầu chuyển quyền sở hữu ngay. GDOM sẽ
            gửi yêu cầu bằng tài khoản nguồn và chấp nhận bằng tài khoản nhận.
            Không có hoàn tác tự động.
          </p>
        </div>

        {error && (
          <p className="error" role="alert">
            {error}
          </p>
        )}

        <div className="owner-picker-targets" role="group" aria-label="Target account list">
          <p className="targets-title">Select recipient account:</p>

          {targetAccounts.length === 0 ? (
            <div className="no-targets">
              <p>No other connected Google accounts available.</p>
              <button
                type="button"
                className="secondary-button"
                onClick={onAddAccount}
                disabled={busy}
              >
                + Add another account
              </button>
            </div>
          ) : (
            <div className="target-accounts-list">
              {targetAccounts.map((target) => {
                const targetName = accountDisplayLabel(target);
                const isReauth = target.authStatus === "REAUTH_REQUIRED";
                return (
                  <button
                    key={target.id}
                    type="button"
                    className={`target-account-card ${isReauth ? "disabled" : ""}`}
                    onClick={() => void handleSelectTarget(target)}
                    disabled={busy || isReauth}
                    aria-label={`Transfer ownership to ${targetName} (${target.email})`}
                  >
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
                    <span className="target-select-hint" aria-hidden="true">
                      {busy ? "Starting…" : "Transfer →"}
                    </span>
                  </button>
                );
              })}

              <button
                type="button"
                className="add-target-button"
                onClick={onAddAccount}
                disabled={busy}
              >
                + Add another account
              </button>
            </div>
          )}
        </div>

        <div className="dialog-actions">
          <button
            type="button"
            className="secondary-button"
            onClick={onClose}
            disabled={busy}
          >
            Cancel
          </button>
        </div>
      </div>
    </Dialog>
  );
}
