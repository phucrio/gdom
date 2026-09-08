import { useState } from "react";
import { Dialog } from "../ui/Dialog.tsx";
import type { UpdateStatusDto } from "../ipc/types.ts";
import type { useUpdates } from "./useUpdates.ts";

type Props = { readonly updates: ReturnType<typeof useUpdates> };
const messages: Record<UpdateStatusDto["phase"], string> = {
  idle: "Check for a newer stable release of GDOM.",
  checking: "Checking for updates…",
  upToDate: "You are using the latest stable release.",
  available: "A newer stable release is available. Downloading will not install it.",
  downloading: "Downloading and verifying the update…",
  ready: "The update is verified and ready to install. Installation will restart GDOM.",
  deferred: "Installation is deferred while migration work is active. Wait for it to finish, then confirm Install and restart again.",
  installing: "Installing the update. GDOM will restart…",
  error: "The update could not be completed. Check for updates to try again.",
  unavailable: "Automatic updates are unavailable for this installation.",
};

export function UpdateControl({ updates }: Props) {
  const [open, setOpen] = useState(false);
  const { status, pending, error } = updates;
  const phase = status?.phase;
  const busy = pending || phase === "checking" || phase === "downloading" || phase === "installing";
  const available = phase === "available" || phase === "ready" || phase === "deferred";
  return <>
    <button type="button" className="ghost-button" aria-haspopup="dialog"
      onClick={() => { setOpen(true); }}>
      {available ? "Update available" : "Check for updates"}
    </button>
    {open && <Dialog title="Software updates" onClose={() => { setOpen(false); }}>
      <div className="dialog-body update-details">
        <p>Installed version: <strong>{status?.installedVersion ?? "Loading…"}</strong></p>
        {status?.targetVersion && <p>New version: <strong>{status.targetVersion}</strong></p>}
        <p role="status" aria-live="polite">{phase ? messages[phase] : pending ? "Checking for updates…" : "Update information is unavailable."}</p>
        {(error || status?.error) && <p className="error" role="alert">{error ?? status?.error}</p>}
        {phase === "downloading" && <div>
          <progress aria-label="Update download" max={status?.totalBytes || undefined}
            value={status?.totalBytes ? status.downloadedBytes : undefined} />
          <p className="muted">{status?.downloadedBytes.toLocaleString()} bytes downloaded
            {status?.totalBytes ? ` of ${status.totalBytes.toLocaleString()}` : ""}</p>
        </div>}
        {status?.notes && <section aria-label="Release notes">
          <h3>Release notes</h3><p className="update-notes">{status.notes}</p>
        </section>}
        <div className="dialog-actions">
          {phase === "available" && <button type="button" className="primary-button" disabled={busy}
            onClick={() => { void updates.download(); }}>Confirm download</button>}
          {(phase === "ready" || phase === "deferred") && <button type="button" className="primary-button" disabled={busy}
            onClick={() => { void updates.install(); }}>Install and restart</button>}
          {phase !== "available" && phase !== "ready" && phase !== "deferred" &&
            <button type="button" className="ghost-button" disabled={busy}
              onClick={() => { void updates.check(); }}>{error || phase === "error" ? "Retry check" : "Check again"}</button>}
        </div>
      </div>
    </Dialog>}
  </>;
}
