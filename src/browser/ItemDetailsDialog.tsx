import type { DriveFileItemDto } from "../ipc/types.ts";
import { Dialog } from "../ui/Dialog.tsx";
import { formatDate, formatFileSize } from "./format.ts";

type ItemDetailsDialogProps = {
  item: DriveFileItemDto;
  onClose: () => void;
};

export function ItemDetailsDialog({ item, onClose }: ItemDetailsDialogProps) {
  const type = item.shortcutTargetId
    ? item.folderId ? "Folder shortcut" : "File shortcut"
    : item.isFolder ? "Folder" : "File";
  const details = [
    ["Name", item.name],
    ["Type", type],
    ["File format", item.mimeType],
    ["Size", item.folderId ? "Not applicable" : item.size === null ? "Unknown" : formatFileSize(item.size)],
    ["Owner", item.owners.map(owner => owner.emailAddress ?? owner.permissionId).join(", ") || "Unknown"],
    ["Modified", item.modifiedTime ? formatDate(item.modifiedTime).full : "Unknown"],
    ["Item ID", item.id],
    ...(item.shortcutTargetId ? [["Shortcut target ID", item.shortcutTargetId]] : []),
    ["Owned by this account", item.isOwner ? "Yes" : "No"],
  ];
  return (
    <Dialog title="Item details" onClose={onClose}>
      <dl className="dialog-body item-details">
        {details.map(([label, value]) => (
          <div key={label}><dt>{label}</dt><dd>{value}</dd></div>
        ))}
      </dl>
    </Dialog>
  );
}
