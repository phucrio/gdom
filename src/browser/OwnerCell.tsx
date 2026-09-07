import { useState } from "react";
import type { AccountDto, DriveFileItemDto } from "../ipc/types.ts";

type OwnerCellProps = {
  item: DriveFileItemDto;
  account: AccountDto;
  accounts: AccountDto[];
};

export function OwnerCell({ item, account, accounts }: OwnerCellProps) {
  const owner = item.isOwner
    ? item.owners.find(owner => owner.permissionId === account.googlePermissionId)
    : item.owners[0];
  const connectedOwner = item.isOwner
    ? account
    : accounts.find(account => account.googlePermissionId === owner?.permissionId);
  const label = item.isOwner ? "Me" : owner?.emailAddress ?? "Unknown";
  const avatarUrl = owner?.avatarUrl ?? connectedOwner?.avatarUrl;
  const [failedAvatarUrl, setFailedAvatarUrl] = useState<string | null>(null);
  const initials = (connectedOwner?.displayName || owner?.emailAddress || "?")
    .trim().slice(0, 2).toUpperCase();
  const ownerNames = item.owners.map(owner => owner.emailAddress).filter(Boolean).join(", ");

  return (
    <span className="owner-cell" title={ownerNames || undefined}>
      {avatarUrl && avatarUrl !== failedAvatarUrl ? (
        <img
          className="avatar-circle avatar-circle-small avatar-img"
          src={avatarUrl}
          alt=""
          loading="lazy"
          referrerPolicy="no-referrer"
          onError={() => setFailedAvatarUrl(avatarUrl)}
        />
      ) : (
        <span className="avatar-circle avatar-circle-small" aria-hidden="true">{initials}</span>
      )}
      <span className="owner-name">{label}</span>
    </span>
  );
}
