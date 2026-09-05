import { useEffect, useMemo, useState } from "react";

import { isCommandMissing, toIpcError } from "../ipc/errors.ts";
import type { BackendPort } from "../ipc/port.ts";
import { IPC_EVENTS, type AccountDto, type JobDto, type JobErrorEntry, type ScanSummary } from "../ipc/types.ts";
import { QUOTA_WARNING } from "../legal/copy.ts";
import { accountDisplayLabel } from "../accounts/status.ts";
import { accountPairErrorMessage, selectAccountPair } from "./accountPair.ts";
import { type BackendCall } from "./backendCall.ts";
import { confirmCanaryEmail } from "./canary.ts";
import {
  folderParseErrorMessage,
  parseFolderInput,
  type FolderParseResult,
} from "./folderInput.ts";
import { canaryAllowsBulk, isDraftJob, scanAllowsCanary } from "./jobStatus.ts";
import {
  WIZARD_STEP_ORDER,
  WIZARD_STEP_TITLES,
  advanceWizard,
  moveToStep,
  wizardAdvanceErrorMessage,
  type WizardGate,
  type WizardStepId,
} from "./steps.ts";

type MigrationWizardProps = {
  backend: BackendPort;
  accounts: AccountDto[];
  onAnnounce: (message: string) => void;
};

type LocalRoot = {
  folderId: string;
  input: string;
};

function emptyScan(): ScanSummary {
  return {
    files: 0,
    folders: 0,
    skipped: 0,
    ineligible: 0,
    quotaWarning: false,
  };
}

export function MigrationWizard({ backend, accounts, onAnnounce }: MigrationWizardProps) {
  const [step, setStep] = useState<WizardStepId>("select-accounts");
  const [sourceAccountId, setSourceAccountId] = useState<string | null>(null);
  const [targetAccountId, setTargetAccountId] = useState<string | null>(null);
  const [rootDraft, setRootDraft] = useState("");
  const [roots, setRoots] = useState<LocalRoot[]>([]);
  const [job, setJob] = useState<JobDto | null>(null);
  const [scanCommandMissing, setScanCommandMissing] = useState(false);
  const [canaryCommandMissing, setCanaryCommandMissing] = useState(false);
  const [jobEngineUnavailable, setJobEngineUnavailable] = useState(false);
  const [canaryEmail, setCanaryEmail] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [localErrors, setLocalErrors] = useState<JobErrorEntry[]>([]);

  const source = accounts.find((account) => account.id === sourceAccountId) ?? null;
  const target = accounts.find((account) => account.id === targetAccountId) ?? null;
  const pair = selectAccountPair(sourceAccountId, targetAccountId);
  const snapshotTargetEmail = job?.targetSnapshot.email ?? target?.email ?? "";
  const canaryConfirmed = confirmCanaryEmail(canaryEmail, snapshotTargetEmail);
  const liveParse: FolderParseResult = parseFolderInput(rootDraft);
  const jobId = job?.id ?? null;
  const draftOpen = job === null || isDraftJob(job.status);
  const pairLocked = job !== null && !isDraftJob(job.status);
  const preflightReady = scanCommandMissing || (job !== null && scanAllowsCanary(job.status));
  const canaryReady =
    canaryConfirmed && (canaryCommandMissing || (job !== null && canaryAllowsBulk(job.status)));

  useEffect(() => {
    if (jobId === null) {
      return;
    }

    let cancelled = false;

    const refreshJob = () => {
      void backend
        .getJob(jobId)
        .then((next) => {
          if (!cancelled) {
            setJob(next);
          }
        })
        .catch((caught: unknown) => {
          const ipcError = toIpcError(caught, "get_job");
          if (!cancelled && !isCommandMissing(ipcError)) {
            const message =
              caught instanceof Error ? caught.message : "Could not refresh the job.";
            setError(message);
            onAnnounce(message);
          }
        });
    };

    const subscriptions = [
      IPC_EVENTS.jobStatusChanged,
      IPC_EVENTS.scanProgress,
      IPC_EVENTS.migrationProgress,
    ].map((event) => backend.subscribe(event, refreshJob));

    return () => {
      cancelled = true;
      void Promise.all(subscriptions).then((unlistens) => {
        for (const unlisten of unlistens) {
          unlisten();
        }
      });
    };
  }, [backend, jobId, onAnnounce]);

  const gate: WizardGate = useMemo(
    () => ({
      sourceAccountId,
      targetAccountId,
      validRootCount: roots.length,
      preflightReady,
      canaryConfirmed,
      pairLocked,
    }),
    [sourceAccountId, targetAccountId, roots.length, preflightReady, canaryConfirmed, pairLocked],
  );

  function showError(message: string) {
    setError(message);
    onAnnounce(message);
  }

  function goTo(next: WizardStepId) {
    const result = moveToStep(step, next, gate);
    if (!result.ok) {
      showError(wizardAdvanceErrorMessage(result.reason));
      return;
    }
    setError(null);
    setStep(result.step);
  }

  function goForward() {
    const result = advanceWizard(step, gate);
    if (!result.ok) {
      showError(wizardAdvanceErrorMessage(result.reason));
      return;
    }
    setError(null);
    setStep(result.step);
    onAnnounce(`${WIZARD_STEP_TITLES[result.step]} step.`);
  }

  function goBack() {
    const index = WIZARD_STEP_ORDER.indexOf(step);
    const previous = index > 0 ? WIZARD_STEP_ORDER[index - 1] : null;
    if (previous) {
      goTo(previous);
    }
  }

  async function withBackend<T>(work: () => Promise<T>): Promise<BackendCall<T>> {
    setBusy(true);
    try {
      const value = await work();
      setJobEngineUnavailable(false);
      return { status: "ok", value };
    } catch (caught) {
      const ipcError = toIpcError(caught, "job");
      if (isCommandMissing(ipcError)) {
        setJobEngineUnavailable(true);
        return { status: "missing" };
      }
      showError(caught instanceof Error ? caught.message : "The job command failed.");
      return { status: "error" };
    } finally {
      setBusy(false);
    }
  }

  async function ensureJob(): Promise<BackendCall<JobDto>> {
    if (!pair.ok) {
      showError(accountPairErrorMessage(pair.reason));
      return { status: "error" };
    }
    if (job !== null) {
      if (
        job.sourceAccountId === pair.sourceAccountId &&
        job.targetAccountId === pair.targetAccountId
      ) {
        return { status: "ok", value: job };
      }
      if (!isDraftJob(job.status)) {
        showError("Account pair cannot be changed after scan starts.");
        return { status: "error" };
      }
      const updated = await withBackend(() =>
        backend.updateDraftJobAccounts(job.id, pair.sourceAccountId, pair.targetAccountId),
      );
      if (updated.status === "ok") {
        setJob(updated.value);
      }
      return updated;
    }
    const created = await withBackend(() =>
      backend.createJob(pair.sourceAccountId, pair.targetAccountId),
    );
    if (created.status === "ok") {
      setJob(created.value);
    }
    return created;
  }

  function addRoot() {
    if (!draftOpen) {
      showError("Root folders cannot be modified after scan starts.");
      return;
    }
    const parsed = parseFolderInput(rootDraft);
    if (!parsed.ok) {
      showError(folderParseErrorMessage(parsed.reason));
      return;
    }
    if (roots.some((root) => root.folderId === parsed.folderId)) {
      showError("That folder is already in this job.");
      return;
    }
    setRoots((current) => [...current, { folderId: parsed.folderId, input: rootDraft.trim() }]);
    setRootDraft("");
    setScanCommandMissing(false);
    setError(null);
    onAnnounce(`Root folder ${parsed.folderId} added.`);
  }

  async function removeRoot(folderId: string) {
    if (!draftOpen) {
      showError("Root folders cannot be modified after scan starts.");
      return;
    }
    if (job !== null) {
      const persisted = job.roots.find((root) => root.rootFileId === folderId);
      if (persisted !== undefined) {
        const removed = await withBackend(() => backend.removeRoot(job.id, persisted.id));
        if (removed.status === "error") {
          return;
        }
        if (removed.status === "ok") {
          setJob(removed.value);
        }
      }
    }
    setRoots((current) => current.filter((item) => item.folderId !== folderId));
    setScanCommandMissing(false);
  }

  function selectSource(accountId: string | null) {
    if (pairLocked) {
      return;
    }
    setSourceAccountId(accountId);
    setScanCommandMissing(false);
    setCanaryCommandMissing(false);
  }

  function selectTarget(accountId: string | null) {
    if (pairLocked) {
      return;
    }
    setTargetAccountId(accountId);
    setScanCommandMissing(false);
    setCanaryCommandMissing(false);
  }

  async function handleStartScan() {
    const created = await ensureJob();
    if (created.status === "error") {
      return;
    }
    if (created.status === "missing") {
      setScanCommandMissing(true);
      onAnnounce("Job engine is unavailable. Review the local dry-run dashboard before canary.");
      return;
    }

    const persistedIds = new Set(created.value.roots.map((root) => root.rootFileId));
    for (const root of roots) {
      if (persistedIds.has(root.folderId)) {
        continue;
      }
      const added = await withBackend(() => backend.addRoot(created.value.id, root.input));
      if (added.status === "error") {
        return;
      }
      if (added.status === "ok") {
        setJob(added.value);
        persistedIds.add(root.folderId);
      }
    }

    const scanned = await withBackend(() => backend.startScan(created.value.id));
    if (scanned.status === "error") {
      return;
    }
    if (scanned.status === "ok") {
      setJob(scanned.value);
      onAnnounce("Scan started.");
      return;
    }
    setScanCommandMissing(true);
    onAnnounce("Job engine is unavailable. Review the local dry-run dashboard before canary.");
  }

  async function handleStartCanary() {
    if (!canaryConfirmed) {
      showError(wizardAdvanceErrorMessage("canary-not-confirmed"));
      return;
    }
    if (job !== null) {
      const started = await withBackend(() => backend.startCanary(job.id, canaryEmail.trim()));
      if (started.status === "error") {
        return;
      }
      if (started.status === "ok") {
        setJob(started.value);
        onAnnounce("Canary started. Review the outcome before bulk migration.");
        return;
      }
      setCanaryCommandMissing(true);
    } else {
      setCanaryCommandMissing(true);
    }
    onAnnounce("Job engine is unavailable. Confirm the target email before bulk migration.");
  }

  function handleContinueAfterCanary() {
    if (!canaryReady) {
      showError(wizardAdvanceErrorMessage("canary-not-confirmed"));
      return;
    }
    goForward();
  }

  async function handleContinueMigration() {
    if (job === null) {
      onAnnounce("Job engine is unavailable. Live migration commands are not registered yet.");
      return;
    }
    const next = await withBackend(() => backend.continueMigration(job.id));
    if (next.status === "error") {
      return;
    }
    if (next.status === "ok") {
      setJob(next.value);
      onAnnounce("Live migration running.");
      return;
    }
    onAnnounce("Job engine is unavailable. Live migration commands are not registered yet.");
  }

  async function handlePause() {
    if (job === null) {
      onAnnounce("Job engine is unavailable. Live migration commands are not registered yet.");
      return;
    }
    const next = await withBackend(() => backend.pauseMigration(job.id));
    if (next.status === "error") {
      return;
    }
    if (next.status === "ok") {
      setJob(next.value);
      onAnnounce("Migration paused.");
      return;
    }
    onAnnounce("Job engine is unavailable. Live migration commands are not registered yet.");
  }

  async function handleResume() {
    if (job === null) {
      onAnnounce("Job engine is unavailable. Live migration commands are not registered yet.");
      return;
    }
    const next = await withBackend(() => backend.resumeMigration(job.id));
    if (next.status === "error") {
      return;
    }
    if (next.status === "ok") {
      setJob(next.value);
      onAnnounce("Migration resumed.");
      return;
    }
    onAnnounce("Job engine is unavailable. Live migration commands are not registered yet.");
  }

  async function handleCancel() {
    if (job === null) {
      onAnnounce("Job engine is unavailable. Live migration commands are not registered yet.");
      return;
    }
    const next = await withBackend(() => backend.cancelMigration(job.id));
    if (next.status === "error") {
      return;
    }
    if (next.status === "ok") {
      setJob(next.value);
    }
    setLocalErrors((current) => [
      ...current,
      {
        itemId: "job",
        message: "Migration cancelled. Transferred items are not rolled back.",
        at: new Date().toISOString(),
      },
    ]);
    onAnnounce("Migration cancelled.");
  }

  const scan = job?.scan ?? emptyScan();
  const progress = job?.progress ?? { completed: 0, total: 0, currentPath: null };
  const errors = [...(job?.errors ?? []), ...localErrors];
  const progressMax = Math.max(progress.total, 1);
  const progressValue = Math.min(progress.completed, progressMax);

  return (
    <section id="migration-wizard" className="wizard" aria-labelledby="wizard-title" tabIndex={-1}>
      <div className="section-heading">
        <div>
          <p className="eyebrow">Migration job</p>
          <h2 id="wizard-title">New job wizard</h2>
        </div>
      </div>

      <ol className="wizard-steps" aria-label="Wizard steps">
        {WIZARD_STEP_ORDER.map((id, index) => {
          const current = id === step;
          return (
            <li key={id}>
              <button
                type="button"
                className={current ? "step-tab current" : "step-tab"}
                aria-current={current ? "step" : undefined}
                onClick={() => goTo(id)}
              >
                <span className="step-index">{index + 1}</span>
                {WIZARD_STEP_TITLES[id]}
              </button>
            </li>
          );
        })}
      </ol>

      {error !== null && (
        <p className="error" role="alert">
          {error}
        </p>
      )}

      {jobEngineUnavailable && (
        <p className="notice" role="status">
          Job commands are not registered in this build yet. Account pairing, folder checks, and
          the canary gate still run locally.
        </p>
      )}

      {step === "select-accounts" && (
        <div className="wizard-panel">
          <fieldset>
            <legend>Source account</legend>
            <AccountSelect
              id="source-account"
              label="Source"
              accounts={accounts}
              value={sourceAccountId}
              excludeId={null}
              disabled={pairLocked}
              onChange={selectSource}
            />
          </fieldset>
          <fieldset>
            <legend>Target account</legend>
            <AccountSelect
              id="target-account"
              label="Target"
              accounts={accounts}
              value={targetAccountId}
              excludeId={sourceAccountId}
              disabled={pairLocked}
              onChange={selectTarget}
            />
          </fieldset>
          {!pair.ok && sourceAccountId !== null && targetAccountId !== null && (
            <p className="error" role="alert">
              {accountPairErrorMessage(pair.reason)}
            </p>
          )}
        </div>
      )}

      {step === "add-roots" && (
        <div className="wizard-panel">
          <p>
            Add Drive folder URLs or raw folder IDs owned by{" "}
            {source ? accountDisplayLabel(source) : "the source account"}.
          </p>
          <div className="field">
            <label htmlFor="root-folder">Root folder URL or ID</label>
            <div className="inline-field">
              <input
                id="root-folder"
                value={rootDraft}
                onChange={(event) => setRootDraft(event.target.value)}
                aria-invalid={rootDraft.trim().length > 0 && !liveParse.ok}
                aria-describedby="root-folder-feedback"
                disabled={!draftOpen}
              />
              <button type="button" className="primary-button" onClick={addRoot} disabled={!draftOpen}>
                Add
              </button>
            </div>
            <p id="root-folder-feedback" className={liveParse.ok ? "muted" : "error"} role="status">
              {rootDraft.trim().length === 0
                ? "Paste a Drive folder URL or folder ID."
                : liveParse.ok
                  ? `Valid ${liveParse.source}: ${liveParse.folderId}`
                  : folderParseErrorMessage(liveParse.reason)}
            </p>
          </div>
          {roots.length === 0 ? (
            <p className="empty">No root folders yet.</p>
          ) : (
            <ul className="root-list">
              {roots.map((root) => (
                <li key={root.folderId}>
                  <code>{root.folderId}</code>
                  <span>{root.input}</span>
                  <button
                    type="button"
                    onClick={() => void removeRoot(root.folderId)}
                    disabled={!draftOpen || busy}
                  >
                    Remove
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
      )}

      {step === "scan-preflight" && (
        <div className="wizard-panel">
          <p>
            Scan uses the source account to list owned items. Review the dry-run before any
            ownership change.
          </p>
          <button
            type="button"
            className="primary-button"
            onClick={() => void handleStartScan()}
            disabled={busy}
          >
            {busy ? "Scanning…" : "Start scan"}
          </button>
          <div className="preflight" aria-label="Dry-run preflight dashboard">
            <article>
              <span className="metric-label">Files</span>
              <strong>{scan.files}</strong>
            </article>
            <article>
              <span className="metric-label">Folders</span>
              <strong>{scan.folders}</strong>
            </article>
            <article>
              <span className="metric-label">Skipped</span>
              <strong>{scan.skipped}</strong>
            </article>
            <article>
              <span className="metric-label">Ineligible</span>
              <strong>{scan.ineligible}</strong>
            </article>
          </div>
          <p className={scan.quotaWarning ? "warning" : "notice"} role="status">
            {QUOTA_WARNING}
          </p>
        </div>
      )}

      {step === "canary-review" && (
        <div className="wizard-panel">
          <p>
            Canary transfers a small sample first. Re-enter the target email to confirm the
            accepting account. Bulk migration cannot start until it matches.
          </p>
          <p>
            Target snapshot: <strong>{snapshotTargetEmail.length > 0 ? snapshotTargetEmail : "none selected"}</strong>
          </p>
          <div className="field">
            <label htmlFor="canary-email">Re-enter target email</label>
            <input
              id="canary-email"
              type="email"
              autoComplete="off"
              value={canaryEmail}
              onChange={(event) => setCanaryEmail(event.target.value)}
              aria-invalid={canaryEmail.length > 0 && !canaryConfirmed}
            />
          </div>
          <p role="status">
            {canaryConfirmed
              ? "Target email matches the job snapshot."
              : "Continue is blocked until the email matches the target snapshot."}
          </p>
          <div className="migration-controls">
            <button
              type="button"
              className="primary-button"
              disabled={!canaryConfirmed || busy || canaryReady}
              onClick={() => void handleStartCanary()}
            >
              Start canary
            </button>
            <button
              type="button"
              className="primary-button"
              disabled={!canaryReady || busy}
              onClick={handleContinueAfterCanary}
            >
              Continue to bulk
            </button>
          </div>
        </div>
      )}

      {step === "live-migration" && (
        <div className="wizard-panel">
          <p>
            Items transfer leaf-first: nested files, then child folders, then roots. Pause stops
            new work; cancel does not roll back completed transfers.
          </p>
          <div className="progress-block">
            <label htmlFor="leaf-first-progress">Leaf-first progress</label>
            <progress
              id="leaf-first-progress"
              max={progressMax}
              value={progressValue}
              aria-valuemin={0}
              aria-valuemax={progressMax}
              aria-valuenow={progressValue}
            />
            <p>
              {progress.completed} of {progress.total} items
              {progress.currentPath ? ` · ${progress.currentPath}` : ""}
            </p>
          </div>
          <div className="migration-controls">
            <button type="button" className="primary-button" onClick={() => void handleContinueMigration()} disabled={busy}>
              Start
            </button>
            <button type="button" onClick={() => void handlePause()} disabled={busy}>
              Pause
            </button>
            <button type="button" onClick={() => void handleResume()} disabled={busy}>
              Resume
            </button>
            <button type="button" className="danger-button" onClick={() => void handleCancel()} disabled={busy}>
              Cancel
            </button>
          </div>
          <section aria-labelledby="error-log-title">
            <h3 id="error-log-title">Error log</h3>
            {errors.length === 0 ? (
              <p className="empty">No errors.</p>
            ) : (
              <ol className="error-log">
                {errors.map((entry, index) => (
                  <li key={`${entry.itemId}-${index}`}>
                    <code>{entry.itemId}</code> {entry.message}
                  </li>
                ))}
              </ol>
            )}
          </section>
        </div>
      )}

      <div className="wizard-nav">
        <button type="button" className="ghost-button" onClick={goBack} disabled={step === "select-accounts"}>
          Back
        </button>
        {step !== "canary-review" && step !== "live-migration" && (
          <button type="button" className="primary-button" onClick={goForward}>
            Continue
          </button>
        )}
      </div>
    </section>
  );
}

type AccountSelectProps = {
  id: string;
  label: string;
  accounts: AccountDto[];
  value: string | null;
  excludeId: string | null;
  disabled?: boolean;
  onChange: (accountId: string | null) => void;
};

function AccountSelect({
  id,
  label,
  accounts,
  value,
  excludeId,
  disabled = false,
  onChange,
}: AccountSelectProps) {
  return (
    <div className="field">
      <label htmlFor={id}>{label}</label>
      <select
        id={id}
        value={value ?? ""}
        disabled={disabled}
        onChange={(event) => onChange(event.target.value === "" ? null : event.target.value)}
      >
        <option value="">Select an account</option>
        {accounts
          .filter((account) => account.authStatus === "CONNECTED" || account.id === value)
          .map((account) => (
          <option key={account.id} value={account.id} disabled={account.id === excludeId}>
            {accountDisplayLabel(account)} ({account.email})
          </option>
        ))}
      </select>
    </div>
  );
}
