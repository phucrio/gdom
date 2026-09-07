use std::sync::atomic::{AtomicBool, Ordering};

use crate::application::AccessToken;
use crate::application::backoff::{JitterSource, MAX_RETRY_ATTEMPTS, Sleeper, backoff_delay};
use crate::application::drive_folder::DriveFolderOwner;
use crate::application::drive_transfer::{DrivePermission, DriveTransferError, DriveTransferPort};
use crate::application::item_store::{ItemStoreError, ItemStorePort};
use crate::application::job_events::{JobEventSink, JobRuntimeEvent};
use crate::application::time::iso_now;
use crate::domain::GooglePermissionId;
use crate::domain::item::{ItemError, ItemState, MigrationItem};
use crate::domain::job::{JobError, JobId, MigrationJob};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TransferPhase {
    Reconcile,
    PendingOwner,
    Accept,
    Verify,
}

#[derive(Debug)]
pub enum TransferError {
    Drive(DriveTransferError),
    Store(ItemStoreError),
    Job(JobError),
    InvalidItemState,
}

impl std::fmt::Display for TransferError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Drive(err) => write!(f, "transfer Drive error: {err}"),
            Self::Store(err) => write!(f, "transfer persistence error: {err}"),
            Self::Job(err) => write!(f, "transfer job error: {err}"),
            Self::InvalidItemState => write!(f, "illegal item state transition during transfer"),
        }
    }
}

impl std::error::Error for TransferError {}

impl From<DriveTransferError> for TransferError {
    fn from(err: DriveTransferError) -> Self {
        Self::Drive(err)
    }
}

impl From<ItemStoreError> for TransferError {
    fn from(err: ItemStoreError) -> Self {
        Self::Store(err)
    }
}

impl From<JobError> for TransferError {
    fn from(err: JobError) -> Self {
        Self::Job(err)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransferHalt {
    Exhausted { verified: usize, failed: usize },
    SharingRateLimited { message: String },
    WaitingForQuota { message: String },
    AuthRequired { message: String },
    Paused,
    Cancelled,
}

pub struct TransferRun<'a> {
    pub drive: &'a dyn DriveTransferPort,
    pub store: &'a dyn ItemStorePort,
    pub sleeper: &'a dyn Sleeper,
    pub jitter: &'a dyn JitterSource,
    pub source_token: &'a AccessToken,
    pub target_token: &'a AccessToken,
    pub source_permission_id: &'a GooglePermissionId,
    pub target_permission_id: &'a GooglePermissionId,
    pub target_email: &'a str,
    pub pause: Option<&'a AtomicBool>,
    pub cancel: Option<&'a AtomicBool>,
    pub job_id: JobId,
    pub events: Option<&'a dyn JobEventSink>,
    pub progress_total: u64,
}

pub async fn execute_canary(
    run: &TransferRun<'_>,
    job: &mut MigrationJob,
) -> Result<TransferHalt, TransferError> {
    job.start_canary()?;
    let batch = select_canary_batch(run, job).await?;
    let halt = transfer_items(run, &batch).await?;
    apply_halt(job, &halt)?;
    Ok(halt)
}

pub async fn execute_auto_transfer(
    run: &TransferRun<'_>,
    job: &mut MigrationJob,
) -> Result<TransferHalt, TransferError> {
    let canary_halt = execute_canary(run, job).await?;
    if let TransferHalt::Exhausted { .. } = canary_halt {
        let remaining = run.store.list_items_for_transfer(job.id()).await?;
        if remaining.iter().all(|item| item.canary_selected) {
            let cohort = run.store.list_canary_cohort(job.id()).await?;
            let had_errors = cohort.iter().any(|item| {
                matches!(
                    item.state,
                    ItemState::RetryableFailed | ItemState::PermanentFailed
                )
            });
            job.start_bulk()?;
            job.complete_transfer(iso_now(), had_errors)?;
        }
    }
    Ok(canary_halt)
}

async fn select_canary_batch(
    run: &TransferRun<'_>,
    job: &MigrationJob,
) -> Result<Vec<MigrationItem>, TransferError> {
    let cohort = run.store.list_canary_cohort(job.id()).await?;
    if !cohort.is_empty() {
        return Ok(cohort
            .into_iter()
            .filter(|item| !item.state.is_terminal())
            .collect());
    }

    let items = run.store.list_items_for_transfer(job.id()).await?;
    let mut in_progress: Vec<MigrationItem> = items
        .iter()
        .filter(|item| item.state != ItemState::Eligible)
        .cloned()
        .collect();
    if !in_progress.is_empty() {
        for item in &mut in_progress {
            if !item.canary_selected {
                item.canary_selected = true;
                run.store.save_item(item).await?;
            }
        }
        return Ok(in_progress);
    }

    let mut batch: Vec<MigrationItem> = items.into_iter().take(job.canary_size()).collect();
    for item in &mut batch {
        item.canary_selected = true;
        if item.state == ItemState::Eligible {
            apply_state(item, ItemState::PendingOwnerRequired).map_err(StepError::into_transfer)?;
        }
        run.store.save_item(item).await?;
    }
    Ok(batch)
}

pub async fn execute_bulk(
    run: &TransferRun<'_>,
    job: &mut MigrationJob,
) -> Result<TransferHalt, TransferError> {
    job.start_bulk()?;
    let items = run.store.list_items_for_transfer(job.id()).await?;
    let halt = transfer_items(run, &items).await?;
    if matches!(halt, TransferHalt::Exhausted { .. }) {
        let aggregates = run.store.item_aggregates(job.id()).await?;
        job.complete_transfer(iso_now(), aggregates.failed > 0)?;
    } else {
        apply_halt(job, &halt)?;
    }
    Ok(halt)
}

fn apply_halt(job: &mut MigrationJob, halt: &TransferHalt) -> Result<(), JobError> {
    match halt {
        TransferHalt::Exhausted { failed, .. } => {
            if job.status() == crate::domain::job::JobStatus::RunningCanary {
                job.complete_canary()
            } else {
                job.complete_transfer(iso_now(), *failed > 0)
            }
        }
        TransferHalt::SharingRateLimited { message } => {
            job.pause_sharing_rate_limit(message.clone())
        }
        TransferHalt::WaitingForQuota { message } => job.wait_for_quota(message.clone()),
        TransferHalt::AuthRequired { message } => job.require_auth(message.clone()),
        TransferHalt::Paused => job.pause_transfer(),
        TransferHalt::Cancelled => job.cancel_job(iso_now()),
    }
}

fn stop_requested(run: &TransferRun<'_>) -> Option<TransferHalt> {
    if run.cancel.is_some_and(|flag| flag.load(Ordering::SeqCst)) {
        return Some(TransferHalt::Cancelled);
    }
    if run.pause.is_some_and(|flag| flag.load(Ordering::SeqCst)) {
        return Some(TransferHalt::Paused);
    }
    None
}

pub async fn transfer_items(
    run: &TransferRun<'_>,
    items: &[MigrationItem],
) -> Result<TransferHalt, TransferError> {
    let mut verified = 0;
    let mut failed = 0;
    for item in items {
        if let Some(halt) = stop_requested(run) {
            return Ok(halt);
        }
        match transfer_one(run, item.clone()).await {
            Ok(ItemState::Verified) => verified += 1,
            Ok(_) => {}
            Err(StepError::Halt(halt)) => return Ok(halt),
            Err(StepError::Failed) => failed += 1,
            Err(StepError::Fatal(err)) => return Err(err),
        }
        emit_transfer_progress(run, Some(item.name.clone())).await?;
    }
    Ok(TransferHalt::Exhausted { verified, failed })
}

enum StepError {
    Halt(TransferHalt),
    Failed,
    Fatal(TransferError),
}

impl StepError {
    fn into_transfer(self) -> TransferError {
        match self {
            Self::Fatal(err) => err,
            Self::Halt(_) | Self::Failed => TransferError::InvalidItemState,
        }
    }
}

enum StepOutcome {
    Retryable,
    Permanent,
    Halt(TransferHalt),
    Store(ItemStoreError),
    InvalidState,
}

enum PrepareAction {
    Done(ItemState),
    VerifyOnly,
    Accept { permission_id: String },
}

async fn transfer_one(
    run: &TransferRun<'_>,
    mut item: MigrationItem,
) -> Result<ItemState, StepError> {
    if item.state.is_terminal() {
        return Ok(item.state);
    }

    match reconcile_and_prepare(run, &mut item).await {
        Ok(PrepareAction::Done(state)) => return Ok(state),
        Ok(PrepareAction::VerifyOnly) => {}
        Ok(PrepareAction::Accept { permission_id }) => {
            if let Err(err) = accept_ownership(run, &mut item, &permission_id).await {
                return finalize_step(run, &mut item, err).await;
            }
        }
        Err(err) => return finalize_step(run, &mut item, err).await,
    }

    match verify_ownership(run, &mut item).await {
        Ok(()) => {
            if item.state.is_terminal() {
                return Ok(item.state);
            }
            apply_state(&mut item, ItemState::Verified)?;
            persist(run, &item).await?;
            Ok(ItemState::Verified)
        }
        Err(err) => finalize_step(run, &mut item, err).await,
    }
}

async fn reconcile_and_prepare(
    run: &TransferRun<'_>,
    item: &mut MigrationItem,
) -> Result<PrepareAction, StepOutcome> {
    if matches!(item.state, ItemState::Transferred | ItemState::Verifying) {
        apply_state_outcome(item, ItemState::Verifying)?;
        persist_outcome(run, item).await?;
        return Ok(PrepareAction::VerifyOnly);
    }
    if matches!(item.state, ItemState::AcceptRequired | ItemState::Accepting) {
        let permission_id = item
            .target_permission_id
            .as_ref()
            .map(|id| id.as_str().to_string())
            .ok_or(StepOutcome::Permanent)?;
        return Ok(PrepareAction::Accept { permission_id });
    }

    let snapshot = retry(run, TransferPhase::Reconcile, || {
        run.drive.get_file(run.source_token, &item.file_id)
    })
    .await?;

    if snapshot.trashed {
        apply_state_outcome(item, ItemState::SkippedTrashed)?;
        persist_outcome(run, item).await?;
        return Ok(PrepareAction::Done(ItemState::SkippedTrashed));
    }
    if snapshot.drive_id.is_some() {
        apply_state_outcome(item, ItemState::SkippedSharedDrive)?;
        persist_outcome(run, item).await?;
        return Ok(PrepareAction::Done(ItemState::SkippedSharedDrive));
    }

    let target_owns = is_owner(&snapshot.owners, run.target_permission_id);
    let source_owns = is_owner(&snapshot.owners, run.source_permission_id);
    if target_owns && !source_owns {
        apply_state_outcome(item, ItemState::Verifying)?;
        persist_outcome(run, item).await?;
        return Ok(PrepareAction::VerifyOnly);
    }
    if !source_owns {
        apply_state_outcome(item, ItemState::PermanentFailed)?;
        persist_outcome(run, item).await?;
        return Err(StepOutcome::Permanent);
    }

    if let Some(existing) = find_target_permission(
        &snapshot.permissions,
        run.target_email,
        run.target_permission_id,
    ) {
        item.target_permission_id = Some(GooglePermissionId::new(existing.id.clone()));
        if existing.pending_owner || existing.role.eq_ignore_ascii_case("owner") {
            advance_to_accept_required(item)?;
            persist_outcome(run, item).await?;
            return Ok(PrepareAction::Accept {
                permission_id: existing.id.clone(),
            });
        }

        apply_state_outcome(item, ItemState::PendingOwnerRequired)?;
        persist_outcome(run, item).await?;
        let updated = retry(run, TransferPhase::PendingOwner, || {
            run.drive
                .update_pending_owner(run.source_token, &item.file_id, &existing.id)
        })
        .await?;
        item.target_permission_id = Some(GooglePermissionId::new(updated.id.clone()));
        advance_to_accept_required(item)?;
        persist_outcome(run, item).await?;
        return Ok(PrepareAction::Accept {
            permission_id: updated.id,
        });
    }

    apply_state_outcome(item, ItemState::PendingOwnerRequired)?;
    persist_outcome(run, item).await?;
    let created = retry(run, TransferPhase::PendingOwner, || {
        run.drive
            .create_pending_owner(run.source_token, &item.file_id, run.target_email)
    })
    .await?;
    item.target_permission_id = Some(GooglePermissionId::new(created.id.clone()));
    advance_to_accept_required(item)?;
    persist_outcome(run, item).await?;
    Ok(PrepareAction::Accept {
        permission_id: created.id,
    })
}

fn advance_to_accept_required(item: &mut MigrationItem) -> Result<(), StepOutcome> {
    match item.state {
        ItemState::Eligible | ItemState::RetryableFailed => {
            apply_state_outcome(item, ItemState::PendingOwnerRequired)?;
            apply_state_outcome(item, ItemState::PendingOwnerCreated)?;
            apply_state_outcome(item, ItemState::AcceptRequired)
        }
        ItemState::PendingOwnerRequired => {
            apply_state_outcome(item, ItemState::PendingOwnerCreated)?;
            apply_state_outcome(item, ItemState::AcceptRequired)
        }
        ItemState::PendingOwnerCreated => apply_state_outcome(item, ItemState::AcceptRequired),
        ItemState::AcceptRequired | ItemState::Accepting => Ok(()),
        _ => Err(StepOutcome::InvalidState),
    }
}

async fn accept_ownership(
    run: &TransferRun<'_>,
    item: &mut MigrationItem,
    permission_id: &str,
) -> Result<(), StepOutcome> {
    apply_state_outcome(item, ItemState::Accepting)?;
    persist_outcome(run, item).await?;
    retry(run, TransferPhase::Accept, || {
        run.drive
            .accept_ownership(run.target_token, &item.file_id, permission_id)
    })
    .await?;
    apply_state_outcome(item, ItemState::Transferred)?;
    persist_outcome(run, item).await?;
    Ok(())
}

async fn verify_ownership(
    run: &TransferRun<'_>,
    item: &mut MigrationItem,
) -> Result<(), StepOutcome> {
    apply_state_outcome(item, ItemState::Verifying)?;
    persist_outcome(run, item).await?;

    let mut attempt = 0;
    loop {
        let snapshot = retry(run, TransferPhase::Verify, || {
            run.drive.get_file(run.target_token, &item.file_id)
        })
        .await?;

        if snapshot.trashed {
            apply_state_outcome(item, ItemState::SkippedTrashed)?;
            persist_outcome(run, item).await?;
            return Ok(());
        }

        let target_owns = is_owner(&snapshot.owners, run.target_permission_id);
        let source_owns = is_owner(&snapshot.owners, run.source_permission_id);
        let parents_ok = same_parents(&item.original_parent_ids, &snapshot.parents);
        if target_owns && !source_owns && parents_ok {
            return Ok(());
        }

        if !target_owns && !source_owns {
            apply_state_outcome(item, ItemState::PermanentFailed)?;
            persist_outcome(run, item).await?;
            return Err(StepOutcome::Permanent);
        }

        if attempt >= MAX_RETRY_ATTEMPTS {
            return Err(StepOutcome::Retryable);
        }
        run.sleeper
            .sleep(backoff_delay(attempt, run.jitter.jitter_secs()))
            .await;
        attempt += 1;
    }
}

async fn retry<T, F, Fut>(
    run: &TransferRun<'_>,
    phase: TransferPhase,
    mut op: F,
) -> Result<T, StepOutcome>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, DriveTransferError>>,
{
    let mut attempt = 0;
    loop {
        match op().await {
            Ok(value) => return Ok(value),
            Err(err) => match classify(err, phase) {
                StepOutcome::Retryable if attempt < MAX_RETRY_ATTEMPTS => {
                    run.sleeper
                        .sleep(backoff_delay(attempt, run.jitter.jitter_secs()))
                        .await;
                    attempt += 1;
                }
                other => return Err(other),
            },
        }
    }
}

fn classify(err: DriveTransferError, phase: TransferPhase) -> StepOutcome {
    match err {
        DriveTransferError::SharingRateLimitExceeded => {
            StepOutcome::Halt(TransferHalt::SharingRateLimited {
                message: err.to_string(),
            })
        }
        DriveTransferError::StorageQuotaExceeded => {
            StepOutcome::Halt(TransferHalt::WaitingForQuota {
                message: err.to_string(),
            })
        }
        DriveTransferError::Unauthorized => StepOutcome::Halt(TransferHalt::AuthRequired {
            message: err.to_string(),
        }),
        DriveTransferError::NotFound => match phase {
            TransferPhase::Verify | TransferPhase::Accept => StepOutcome::Retryable,
            TransferPhase::Reconcile | TransferPhase::PendingOwner => StepOutcome::Permanent,
        },
        DriveTransferError::RateLimited
        | DriveTransferError::ServerUnavailable
        | DriveTransferError::Transport => StepOutcome::Retryable,
        DriveTransferError::Forbidden
        | DriveTransferError::InvalidResponse
        | DriveTransferError::UnexpectedStatus(_) => StepOutcome::Permanent,
    }
}

fn apply_state(item: &mut MigrationItem, next: ItemState) -> Result<(), StepError> {
    item.state = item
        .state
        .transition_to(next)
        .map_err(|_| StepError::Fatal(TransferError::InvalidItemState))?;
    item.updated_at = iso_now();
    Ok(())
}

fn apply_state_outcome(item: &mut MigrationItem, next: ItemState) -> Result<(), StepOutcome> {
    item.state = item
        .state
        .transition_to(next)
        .map_err(|_err: ItemError| StepOutcome::InvalidState)?;
    item.updated_at = iso_now();
    Ok(())
}

async fn persist(run: &TransferRun<'_>, item: &MigrationItem) -> Result<(), StepError> {
    run.store
        .save_item(item)
        .await
        .map_err(|err| StepError::Fatal(TransferError::Store(err)))?;
    emit_item_state(run, item);
    Ok(())
}

async fn persist_outcome(run: &TransferRun<'_>, item: &MigrationItem) -> Result<(), StepOutcome> {
    run.store
        .save_item(item)
        .await
        .map_err(StepOutcome::Store)?;
    emit_item_state(run, item);
    Ok(())
}

fn emit_item_state(run: &TransferRun<'_>, item: &MigrationItem) {
    let Some(events) = run.events else {
        return;
    };
    events.emit(JobRuntimeEvent::ItemStateChanged {
        job_id: run.job_id,
        item_id: item.file_id.clone(),
        state: item.state.as_str().to_string(),
    });
}

async fn emit_transfer_progress(
    run: &TransferRun<'_>,
    current_path: Option<String>,
) -> Result<(), TransferError> {
    let Some(events) = run.events else {
        return Ok(());
    };
    let aggregates = run.store.item_aggregates(run.job_id).await?;
    events.emit(JobRuntimeEvent::MigrationProgress {
        job_id: run.job_id,
        completed: aggregates.completed,
        total: aggregates.total,
        current_path,
    });
    Ok(())
}

async fn finalize_step(
    run: &TransferRun<'_>,
    item: &mut MigrationItem,
    err: StepOutcome,
) -> Result<ItemState, StepError> {
    match err {
        StepOutcome::Halt(halt) => Err(StepError::Halt(halt)),
        StepOutcome::Store(err) => Err(StepError::Fatal(TransferError::Store(err))),
        StepOutcome::InvalidState => Err(StepError::Fatal(TransferError::InvalidItemState)),
        StepOutcome::Permanent => {
            if !item.state.is_terminal() {
                apply_state(item, ItemState::PermanentFailed)?;
                persist(run, item).await?;
            }
            Err(StepError::Failed)
        }
        StepOutcome::Retryable => {
            if !item.state.is_terminal() {
                apply_state(item, ItemState::RetryableFailed)?;
                persist(run, item).await?;
            }
            Err(StepError::Failed)
        }
    }
}

fn is_owner(owners: &[DriveFolderOwner], permission_id: &GooglePermissionId) -> bool {
    owners
        .iter()
        .any(|owner| owner.permission_id.as_str() == permission_id.as_str())
}

fn find_target_permission<'a>(
    permissions: &'a [DrivePermission],
    target_email: &str,
    target_permission_id: &GooglePermissionId,
) -> Option<&'a DrivePermission> {
    permissions.iter().find(|permission| {
        permission.id == target_permission_id.as_str()
            || permission
                .email_address
                .as_deref()
                .is_some_and(|email| email.eq_ignore_ascii_case(target_email))
    })
}

fn same_parents(original: &[String], remote: &[String]) -> bool {
    let mut left = original.to_vec();
    let mut right = remote.to_vec();
    left.sort();
    right.sort();
    left == right
}

pub async fn reconcile_checkpoint(
    run: &TransferRun<'_>,
    items: &[MigrationItem],
) -> Result<(), TransferError> {
    for item in items {
        if !item.state.is_intermediate_checkpoint() {
            continue;
        }
        let mut item = item.clone();
        reconcile_intermediate_readonly(run, &mut item).await?;
    }
    Ok(())
}

async fn reconcile_intermediate_readonly(
    run: &TransferRun<'_>,
    item: &mut MigrationItem,
) -> Result<(), TransferError> {
    let token = match item.state {
        ItemState::Transferred | ItemState::Verifying => run.target_token,
        _ => run.source_token,
    };
    let snapshot = match run.drive.get_file(token, &item.file_id).await {
        Ok(snapshot) => snapshot,
        Err(DriveTransferError::NotFound) => {
            apply_state(item, ItemState::PermanentFailed).map_err(StepError::into_transfer)?;
            persist(run, item).await.map_err(StepError::into_transfer)?;
            return Ok(());
        }
        Err(err) => return Err(TransferError::Drive(err)),
    };

    if snapshot.trashed {
        apply_state(item, ItemState::SkippedTrashed).map_err(StepError::into_transfer)?;
        persist(run, item).await.map_err(StepError::into_transfer)?;
        return Ok(());
    }

    let target_owns = is_owner(&snapshot.owners, run.target_permission_id);
    let source_owns = is_owner(&snapshot.owners, run.source_permission_id);
    if target_owns && !source_owns {
        let next = match item.state {
            ItemState::PendingOwnerCreated | ItemState::Accepting => ItemState::Verifying,
            ItemState::Transferred | ItemState::Verifying => item.state,
            other => other,
        };
        if next != item.state {
            apply_state(item, next).map_err(StepError::into_transfer)?;
        }
        persist(run, item).await.map_err(StepError::into_transfer)?;
        return Ok(());
    }

    if let Some(existing) = find_target_permission(
        &snapshot.permissions,
        run.target_email,
        run.target_permission_id,
    ) {
        item.target_permission_id = Some(GooglePermissionId::new(existing.id.clone()));
        if existing.pending_owner && item.state == ItemState::PendingOwnerCreated {
            apply_state(item, ItemState::AcceptRequired).map_err(StepError::into_transfer)?;
        }
        persist(run, item).await.map_err(StepError::into_transfer)?;
        return Ok(());
    }

    persist(run, item).await.map_err(StepError::into_transfer)?;
    Ok(())
}
