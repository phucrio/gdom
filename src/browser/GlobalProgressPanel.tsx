import { useEffect, useState } from "react";
import type { BackendPort } from "../ipc/port.ts";
import { canResumeJob, progressCounts } from "./progress.ts";
import { useProgressSnapshot } from "./useProgressSnapshot.ts";
import { confirmCanaryEmail } from "../wizard/canary.ts";
import { FileTypeIcon } from "./FileTypeIcon.tsx";
import { getFileIconKind } from "./format.ts";

export type GlobalProgressPanelProps = {
  jobId: string | null;
  backend: BackendPort;
  onAnnounce: (message: string) => void;
  onRefreshJobs: () => void;
  onDismiss: () => void;
};

export function GlobalProgressPanel(props: GlobalProgressPanelProps) {
  return props.jobId ? <ProgressPanel key={props.jobId} {...props} jobId={props.jobId} /> : null;
}

const TERMINAL_JOB_STATUSES = new Set(["COMPLETED", "COMPLETED_WITH_ERRORS", "CANCELLED", "FAILED"]);

function ProgressPanel({ jobId, backend, onAnnounce, onRefreshJobs, onDismiss }:
  GlobalProgressPanelProps & { jobId: string }) {
  const [expanded, setExpanded] = useState(false);
  const [page, setPage] = useState(1);
  const [actionBusy, setActionBusy] = useState(false);
  const [confirmCancel, setConfirmCancel] = useState(false);
  const [confirmationEmail, setConfirmationEmail] = useState("");
  const { job, items, hasMore, loadingItems, loadError, refresh: fetchJob } =
    useProgressSnapshot(backend, jobId, expanded ? page : 0);

  useEffect(() => {
    if (job !== null && TERMINAL_JOB_STATUSES.has(job.status)) {
      setConfirmCancel(false);
    }
  }, [job?.status]);

  function handleScroll(event: React.UIEvent<HTMLDivElement>) {
    const element = event.currentTarget;
    if (element.scrollHeight - element.scrollTop - element.clientHeight < 60 && hasMore && !loadingItems) {
      setPage((previous) => previous + 1);
    }
  }

  async function handlePause() {
    if (!jobId) return;
    setActionBusy(true);
    try {
      if (job?.phase === "scan") await backend.pauseScan(jobId);
      else await backend.pauseMigration(jobId);
      onAnnounce("Migration paused.");
      void fetchJob();
      onRefreshJobs();
    } catch (err: unknown) {
      onAnnounce(err instanceof Error ? err.message : "Failed to pause migration.");
    } finally {
      setActionBusy(false);
    }
  }

  async function handleResume() {
    if (!jobId) return;
    setActionBusy(true);
    try {
      if (job?.phase === "scan") await backend.startScan(jobId);
      else await backend.resumeMigration(jobId);
      onAnnounce("Migration resumed.");
      void fetchJob();
      onRefreshJobs();
    } catch (err: unknown) {
      onAnnounce(err instanceof Error ? err.message : "Failed to resume migration.");
    } finally {
      setActionBusy(false);
    }
  }

  async function handleCancel() {
    if (!jobId) return;
    setActionBusy(true);
    try {
      await backend.cancelMigration(jobId);
      setConfirmCancel(false);
      onAnnounce("Migration cancelled.");
      void fetchJob();
      onRefreshJobs();
    } catch (err: unknown) {
      onAnnounce(err instanceof Error ? err.message : "Failed to cancel migration.");
    } finally {
      setActionBusy(false);
    }
  }

  async function handleContinue() {
    if (!job || !confirmCanaryEmail(confirmationEmail, job.targetSnapshot.email)) return;
    setActionBusy(true);
    try {
      if (job.status === "READY_FOR_REVIEW") await backend.startCanary(jobId, confirmationEmail);
      else await backend.continueMigration(jobId);
      fetchJob();
      onRefreshJobs();
    } catch (caught: unknown) {
      onAnnounce(caught instanceof Error ? caught.message : "Failed to continue migration.");
    } finally { setActionBusy(false); }
  }

  if (!job) {
    return <div className="global-transfer-panel" role="status">
      {loadError ?? "Loading migration progress-"}
      {loadError && <button type="button" onClick={fetchJob}>Retry</button>}
    </div>;
  }

  const isScanning = job.status === "SCANNING";
  const isRunning = isScanning || job.status === "RUNNING" || job.status === "RUNNING_CANARY";
  const isPaused = canResumeJob(job);
  const isFinished = TERMINAL_JOB_STATUSES.has(job.status);
  const { total, processed, percent, succeeded: succeededCount, failed: failedCount, skipped: skippedCount } = progressCounts(job);

  return (
    <div
      className={`global-transfer-panel ${expanded ? "expanded" : ""}`}
      role="region"
      aria-label="Migration progress status"
    >
      {/* Details dropdown positioned above the bar */}
      {expanded && (
        <div className="transfer-dropdown-container">
          <div className="transfer-dropdown-header">
            <h4>Transfer items ({total})</h4>
            <span className="dropdown-counter-badge">
              {succeededCount} succeeded · {failedCount} failed · {skippedCount} skipped
            </span>
          </div>

          <div
            className="transfer-items-scrollable"
            onScroll={handleScroll}
            tabIndex={0}
            role="feed"
            aria-label="List of transfer items"
          >
            {items.length === 0 && !loadingItems ? (
              <p className="no-items">No items listed yet.</p>
            ) : (
              items.map((item) => {
                const iconKind = getFileIconKind(item.name, item.mimeType, item.mimeType.includes("folder"));
                const isVerified = item.state === "VERIFIED";
                const isFailed = item.state.includes("FAILED");
                const isSkipped = item.state.includes("SKIPPED");
                return (
                  <div key={item.id} className="transfer-item-row" role="article">
                    <span className="item-icon"><FileTypeIcon kind={iconKind} /></span>
                    <div className="item-info">
                      <span className="item-name" title={item.name}>{item.name}</span>
                      <span className="item-state-label">
                        {item.state.toLowerCase().replace(/_/g, " ")}
                      </span>
                    </div>
                    <span className={`status-indicator ${isVerified ? "success" : isFailed ? "error" : isSkipped ? "skipped" : "pending"}`}>
                      {isVerified ? "✓" : isFailed ? "✕" : isSkipped ? "—" : "•"}
                    </span>
                  </div>
                );
              })
            )}
            {loadingItems && <p className="loading-more-items">Loading items…</p>}
          </div>
        </div>
      )}

      {loadError && <div className="error" role="alert">{loadError}{" "}
        <button type="button" className="secondary-button btn-sm" onClick={fetchJob}>Retry</button>
      </div>}
      {job.lastError && <p className="warning" role="status">{job.lastError}</p>}
      {confirmCancel && (
        <div className="notice cancel-confirmation" role="alert">
          <p>Cancel this migration? Transfers already completed will not be reversed.</p>
          <div className="dialog-actions">
            <button type="button" className="secondary-button btn-sm" onClick={() => setConfirmCancel(false)} disabled={actionBusy}>Keep running</button>
            <button type="button" className="danger-button btn-sm" onClick={() => void handleCancel()} disabled={actionBusy}>{actionBusy ? "Cancelling…" : "Cancel migration"}</button>
          </div>
        </div>
      )}
      {(job.status === "CANARY_REVIEW" || job.status === "READY_FOR_REVIEW") && <div className="notice field">
        <p>{job.status === "CANARY_REVIEW" ? `Canary finished: ${succeededCount} verified - ${failedCount} failed. Review the items before approving the remaining transfers.` : `Scan finished: ${total} items. Review the items before starting the canary transfer.`}</p>
        <div className="field"><label htmlFor="canary-confirmation-email">Re-enter target email: {job.targetSnapshot.email}</label>
        <input id="canary-confirmation-email" type="email" value={confirmationEmail}
          onChange={(event) => setConfirmationEmail(event.target.value)} /></div>
          {(job.status === "CANARY_REVIEW" || job.status === "READY_FOR_REVIEW") && (
            <button type="button" className="primary-button btn-sm"
              disabled={actionBusy || !confirmCanaryEmail(confirmationEmail, job.targetSnapshot.email)}
              onClick={() => void handleContinue()}>{job.status === "READY_FOR_REVIEW" ? "Start canary transfer" : "Approve remaining transfers"}</button>
          )}
      </div>}
      {/* Main Bar */}
      <div className="transfer-bar">
        <button
          type="button"
          className="transfer-bar-toggle"
          onClick={() => setExpanded((prev) => !prev)}
          aria-expanded={expanded}
          aria-label={expanded ? "Collapse migration details" : "Expand migration details"}
        >
          <div className="transfer-bar-info">
            <div className="transfer-accounts-route">
              <span className="source-tag">{job.sourceSnapshot.email}</span>
              <span className="route-arrow" aria-hidden="true">→</span>
              <span className="target-tag">{job.targetSnapshot.email}</span>
            </div>

            <div className="transfer-status-line">
              {isScanning ? (
                <span>Scanning… {job.scan?.totalItems ?? 0} items found</span>
              ) : total > 0 ? (
                <span>
                  {processed} / {total} processed · {succeededCount} succeeded · {failedCount} failed · {skippedCount} skipped
                </span>
              ) : (
                <span>Status: {job.status.toLowerCase().replace(/_/g, " ")}</span>
              )}
            </div>

            {/* Progress Bar */}
            <div className="progress-bar-bg" aria-hidden="true">
              {isScanning ? (
                <div className="progress-bar-fill indeterminate" />
              ) : (
                <div className="progress-bar-fill" style={{ width: `${percent}%` }} />
              )}
            </div>
          </div>

          <span className="chevron-icon" aria-hidden="true">
            {expanded ? "▼" : "▲"}
          </span>
        </button>

        {/* Controls */}
        <div className="transfer-bar-controls">
          {isRunning && (
            <button
              type="button"
              className="secondary-button btn-sm"
              onClick={() => void handlePause()}
              disabled={actionBusy}
            >
              Pause
            </button>
          )}

          {isPaused && (
            <button
              type="button"
              className="primary-button btn-sm"
              onClick={() => void handleResume()}
              disabled={actionBusy}
            >
              Resume
            </button>
          )}

          {!isFinished && !isScanning && (
            <button
              type="button"
              className="danger-button btn-sm"
              onClick={() => setConfirmCancel(true)}
              disabled={actionBusy}
            >
              Cancel
            </button>
          )}

          {isFinished && (
            <button
              type="button"
              className="secondary-button btn-sm"
              onClick={onDismiss}
              aria-label="Dismiss progress panel"
            >
              Dismiss
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
