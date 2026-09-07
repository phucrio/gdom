import { useEffect, useRef, useState } from "react";

import type { BackendPort } from "../ipc/port.ts";
import type { AccountDto } from "../ipc/types.ts";
import { accountDisplayLabel } from "./status.ts";

type AvatarMenuProps = {
  activeAccount: AccountDto | null;
  accounts: AccountDto[];
  backend: BackendPort;
  onSelectAccount: (accountId: string) => void;
  onAddAccount: () => void;
  onAnnounce: (message: string) => void;
  onRefresh: () => void;
};

function getInitials(name: string): string {
  const parts = name.trim().split(/\s+/).filter(Boolean);
  if (parts.length === 0) {
    return "?";
  }
  const first = parts[0] ?? "";
  if (parts.length === 1) {
    return first.substring(0, 2).toUpperCase();
  }
  const last = parts[parts.length - 1] ?? "";
  return ((first[0] ?? "") + (last[0] ?? "")).toUpperCase() || "?";
}

export function AvatarMenu({
  activeAccount,
  accounts,
  backend,
  onSelectAccount,
  onAddAccount,
  onAnnounce,
  onRefresh,
}: AvatarMenuProps) {
  const [open, setOpen] = useState(false);
  const [disconnecting, setDisconnecting] = useState(false);
  const [reauthenticating, setReauthenticating] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      if (menuRef.current && !menuRef.current.contains(event.target as Node)) {
        setOpen(false);
      }
    }
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape" && open) {
        setOpen(false);
        buttonRef.current?.focus();
      }
    }
    if (open) {
      document.addEventListener("mousedown", handleClickOutside);
      document.addEventListener("keydown", handleKeyDown);
    }
    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [open]);

  if (!activeAccount) {
    return null;
  }

  const displayName = accountDisplayLabel(activeAccount);
  const initials = getInitials(displayName);
  const isNeedsReconnect = (activeAccount.authStatus === "REAUTH_REQUIRED" || activeAccount.authStatus === "DISCONNECTED");

  async function handleDisconnect() {
    if (!activeAccount) return;
    setDisconnecting(true);
    try {
      await backend.disconnectAccount(activeAccount.id);
      onAnnounce(`Account ${activeAccount.email} disconnected.`);
      setOpen(false);
      onRefresh();
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : "Failed to disconnect account.";
      onAnnounce(msg);
    } finally {
      setDisconnecting(false);
    }
  }

  async function handleReconnect(acc: AccountDto) {
    setReauthenticating(true);
    onAnnounce(`Reconnecting ${acc.email} in system browser…`);
    try {
      await backend.reauthenticateAccount(acc.id);
      onAnnounce(`Account ${acc.email} reconnected.`);
      onRefresh();
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : "Failed to reconnect account.";
      onAnnounce(msg);
    } finally {
      setReauthenticating(false);
    }
  }

  return (
    <div className="avatar-menu-container" ref={menuRef}>
      <button
        ref={buttonRef}
        type="button"
        className="avatar-button"
        aria-haspopup="true"
        aria-expanded={open}
        aria-label={`Account menu for ${displayName} (${activeAccount.email})`}
        onClick={() => setOpen((prev) => !prev)}
      >
        {activeAccount.avatarUrl ? (
          <img
            src={activeAccount.avatarUrl}
            alt=""
            className="avatar-circle avatar-img"
            referrerPolicy="no-referrer"
          />
        ) : (
          <span className="avatar-circle" aria-hidden="true">
            {initials}
          </span>
        )}
      </button>

      {open && (
        <div className="avatar-dropdown" role="menu" aria-label="Account details and switching">
          <div className="avatar-dropdown-current">
            {activeAccount.avatarUrl ? (
              <img
                src={activeAccount.avatarUrl}
                alt=""
                className="avatar-circle avatar-circle-large avatar-img"
                referrerPolicy="no-referrer"
              />
            ) : (
              <div className="avatar-circle avatar-circle-large" aria-hidden="true">
                {initials}
              </div>
            )}
            <div className="current-account-info">
              <p className="current-account-name">{displayName}</p>
              <p className="current-account-email">{activeAccount.email}</p>
              {isNeedsReconnect && (
                <span className="badge badge-warning">Reconnect required</span>
              )}
            </div>
          </div>

          <div className="avatar-dropdown-actions">
            {isNeedsReconnect ? (
              <button
                type="button"
                className="dropdown-action-button warning"
                role="menuitem"
                onClick={() => void handleReconnect(activeAccount)}
                disabled={reauthenticating}
              >
                {reauthenticating ? "Reconnecting…" : "Reconnect account"}
              </button>
            ) : null}
            <button
              type="button"
              className="dropdown-action-button"
              role="menuitem"
              onClick={() => {
                setOpen(false);
                onAddAccount();
              }}
            >
              + Add account
            </button>
            <button
              type="button"
              className="dropdown-action-button danger"
              role="menuitem"
              onClick={() => void handleDisconnect()}
              disabled={disconnecting}
            >
              {disconnecting ? "Disconnecting…" : "Disconnect account"}
            </button>
          </div>

          <div className="avatar-dropdown-divider" role="separator" />

          <p className="dropdown-section-title">All accounts</p>
          <div className="dropdown-account-list" role="group">
            {accounts.map((acc) => {
              const isActive = acc.id === activeAccount.id;
              const accName = accountDisplayLabel(acc);
              const accInitials = getInitials(accName);
              const needsReauth = (acc.authStatus === "REAUTH_REQUIRED" || acc.authStatus === "DISCONNECTED");
              return (
                <button
                  key={acc.id}
                  type="button"
                  role="menuitem"
                  className={`account-row-button ${isActive ? "active" : ""}`}
                  onClick={() => {
                    if (!isActive) {
                      onSelectAccount(acc.id);
                      setOpen(false);
                    }
                  }}
                  aria-current={isActive ? "true" : undefined}
                >
                  {acc.avatarUrl ? (
                    <img
                      src={acc.avatarUrl}
                      alt=""
                      className="avatar-circle avatar-circle-small avatar-img"
                      referrerPolicy="no-referrer"
                    />
                  ) : (
                    <span className="avatar-circle avatar-circle-small" aria-hidden="true">
                      {accInitials}
                    </span>
                  )}
                  <div className="account-row-details">
                    <span className="account-row-name">{accName}</span>
                    <span className="account-row-email">{acc.email}</span>
                  </div>
                  {needsReauth && (
                    <span className="badge badge-warning badge-sm">Reconnect</span>
                  )}
                  {isActive && (
                    <span className="check-mark" aria-hidden="true">✓</span>
                  )}
                </button>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}
