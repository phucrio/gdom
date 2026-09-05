use crate::application::AccessToken;
use crate::application::backoff::{JitterSource, MAX_RETRY_ATTEMPTS, Sleeper, backoff_delay};
use crate::application::drive_folder::DriveFolderOwner;
use crate::application::drive_transfer::{DrivePermission, DriveTransferError, DriveTransferPort};
use crate::application::item_store::{ItemStoreError, ItemStorePort};
use crate::application::time::iso_now;
use crate::domain::GooglePermissionId;
use crate::domain::item::{ItemError, ItemState, MigrationItem};
use crate::domain::job::{JobError, MigrationJob};

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
    apply_halt(job, &halt)?;
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
    }
}

pub async fn transfer_items(
    run: &TransferRun<'_>,
    items: &[MigrationItem],
) -> Result<TransferHalt, TransferError> {
    let mut verified = 0;
    let mut failed = 0;
    for item in items {
        match transfer_one(run, item.clone()).await {
            Ok(ItemState::Verified) => verified += 1,
            Ok(_) => {}
            Err(StepError::Halt(halt)) => return Ok(halt),
            Err(StepError::Failed) => failed += 1,
            Err(StepError::Fatal(err)) => return Err(err),
        }
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

    if matches!(item.state, ItemState::Transferred | ItemState::Verifying) {
        apply_state_outcome(item, ItemState::Verifying)?;
        persist_outcome(run, item).await?;
        return Ok(PrepareAction::VerifyOnly);
    }

    if let Some(existing) = find_target_permission(
        &snapshot.permissions,
        run.target_email,
        run.target_permission_id,
    ) {
        item.target_permission_id = Some(GooglePermissionId::new(existing.id.clone()));
        if matches!(item.state, ItemState::AcceptRequired | ItemState::Accepting) {
            persist_outcome(run, item).await?;
            return Ok(PrepareAction::Accept {
                permission_id: existing.id.clone(),
            });
        }
        if existing.pending_owner || existing.role.eq_ignore_ascii_case("owner") {
            walk_to_accept_required(item)?;
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
        walk_to_accept_required(item)?;
        persist_outcome(run, item).await?;
        return Ok(PrepareAction::Accept {
            permission_id: updated.id,
        });
    }

    if matches!(item.state, ItemState::AcceptRequired | ItemState::Accepting) {
        let permission_id = item
            .target_permission_id
            .as_ref()
            .map(|id| id.as_str().to_string())
            .ok_or(StepOutcome::Permanent)?;
        return Ok(PrepareAction::Accept { permission_id });
    }

    apply_state_outcome(item, ItemState::PendingOwnerRequired)?;
    persist_outcome(run, item).await?;
    let created = retry(run, TransferPhase::PendingOwner, || {
        run.drive
            .create_pending_owner(run.source_token, &item.file_id, run.target_email)
    })
    .await?;
    item.target_permission_id = Some(GooglePermissionId::new(created.id.clone()));
    walk_to_accept_required(item)?;
    persist_outcome(run, item).await?;
    Ok(PrepareAction::Accept {
        permission_id: created.id,
    })
}

fn walk_to_accept_required(item: &mut MigrationItem) -> Result<(), StepOutcome> {
    for next in [
        ItemState::PendingOwnerRequired,
        ItemState::PendingOwnerCreated,
        ItemState::AcceptRequired,
    ] {
        if item.state == next || item.state == ItemState::Accepting {
            continue;
        }
        if item.state.can_transition_to(next) {
            apply_state_outcome(item, next)?;
        }
    }
    Ok(())
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
        .map_err(|err| StepError::Fatal(TransferError::Store(err)))
}

async fn persist_outcome(run: &TransferRun<'_>, item: &MigrationItem) -> Result<(), StepOutcome> {
    run.store.save_item(item).await.map_err(StepOutcome::Store)
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
