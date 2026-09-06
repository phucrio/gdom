import { useEffect, useState } from "react";

import { isCommandMissing, toIpcError } from "../ipc/errors.ts";
import type { BackendPort } from "../ipc/port.ts";
import type { AccountDto, JobDto, ScanSummary } from "../ipc/types.ts";
import {
  DRY_RUN_BLOCKING_PREFIX,
  DRY_RUN_DASHBOARD_LABEL,
  DRY_RUN_EMPTY_ITEMS,
  DRY_RUN_EXPORT_BUTTON,
  DRY_RUN_EXPORT_DISABLED,
  DRY_RUN_EXPORT_LABEL,
  DRY_RUN_EXPORT_PATH_HINT,
  DRY_RUN_EXPORT_PATH_LABEL,
  DRY_RUN_EXPORT_UNAVAILABLE,
  DRY_RUN_FILTER_LABEL,
  DRY_RUN_ITEM_COLUMNS,
  DRY_RUN_ITEMS_LABEL,
  DRY_RUN_ITEMS_LOADING,
  DRY_RUN_NEXT_PAGE,
  DRY_RUN_NOTICE_PREFIX,
  DRY_RUN_PREV_PAGE,
  DRY_RUN_QUOTA_ESTIMATED_LABEL,
  DRY_RUN_QUOTA_LIMIT_LABEL,
  DRY_RUN_QUOTA_REMAINING_LABEL,
  DRY_RUN_QUOTA_USAGE_LABEL,
  DRY_RUN_ROOTS_LABEL,
  DRY_RUN_SOURCE_LABEL,
  DRY_RUN_TARGET_LABEL,
  dryRunExportedAnnouncement,
  dryRunPageStatus,
} from "./copy.ts";
import { exportDestinationError, suggestedDryRunPath } from "./exportPath.ts";
import {
  JOB_ITEM_FILTERS,
  clampItemPage,
  isJobItemFilter,
  itemFilterLabel,
  itemPageCount,
  type JobItemFilter,
} from "./filters.ts";
import { itemKindLabel, itemQuotaLabel, itemStateLabel } from "./items.ts";
import { dryRunPair, dryRunRootsLabel, preflightCategories } from "./preflight.ts";
import { dryRunWarnings, formatBytes, formatQuotaRemaining } from "./quota.ts";
import { useJobItems } from "./useJobItems.ts";

type DryRunReviewProps = {
  backend: Pick<BackendPort, "listJobItems" | "exportDryRun" | "subscribe">;
  job: JobDto | null;
  accounts: readonly AccountDto[];
  scan: ScanSummary;
  scanComplete: boolean;
  onAnnounce: (message: string) => void;
};

export function DryRunReview({
  backend,
  job,
  accounts,
  scan,
  scanComplete,
  onAnnounce,
}: DryRunReviewProps) {
  const jobId = job?.id ?? null;
  const [filter, setFilter] = useState<JobItemFilter>("all");
  const [page, setPage] = useState(1);
  const [destination, setDestination] = useState("");
  const [exportBusy, setExportBusy] = useState(false);
  const [exportError, setExportError] = useState<string | null>(null);
  const items = useJobItems(backend, jobId, filter, page);
  const pair = dryRunPair(job, accounts);
  const categories = preflightCategories(scan);
  const warnings = dryRunWarnings(scan, scanComplete);
  const pageCount = itemPageCount(items.page.total, Math.max(items.page.pageSize, 1));
  const currentPage = clampItemPage(page, pageCount);

  useEffect(() => {
    setPage(1);
  }, [filter, jobId]);

  useEffect(() => {
    setDestination(jobId === null ? "" : suggestedDryRunPath(jobId));
  }, [jobId]);

  useEffect(() => {
    if (page !== currentPage) {
      setPage(currentPage);
    }
  }, [currentPage, page]);

  async function handleExport() {
    if (jobId === null || !scanComplete) {
      return;
    }
    const invalid = exportDestinationError(destination);
    if (invalid !== null) {
      setExportError(invalid);
      onAnnounce(invalid);
      return;
    }
    setExportBusy(true);
    setExportError(null);
    try {
      const exported = await backend.exportDryRun(jobId, destination.trim());
      const message = dryRunExportedAnnouncement(exported.path, exported.eligibleItems);
      onAnnounce(message);
    } catch (caught) {
      const ipcError = toIpcError(caught, "export_dry_run");
      const message = isCommandMissing(ipcError)
        ? DRY_RUN_EXPORT_UNAVAILABLE
        : caught instanceof Error
          ? caught.message
          : DRY_RUN_EXPORT_UNAVAILABLE;
      setExportError(message);
      onAnnounce(message);
    } finally {
      setExportBusy(false);
    }
  }

  return (
    <div className="dry-run">
      <section className="dry-run-dashboard" aria-label={DRY_RUN_DASHBOARD_LABEL}>
        {pair !== null && (
          <dl className="dry-run-meta">
            <div>
              <dt>{DRY_RUN_SOURCE_LABEL}</dt>
              <dd>{pair.sourceLabel}</dd>
            </div>
            <div>
              <dt>{DRY_RUN_TARGET_LABEL}</dt>
              <dd>{pair.targetLabel}</dd>
            </div>
            <div>
              <dt>{DRY_RUN_ROOTS_LABEL}</dt>
              <dd>{dryRunRootsLabel(job)}</dd>
            </div>
          </dl>
        )}

        <div className="preflight skip-categories">
          {categories.map((category) => (
            <article key={category.id}>
              <span className="metric-label">{category.label}</span>
              <strong>{category.count}</strong>
            </article>
          ))}
        </div>

        <dl className="dry-run-quota">
          <div>
            <dt>{DRY_RUN_QUOTA_ESTIMATED_LABEL}</dt>
            <dd>{formatBytes(scan.estimatedQuotaBytes)}</dd>
          </div>
          <div>
            <dt>{DRY_RUN_QUOTA_REMAINING_LABEL}</dt>
            <dd>{formatQuotaRemaining(scan.targetRemainingBytes)}</dd>
          </div>
          <div>
            <dt>{DRY_RUN_QUOTA_USAGE_LABEL}</dt>
            <dd>{formatBytes(scan.targetUsageBytes)}</dd>
          </div>
          <div>
            <dt>{DRY_RUN_QUOTA_LIMIT_LABEL}</dt>
            <dd>{formatQuotaRemaining(scan.targetLimitBytes)}</dd>
          </div>
        </dl>

        {warnings.map((warning) => (
          <p
            key={warning.message}
            className={warning.kind === "blocking" ? "warning" : "notice"}
            role={warning.kind === "blocking" ? "alert" : "status"}
          >
            {warning.kind === "blocking" ? DRY_RUN_BLOCKING_PREFIX : DRY_RUN_NOTICE_PREFIX}
            {warning.message}
          </p>
        ))}
      </section>

      <section className="dry-run-items" aria-labelledby="dry-run-items-title">
        <div className="dry-run-items-header">
          <h3 id="dry-run-items-title">{DRY_RUN_ITEMS_LABEL}</h3>
          <div className="field">
            <label htmlFor="dry-run-item-filter">{DRY_RUN_FILTER_LABEL}</label>
            <select
              id="dry-run-item-filter"
              value={filter}
              onChange={(event) => {
                const next = event.target.value;
                if (isJobItemFilter(next)) {
                  setFilter(next);
                }
              }}
            >
              {JOB_ITEM_FILTERS.map((id) => (
                <option key={id} value={id}>
                  {itemFilterLabel(id)}
                </option>
              ))}
            </select>
          </div>
        </div>

        {items.error !== null && (
          <p className="notice" role="status">
            {items.error}
          </p>
        )}
        {items.loading ? (
          <p className="muted" role="status">
            {DRY_RUN_ITEMS_LOADING}
          </p>
        ) : items.page.items.length === 0 ? (
          <p className="empty">{DRY_RUN_EMPTY_ITEMS}</p>
        ) : (
          <div className="item-table-wrap">
            <table className="item-table">
              <caption className="visually-hidden">{DRY_RUN_ITEMS_LABEL}</caption>
              <thead>
                <tr>
                  <th scope="col">{DRY_RUN_ITEM_COLUMNS.name}</th>
                  <th scope="col">{DRY_RUN_ITEM_COLUMNS.kind}</th>
                  <th scope="col">{DRY_RUN_ITEM_COLUMNS.state}</th>
                  <th scope="col">{DRY_RUN_ITEM_COLUMNS.depth}</th>
                  <th scope="col">{DRY_RUN_ITEM_COLUMNS.fileId}</th>
                  <th scope="col">{DRY_RUN_ITEM_COLUMNS.quota}</th>
                </tr>
              </thead>
              <tbody>
                {items.page.items.map((item) => (
                  <tr key={item.id}>
                    <td>{item.name}</td>
                    <td>{itemKindLabel(item.mimeType)}</td>
                    <td>{itemStateLabel(item.state)}</td>
                    <td>{item.depth}</td>
                    <td>
                      <code>{item.fileId}</code>
                    </td>
                    <td>{itemQuotaLabel(item)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}

        <div className="item-pager">
          <button
            type="button"
            onClick={() => setPage((current) => Math.max(1, current - 1))}
            disabled={currentPage <= 1 || items.loading}
          >
            {DRY_RUN_PREV_PAGE}
          </button>
          <p role="status">{dryRunPageStatus(currentPage, pageCount, items.page.total)}</p>
          <button
            type="button"
            onClick={() => setPage((current) => current + 1)}
            disabled={currentPage >= pageCount || items.loading}
          >
            {DRY_RUN_NEXT_PAGE}
          </button>
        </div>
      </section>

      <section className="dry-run-export" aria-labelledby="dry-run-export-title">
        <h3 id="dry-run-export-title">{DRY_RUN_EXPORT_LABEL}</h3>
        <div className="field">
          <label htmlFor="dry-run-destination">{DRY_RUN_EXPORT_PATH_LABEL}</label>
          <input
            id="dry-run-destination"
            value={destination}
            onChange={(event) => setDestination(event.target.value)}
            aria-describedby="dry-run-destination-hint"
            disabled={jobId === null}
          />
          <p id="dry-run-destination-hint" className="muted">
            {DRY_RUN_EXPORT_PATH_HINT}
          </p>
        </div>
        {exportError !== null && (
          <p className="error" role="alert">
            {exportError}
          </p>
        )}
        <div className="migration-controls">
          <button
            type="button"
            className="primary-button"
            onClick={() => void handleExport()}
            disabled={jobId === null || !scanComplete || exportBusy}
          >
            {DRY_RUN_EXPORT_BUTTON}
          </button>
        </div>
        {!scanComplete && (
          <p className="muted" role="status">
            {DRY_RUN_EXPORT_DISABLED}
          </p>
        )}
      </section>
    </div>
  );
}
