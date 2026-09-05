import type { AccountDto, AuthStatus } from "../ipc/types.ts";

const BADGE_BY_STATUS: Record<AuthStatus, string> = {
  CONNECTED: "CONNECTED",
  TOKEN_REFRESHING: "TOKEN_REFRESHING",
  REAUTH_REQUIRED: "REAUTH_REQUIRED",
  DISCONNECTED: "DISCONNECTED",
  REMOVAL_PENDING: "REMOVAL_PENDING",
};

export function accountStatusBadge(status: AuthStatus): string {
  return BADGE_BY_STATUS[status];
}

export function accountDisplayLabel(account: AccountDto): string {
  const trimmed = account.label?.trim() ?? "";
  if (trimmed.length > 0) {
    return trimmed;
  }

  if (account.displayName.trim().length > 0) {
    return account.displayName;
  }

  return account.email;
}

export function connectedAccountCount(accounts: readonly AccountDto[]): number {
  return accounts.filter((account) => account.authStatus === "CONNECTED").length;
}
