import { useEffect, useRef, useState } from "react";
import type { DriveFileItemDto } from "../ipc/types.ts";

export type ContextMenuItemAction =
  | "open"
  | "openGoogleDrive"
  | "rename"
  | "copyLink"
  | "transferOwnership"
  | "moveToTrash";

export type ContextMenuProps = {
  anchorPosition: { x: number; y: number } | null;
  targetItems: DriveFileItemDto[];
  onClose: () => void;
  onAction: (action: ContextMenuItemAction, items: DriveFileItemDto[]) => void;
};

export function ContextMenu({
  anchorPosition,
  targetItems,
  onClose,
  onAction,
}: ContextMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);
  const [adjustedPos, setAdjustedPos] = useState(anchorPosition);

  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      if (menuRef.current && !menuRef.current.contains(event.target as Node)) {
        onClose();
      }
    }
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        onClose();
      }
    }
    document.addEventListener("mousedown", handleClickOutside);
    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [onClose]);

  useEffect(() => {
    if (!anchorPosition || !menuRef.current) return;
    const rect = menuRef.current.getBoundingClientRect();
    const vw = window.innerWidth;
    const vh = window.innerHeight;

    let x = anchorPosition.x;
    let y = anchorPosition.y;

    if (x + rect.width > vw - 12) {
      x = Math.max(12, vw - rect.width - 12);
    }
    if (y + rect.height > vh - 12) {
      y = Math.max(12, vh - rect.height - 12);
    }

    setAdjustedPos({ x, y });
  }, [anchorPosition]);

  if (!anchorPosition || targetItems.length === 0) {
    return null;
  }

  const isSingle = targetItems.length === 1;
  const single = targetItems[0];
  const isFolder = single?.isFolder ?? false;
  const canTransfer = targetItems.every((item) => item.canTransferOwnership);

  return (
    <div
      ref={menuRef}
      className="context-menu"
      style={{
        left: `${adjustedPos?.x ?? anchorPosition.x}px`,
        top: `${adjustedPos?.y ?? anchorPosition.y}px`,
      }}
      role="menu"
      aria-label="Item actions"
    >
      {isSingle && (
        <button
          type="button"
          className="context-menu-item"
          role="menuitem"
          onClick={() => {
            onClose();
            onAction("open", targetItems);
          }}
        >
          {isFolder ? "Open folder" : "Open"}
        </button>
      )}

      {isSingle && single?.webViewLink && (
        <button
          type="button"
          className="context-menu-item"
          role="menuitem"
          onClick={() => {
            onClose();
            onAction("openGoogleDrive", targetItems);
          }}
        >
          Open in Google Drive
        </button>
      )}

      {isSingle && (
        <button
          type="button"
          className="context-menu-item"
          role="menuitem"
          onClick={() => {
            onClose();
            onAction("rename", targetItems);
          }}
        >
          Rename
        </button>
      )}

      {isSingle && (
        <button
          type="button"
          className="context-menu-item"
          role="menuitem"
          onClick={() => {
            onClose();
            onAction("copyLink", targetItems);
          }}
        >
          Copy link
        </button>
      )}

      <div className="context-menu-divider" role="separator" />

      <button
        type="button"
        className="context-menu-item highlight"
        role="menuitem"
        disabled={!canTransfer}
        title={
          !canTransfer
            ? "You can only transfer ownership of items that you currently own."
            : undefined
        }
        onClick={() => {
          if (canTransfer) {
            onClose();
            onAction("transferOwnership", targetItems);
          }
        }}
      >
        Transfer ownership…
      </button>

      <div className="context-menu-divider" role="separator" />

      <button
        type="button"
        className="context-menu-item danger"
        role="menuitem"
        onClick={() => {
          onClose();
          onAction("moveToTrash", targetItems);
        }}
      >
        Move to trash
      </button>
    </div>
  );
}
