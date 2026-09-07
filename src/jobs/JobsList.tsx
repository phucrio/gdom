import { progressCounts } from "../browser/progress.ts";
import { useState } from "react";

import { accountDisplayLabel } from "../accounts/status.ts";
import type { AccountDto, JobDto } from "../ipc/types.ts";
import { formatDate } from "../browser/format.ts";
import { Dialog } from "../ui/Dialog.tsx";
import { haltStatusDetail, isHaltStatus, jobStatusLabel } from "./status.ts";
import {
  JOB_ACCOUNT_FILTER_ALL,
  JOB_ACCOUNT_FILTER_ID,
  JOB_ACCOUNT_FILTER_LABEL,
  JOBS_EMPTY,
  JOBS_EMPTY_FILTERED,
  JOBS_EYEBROW,
  JOBS_LOADING,
  JOBS_TITLE,
  JOBS_TITLE_ID,
  accountOptionLabel,
  jobGroupHeadingId,
  jobsShownLabel,
  queuePositionLabel,
  queuedJobsNotice,
} from "./copy.ts";
import {
  JOB_LIST_GROUPS,
  JOB_LIST_GROUP_TITLES,
  filterJobsByAccount,
  groupJobs,
} from "./groups.ts";
import { jobPairLabel } from "./pairLabel.ts";

type JobsListProps = {
  accounts: AccountDto[];
  jobs: JobDto[];
  loading: boolean;
  loadError: string | null;
  accountFilter: string | null;
  onAccountFilter: (accountId: string | null) => void;
  onAnnounce: (message: string) => void;
  onRefresh: () => void;
  onSelectJobForProgress?: (jobId: string) => void;
};

export function JobsList({
  accounts,
  jobs,
  loading,
  loadError,
  accountFilter,
  onAccountFilter,
  onSelectJobForProgress,
}: JobsListProps) {
  const [inspectJob, setInspectJob] = useState<JobDto | null>(null);

  const visible = filterJobsByAccount(jobs, accountFilter);
  const grouped = groupJobs(visible);
  const queuedCount = grouped.queued.length;

  return (
    <section id="jobs" className="jobs" aria-labelledby={JOBS_TITLE_ID} tabIndex={-1}>
      <div className="section-heading">
        <div>
          <p className="eyebrow">{JOBS_EYEBROW}</p>
          <h2 id={JOBS_TITLE_ID}>{JOBS_TITLE}</h2>
        </div>
        <div className="heading-actions">
          <span className="badge">{jobsShownLabel(visible.length)}</span>
        </div>
      </div>

      <div className="field">
        <label htmlFor={JOB_ACCOUNT_FILTER_ID}>{JOB_ACCOUNT_FILTER_LABEL}</label>
        <select
          id={JOB_ACCOUNT_FILTER_ID}
          value={accountFilter ?? ""}
          onChange={(event) => {
            const value = event.target.value;
            onAccountFilter(value.length === 0 ? null : value);
          }}
        >
          <option value="">{JOB_ACCOUNT_FILTER_ALL}</option>
          {accounts.map((account) => (
            <option key={account.id} value={account.id}>
              {accountOptionLabel(accountDisplayLabel(account), account.email)}
            </option>
          ))}
        </select>
      </div>

      {queuedCount > 0 && (
        <p className="notice" role="status">
          {queuedJobsNotice(queuedCount)}
        </p>
      )}

      {loading && <p role="status">{JOBS_LOADING}</p>}
      {loadError !== null && (
        <p className="error" role="alert">
          {loadError}
        </p>
      )}

      {!loading && visible.length === 0 && loadError === null && (
        <p className="empty">{accountFilter !== null ? JOBS_EMPTY_FILTERED : JOBS_EMPTY}</p>
      )}

      {JOB_LIST_GROUPS.map((group) => {
        const items = grouped[group];
        if (items.length === 0) {
          return null;
        }
        const headingId = jobGroupHeadingId(group);
        return (
          <section key={group} className="job-group" aria-labelledby={headingId}>
            <h3 id={headingId}>{JOB_LIST_GROUP_TITLES[group]}</h3>
            <ul className="job-list">
              {items.map((job) => {
                const halt = isHaltStatus(job.status)
                  ? haltStatusDetail(job.status, job.lastError)
                  : null;
                const { total, processed: completed, skipped, failed } = progressCounts(job);
                const dateInfo = formatDate(job.completedAt ?? job.startedAt ?? job.createdAt);

                const rootSummary =
                  job.roots.length > 0
                    ? job.roots.map((r) => r.rootName).join(", ")
                    : "Drive Selection";

                return (
                  <li key={job.id} className="job-card">
                    <div className="job-identity">
                      <strong>{jobPairLabel(job, accounts)}</strong>
                      <span className="job-meta">
                        {rootSummary} · {dateInfo.display}
                      </span>
                    </div>

                    <div className="job-status-area">
                      <span className="status-badge">{jobStatusLabel(job.status)}</span>
                      {total > 0 && (
                        <span className="job-counts-summary">
                          {completed}/{total} items · {skipped} skipped · {failed} failed
                        </span>
                      )}
                    </div>

                    {halt !== null && (
                      <p className="warning" role="status">
                        {halt}
                      </p>
                    )}
                    {job.queuePosition !== null && job.status === "QUEUED" && (
                      <p className="muted">{queuePositionLabel(job.queuePosition)}</p>
                    )}

                    <div className="job-actions">
                      <button
                        type="button"
                        className="secondary-button"
                        onClick={() => setInspectJob(job)}
                      >
                        View details
                      </button>
                      {onSelectJobForProgress && (
                        <button
                          type="button"
                          className="ghost-button"
                          onClick={() => onSelectJobForProgress(job.id)}
                        >
                          Track progress
                        </button>
                      )}
                    </div>
                  </li>
                );
              })}
            </ul>
          </section>
        );
      })}

      {/* Read-Only Details Dialog */}
      {inspectJob !== null && (
        <Dialog title="Migration history details" onClose={() => setInspectJob(null)}>
          <dl className="dialog-body job-details-view">
            <dt>Source</dt><dd>{inspectJob.sourceSnapshot.email} ({inspectJob.sourceSnapshot.displayName})</dd>
            <dt>Target</dt><dd>{inspectJob.targetSnapshot.email} ({inspectJob.targetSnapshot.displayName})</dd>
            <dt>Status</dt><dd><span className="status-badge">{jobStatusLabel(inspectJob.status)}</span></dd>
            <dt>Root folders/items</dt><dd>{inspectJob.roots.map((r) => r.rootName).join(", ") || "None"}</dd>
            <dt>Created</dt><dd>{formatDate(inspectJob.createdAt).full}</dd>
            {inspectJob.startedAt && (
              <><dt>Started</dt><dd>{formatDate(inspectJob.startedAt).full}</dd></>
            )}
            {inspectJob.completedAt && (
              <><dt>Completed</dt><dd>{formatDate(inspectJob.completedAt).full}</dd></>
            )}
            {inspectJob.lastError && (
              <><dt>Last error</dt><dd className="error">{inspectJob.lastError}</dd></>
            )}
          </dl>
        </Dialog>
      )}
    </section>
  );
}
