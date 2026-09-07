import { useCallback, useEffect, useRef, useState } from "react";
import type { BackendPort } from "../ipc/port.ts";
import { IPC_EVENTS, type JobDto, type JobItemDto } from "../ipc/types.ts";
import { getFileIcon } from "../browser/format.ts";

export type GlobalProgressPanelProps = {
  jobId: string | null;
  backend: BackendPort;
  onAnnounce: (message: string) => void;
  onRefreshJobs: () => void;
  onDismiss: () => void;
};

const JOB_EVENTS = [
  IPC_EVENTS.scanProgress,
  IPC_EVENTS.migrationProgress,
  IPC_EVENTS.itemStateChanged,
  IPC_EVENTS.jobStatusChanged,
  IPC_EVENTS.canaryCompleted,
  IPC_EVENTS.migrationCompleted,
] as const;

export function GlobalProgressPanel({
  jobId,
  backend,
  onAnnounce,
  onRefreshJobs,
  onDismiss,
}: GlobalProgressPanelProps) {
  const [job, setJob] = useState<JobDto | null>(null);
  const [expanded, setExpanded] = useState(false);
  const [items, setItems] = useState<JobItemDto[]>([]);
  const [page, setPage] = useState(1);
  const [hasMore, setHasMore] = useState(false);
  const [loadingItems, setLoadingItems] = useState(false);
  const [actionBusy, setActionBusy] = useState(false);

  const scrollRef = useRef<HTMLDivElement>(null);

  const fetchJob = useCallback(async () => {
    if (!jobId) {
      setJob(null);
      return;
    }
    try {
      const data = await backend.getJob(jobId);
      setJob(data);
    } catch {
      // Ignored if job doesn't exist yet
    }
  }, [backend, jobId]);

  useEffect(() => {
    void fetchJob();
  }, [fetchJob]);

  useEffect(() => {
    if (!jobId) return;
    const subs = JOB_EVENTS.map((evt) => backend.subscribe(evt, () => void fetchJob()));
    return () => {
      void Promise.all(subs).then((unlistens) => {
        unlistens.forEach((u) => u());
      });
    };
  }, [backend, fetchJob, jobId]);

  // Load items for dropdown
  const fetchItems = useCallback(
    async (nextPage = 1, append = false) => {
      if (!jobId) return;
      setLoadingItems(true);
      try {
        const pageRes = await backend.listJobItems(jobId, null, nextPage);
        setItems((prev) => (append ? [...prev, ...pageRes.items] : pageRes.items));
        setPage(nextPage);
        setHasMore(pageRes.page * pageRes.pageSize < pageRes.total);
      } catch {
        // Ignored
      } finally {
        setLoadingItems(false);
      }
    },
    [backend, jobId],
  );

  useEffect(() => {
    if (expanded && jobId) {
      void fetchItems(1, false);
    }
  }, [expanded, fetchItems, jobId]);

  function handleScroll(e: React.UIEvent<HTMLDivElement>) {
    const el = e.currentTarget;
    if (el.scrollHeight - el.scrollTop - el.clientHeight < 60 && hasMore && !loadingItems) {
      void fetchItems(page + 1, true);
    }
  }

  async function handlePause() {
    if (!jobId) return;
    setActionBusy(true);
    try {
      await backend.pauseMigration(jobId);
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
      await backend.resumeMigration(jobId);
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
      onAnnounce("Migration cancelled.");
      void fetchJob();
      onRefreshJobs();
    } catch (err: unknown) {
      onAnnounce(err instanceof Error ? err.message : "Failed to cancel migration.");
    } finally {
      setActionBusy(false);
    }
  }

  if (!jobId || !job) {
    return null;
  }

  const isScanning = job.status === "SCANNING";
  const isRunning =
    job.status === "RUNNING" ||
    job.status === "RUNNING_CANARY" ||
    job.status === "READY_FOR_REVIEW";
  const isPaused = job.status === "PAUSED";
  const isFinished =
    job.status === "COMPLETED" ||
    job.status === "COMPLETED_WITH_ERRORS" ||
    job.status === "CANCELLED" ||
    job.status === "FAILED";

  const total = job.progress?.total ?? (job.scan?.totalItems || 0);
  const completed = job.progress?.completed ?? 0;
  const percent = total > 0 ? Math.min(100, Math.floor((completed / total) * 100)) : 0;

  const succeededCount = completed; // successfully verified/transferred
  const failedCount =
    job.errors?.length ?? (job.status === "COMPLETED_WITH_ERRORS" || job.status === "FAILED" ? 1 : 0);
  const skippedCount = job.scan?.skipped ?? 0;

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
            ref={scrollRef}
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
                const icon = getFileIcon(item.mimeType, item.mimeType.includes("folder"));
                const isVerified = item.state === "VERIFIED";
                const isFailed = item.state.includes("FAILED");
                const isSkipped = item.state.includes("SKIPPED");
                return (
                  <div key={item.id} className="transfer-item-row" role="article">
                    <span className="item-icon" aria-hidden="true">{icon}</span>
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
                  {completed} / {total} processed · {succeededCount} succeeded · {failedCount} failed · {skippedCount} skipped
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

          {!isFinished && (
            <button
              type="button"
              className="danger-button btn-sm"
              onClick={() => void handleCancel()}
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
