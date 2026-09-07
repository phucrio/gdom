import { useState } from "react";
import { Dialog } from "../ui/Dialog.tsx";

type RenameDialogProps = {
  isOpen: boolean;
  currentName: string;
  onClose: () => void;
  onConfirm: (newName: string) => Promise<void>;
};

export function RenameDialog({
  isOpen,
  currentName,
  onClose,
  onConfirm,
}: RenameDialogProps) {
  const [name, setName] = useState(currentName);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!isOpen) return null;

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    const trimmed = name.trim();
    if (!trimmed) {
      setError("Name cannot be empty.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await onConfirm(trimmed);
      onClose();
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : "Failed to rename item.");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog title="Rename" onClose={onClose}>
      <form onSubmit={(e) => void handleSubmit(e)}>
        <div className="field">
          <label htmlFor="rename-input">Item name</label>
          <input
            id="rename-input"
            type="text"
            className="text-input"
            value={name}
            onChange={(e) => setName(e.target.value)}
            disabled={busy}
            autoFocus
          />
        </div>

        {error && (
          <p className="error" role="alert">
            {error}
          </p>
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
          <button
            type="submit"
            className="primary-button"
            disabled={busy || !name.trim()}
          >
            {busy ? "Renaming…" : "Rename"}
          </button>
        </div>
      </form>
    </Dialog>
  );
}

type TrashConfirmDialogProps = {
  isOpen: boolean;
  itemsCount: number;
  itemName?: string;
  onClose: () => void;
  onConfirm: () => Promise<void>;
};

export function TrashConfirmDialog({
  isOpen,
  itemsCount,
  itemName,
  onClose,
  onConfirm,
}: TrashConfirmDialogProps) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!isOpen) return null;

  async function handleConfirm() {
    setBusy(true);
    setError(null);
    try {
      await onConfirm();
      onClose();
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : "Failed to move item to trash.");
    } finally {
      setBusy(false);
    }
  }

  const promptText =
    itemsCount === 1 && itemName
      ? `Move "${itemName}" to Google Drive trash?`
      : `Move ${itemsCount} selected items to Google Drive trash?`;

  return (
    <Dialog title="Move to trash" onClose={onClose}>
      <p>{promptText}</p>
      <p className="caption">
        Items moved to trash can be restored from Google Drive trash within 30 days.
      </p>

      {error && (
        <p className="error" role="alert">
          {error}
        </p>
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
        <button
          type="button"
          className="danger-button"
          onClick={() => void handleConfirm()}
          disabled={busy}
        >
          {busy ? "Moving…" : "Move to trash"}
        </button>
      </div>
    </Dialog>
  );
}
