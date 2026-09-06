import { useState } from "react";

import { accountDisplayLabel } from "../accounts/status.ts";
import type { AccountDto, JobDto } from "../ipc/types.ts";
import { WORKSPACE_SECTION_ID } from "../nav/copy.ts";
import { Dialog } from "../ui/Dialog.tsx";
import { haltStatusDetail, isHaltStatus, jobStatusLabel } from "./status.ts";
import { canDeleteDraft, listResumeKind } from "./actions.ts";
import {
  JOB_ACCOUNT_FILTER_ALL,
  JOB_ACCOUNT_FILTER_ID,
  JOB_ACCOUNT_FILTER_LABEL,
  JOB_DELETE_CANCEL_LABEL,
  JOB_DELETE_DRAFT_LABEL,
  JOB_DELETE_DRAFT_TITLE,
  JOB_DELETE_FAILED,
  JOB_OPEN_LABEL,
  JOB_RESUME_LABEL,
  JOBS_EMPTY,
  JOBS_EMPTY_FILTERED,
  JOBS_EYEBROW,
  JOBS_LOADING,
  JOBS_TITLE,
  JOBS_TITLE_ID,
  accountOptionLabel,
  deleteDraftConfirmMessage,
  draftDeletedAnnouncement,
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
  backend: { deleteDraftJob(jobId: string): Promise<void> };
  accounts: AccountDto[];
  jobs: JobDto[];
  loading: boolean;
  loadError: string | null;
  accountFilter: string | null;
  onAccountFilter: (accountId: string | null) => void;
  onOpenJob: (jobId: string) => void;
  onResumeJob: (job: JobDto) => void;
  onAnnounce: (message: string) => void;
  onRefresh: () => void;
};

export function JobsList({
  backend,
  accounts,
  jobs,
  loading,
  loadError,
  accountFilter,
  onAccountFilter,
  onOpenJob,
  onResumeJob,
  onAnnounce,
  onRefresh,
}: JobsListProps) {
  const [pendingDelete, setPendingDelete] = useState<JobDto | null>(null);
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);

  const visible = filterJobsByAccount(jobs, accountFilter);
  const grouped = groupJobs(visible);
  const queuedCount = grouped.queued.length;

  async function confirmDelete() {
    if (pendingDelete === null) {
      return;
    }
    setBusy(true);
    setActionError(null);
    try {
      await backend.deleteDraftJob(pendingDelete.id);
      onAnnounce(draftDeletedAnnouncement(jobPairLabel(pendingDelete, accounts)));
      setPendingDelete(null);
      onRefresh();
    } catch (caught) {
      const message = caught instanceof Error ? caught.message : JOB_DELETE_FAILED;
      setActionError(message);
      onAnnounce(message);
    } finally {
      setBusy(false);
    }
  }

  return (
    <section id={WORKSPACE_SECTION_ID.jobs} className="jobs" aria-labelledby={JOBS_TITLE_ID} tabIndex={-1}>
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
                const resumeKind = listResumeKind(job.status);
                const halt = isHaltStatus(job.status)
                  ? haltStatusDetail(job.status, job.lastError)
                  : null;
                return (
                  <li key={job.id} className="job-card">
                    <div className="job-identity">
                      <strong>{jobPairLabel(job, accounts)}</strong>
                      <span className="job-meta">{jobStatusLabel(job.status)}</span>
                    </div>
                    <span className="status-badge">{jobStatusLabel(job.status)}</span>
                    {halt !== null && (
                      <p className="warning" role="status">
                        {halt}
                      </p>
                    )}
                    {job.queuePosition !== null && job.status === "QUEUED" && (
                      <p className="muted">{queuePositionLabel(job.queuePosition)}</p>
                    )}
                    <div className="job-actions">
                      <button type="button" className="primary-button" onClick={() => onOpenJob(job.id)}>
                        {JOB_OPEN_LABEL}
                      </button>
                      {resumeKind === "transfer" && (
                        <button type="button" onClick={() => onResumeJob(job)}>
                          {JOB_RESUME_LABEL}
                        </button>
                      )}
                      {canDeleteDraft(job.status) && (
                        <button
                          type="button"
                          className="danger-button"
                          onClick={() => {
                            setActionError(null);
                            setPendingDelete(job);
                          }}
                          disabled={busy}
                        >
                          {JOB_DELETE_DRAFT_LABEL}
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

      {pendingDelete !== null && (
        <Dialog title={JOB_DELETE_DRAFT_TITLE} onClose={() => setPendingDelete(null)}>
          <div className="dialog-body">
            <p>{deleteDraftConfirmMessage(jobPairLabel(pendingDelete, accounts))}</p>
            {actionError !== null && (
              <p className="error" role="alert">
                {actionError}
              </p>
            )}
            <div className="dialog-actions">
              <button type="button" className="ghost-button" onClick={() => setPendingDelete(null)}>
                {JOB_DELETE_CANCEL_LABEL}
              </button>
              <button
                type="button"
                className="danger-button"
                disabled={busy}
                onClick={() => void confirmDelete()}
              >
                {JOB_DELETE_DRAFT_LABEL}
              </button>
            </div>
          </div>
        </Dialog>
      )}
    </section>
  );
}
