use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::application::account_token_provider::AccountTokenProvider;
use crate::application::backoff::{JitterSource, Sleeper, SystemJitter, TokioSleeper};
use crate::application::connect_account::AccountStorePort;
use crate::application::drive_folder::{DriveFolderLookupError, DriveFolderMetadata};
use crate::application::drive_transfer::DriveTransferPort;
use crate::application::drive_tree::{DEFAULT_SCAN_CONCURRENCY, DrivePort};
use crate::application::entity_id::next_entity_id;
use crate::application::item_store::{ItemPage, ItemStoreError, ItemStorePort};
use crate::application::job_events::{JobEventSink, JobRuntimeEvent, NoopJobEventSink};
use crate::application::job_store::{JobStorePort, JobStorePortError, MigrationEvent};
use crate::application::preflight::{
    DryRunCsvRow, PreflightSummary, destination_is_csv, render_items_csv,
};
use crate::application::root_parser::{RootParseError, parse_root_input};
use crate::application::scanner::{ScanError, ScanOutcome, ScanRun, run_scan};
use crate::application::time::iso_now;
use crate::application::transfer::{
    TransferError, TransferHalt, TransferRun, execute_bulk, execute_canary,
};
use crate::domain::job::{
    AccountSnapshot, JobError, JobId, JobStatus, MigrationJob, MigrationRoot, RootId,
    RootValidationStatus,
};
use crate::domain::{AccountId, AuthStatus};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JobServiceError {
    SameSourceAndTarget,
    SourceAccountNotFound(AccountId),
    TargetAccountNotFound(AccountId),
    AccountNotActive(AccountId),
    JobNotFound(JobId),
    RootNotFound(RootId),
    DuplicateRoot(String),
    AccountPairLocked,
    RootsLocked,
    ParseError(RootParseError),
    TokenError(String),
    DriveError(String),
    FolderNotFound,
    NotAFolder,
    FolderTrashed,
    SharedDriveNotSupported,
    NotOwnedBySourceAccount,
    StoreError(String),
    NoValidatedRoots,
    IllegalTransition,
    RateLimited,
    ExportFailed(String),
    ScanInProgress,
    ConfirmationMismatch,
    TransferInProgress,
    SharingRateLimited,
    WaitingForQuota,
    AuthRequired,
}

impl fmt::Display for JobServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SameSourceAndTarget => write!(f, "source and target account cannot be identical"),
            Self::SourceAccountNotFound(id) => {
                write!(f, "source account not found: {}", id.value())
            }
            Self::TargetAccountNotFound(id) => {
                write!(f, "target account not found: {}", id.value())
            }
            Self::AccountNotActive(id) => {
                write!(f, "account is not active or connected: {}", id.value())
            }
            Self::JobNotFound(id) => write!(f, "job not found: {id}"),
            Self::RootNotFound(id) => write!(f, "root not found: {id}"),
            Self::DuplicateRoot(file_id) => {
                write!(f, "root folder already added to job: {file_id}")
            }
            Self::AccountPairLocked => {
                write!(f, "account pair cannot be changed after draft status")
            }
            Self::RootsLocked => write!(f, "roots cannot be changed after draft status"),
            Self::ParseError(e) => write!(f, "invalid folder URL or ID: {e}"),
            Self::TokenError(e) => write!(f, "failed to obtain OAuth token: {e}"),
            Self::DriveError(e) => write!(f, "Google Drive API error: {e}"),
            Self::FolderNotFound => write!(f, "folder not found on Google Drive"),
            Self::NotAFolder => write!(f, "selected item is not a folder"),
            Self::FolderTrashed => write!(f, "folder is in Google Drive trash"),
            Self::SharedDriveNotSupported => {
                write!(f, "shared Drive folders cannot be migration roots")
            }
            Self::NotOwnedBySourceAccount => {
                write!(f, "folder is not owned by the selected source account")
            }
            Self::StoreError(e) => write!(f, "persistence error: {e}"),
            Self::NoValidatedRoots => write!(f, "scan requires at least one validated root"),
            Self::IllegalTransition => write!(f, "illegal job status transition"),
            Self::RateLimited => write!(f, "Google Drive rate limit reached"),
            Self::ExportFailed(e) => write!(f, "failed to export dry-run report: {e}"),
            Self::ScanInProgress => write!(f, "a scan is already running for this job"),
            Self::ConfirmationMismatch => {
                write!(f, "target email confirmation does not match the job target")
            }
            Self::TransferInProgress => {
                write!(f, "another migration job is already mutating ownership")
            }
            Self::SharingRateLimited => {
                write!(f, "Google Drive sharing rate limit reached")
            }
            Self::WaitingForQuota => write!(f, "Google Drive storage quota exceeded"),
            Self::AuthRequired => write!(f, "an account needs to be re-authenticated"),
        }
    }
}

impl Error for JobServiceError {}

impl From<JobError> for JobServiceError {
    fn from(err: JobError) -> Self {
        match err {
            JobError::SameSourceAndTarget => Self::SameSourceAndTarget,
            JobError::AccountPairLocked => Self::AccountPairLocked,
            JobError::RootsLocked => Self::RootsLocked,
            JobError::DuplicateRoot(file_id) => Self::DuplicateRoot(file_id),
            JobError::RootNotFound(id) => Self::RootNotFound(id),
            JobError::InvalidJobStatus => Self::StoreError("invalid job status".to_string()),
            JobError::InvalidRootValidationStatus => {
                Self::StoreError("invalid root validation status".to_string())
            }
            JobError::IllegalTransition => Self::IllegalTransition,
            JobError::NoValidatedRoots => Self::NoValidatedRoots,
        }
    }
}

impl From<ItemStoreError> for JobServiceError {
    fn from(err: ItemStoreError) -> Self {
        Self::StoreError(err.to_string())
    }
}

impl From<ScanError> for JobServiceError {
    fn from(err: ScanError) -> Self {
        match err {
            ScanError::RateLimited => Self::RateLimited,
            ScanError::Drive(e) => Self::DriveError(e.to_string()),
            ScanError::Store(e) => Self::StoreError(e.to_string()),
        }
    }
}

impl From<TransferError> for JobServiceError {
    fn from(err: TransferError) -> Self {
        match err {
            TransferError::Drive(drive_err) => match drive_err {
                crate::application::drive_transfer::DriveTransferError::SharingRateLimitExceeded => {
                    Self::SharingRateLimited
                }
                crate::application::drive_transfer::DriveTransferError::StorageQuotaExceeded => {
                    Self::WaitingForQuota
                }
                crate::application::drive_transfer::DriveTransferError::Unauthorized => {
                    Self::AuthRequired
                }
                crate::application::drive_transfer::DriveTransferError::RateLimited => {
                    Self::RateLimited
                }
                other => Self::DriveError(other.to_string()),
            },
            TransferError::Store(err) => Self::StoreError(err.to_string()),
            TransferError::Job(err) => err.into(),
            TransferError::InvalidItemState => {
                Self::StoreError("illegal item state transition during transfer".to_string())
            }
        }
    }
}

impl From<JobStorePortError> for JobServiceError {
    fn from(err: JobStorePortError) -> Self {
        match err {
            JobStorePortError::JobNotFound(id) => Self::JobNotFound(id),
            JobStorePortError::RootNotFound(id) => Self::RootNotFound(id),
            JobStorePortError::SameSourceAndTarget => Self::SameSourceAndTarget,
            JobStorePortError::AccountNotFound(id) => Self::SourceAccountNotFound(id),
            JobStorePortError::AccountHasActiveJobs(id) => {
                Self::StoreError(format!("account has active jobs: {}", id.value()))
            }
            JobStorePortError::DuplicateRoot(file_id) => Self::DuplicateRoot(file_id),
            JobStorePortError::AccountPairLocked => Self::AccountPairLocked,
            JobStorePortError::RootsLocked => Self::RootsLocked,
            JobStorePortError::Database(msg) => Self::StoreError(msg),
            JobStorePortError::MutationLeaseHeld => Self::TransferInProgress,
            JobStorePortError::NotDraftJob(_) => Self::IllegalTransition,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransferProgress {
    pub completed: u64,
    pub failed: u64,
    pub skipped: u64,
    pub total: u64,
    pub current_path: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ItemFailure {
    pub item_id: String,
    pub message: String,
    pub at: String,
}

struct ScanInFlightGuard {
    slots: Arc<std::sync::Mutex<HashSet<JobId>>>,
    job_id: JobId,
}

impl Drop for ScanInFlightGuard {
    fn drop(&mut self) {
        let mut slots = self
            .slots
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        slots.remove(&self.job_id);
    }
}

struct TransferLeaseGuard {
    slot: Arc<std::sync::Mutex<Option<JobId>>>,
    job_id: JobId,
}

fn try_acquire_transfer_slot(
    slot: &Arc<std::sync::Mutex<Option<JobId>>>,
    job_id: JobId,
) -> Result<TransferLeaseGuard, JobServiceError> {
    let mut lease = slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if lease.is_some() {
        return Err(JobServiceError::TransferInProgress);
    }
    *lease = Some(job_id);
    drop(lease);
    Ok(TransferLeaseGuard {
        slot: Arc::clone(slot),
        job_id,
    })
}

impl Drop for TransferLeaseGuard {
    fn drop(&mut self) {
        let mut lease = self
            .slot
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if *lease == Some(self.job_id) {
            *lease = None;
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountJobRole {
    Source,
    Target,
}

impl AccountJobRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Target => "target",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobRunPhase {
    Scan,
    Canary,
    Bulk,
}

impl JobRunPhase {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Scan => "scan",
            Self::Canary => "canary",
            Self::Bulk => "bulk",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountJobReference {
    pub job_id: JobId,
    pub status: JobStatus,
    pub role: AccountJobRole,
}

impl AccountJobReference {
    fn from_job(job: &crate::domain::job::MigrationJob, account_id: AccountId) -> Option<Self> {
        let role = if job.source_account_id() == account_id {
            AccountJobRole::Source
        } else if job.target_account_id() == account_id {
            AccountJobRole::Target
        } else {
            return None;
        };
        Some(Self {
            job_id: job.id(),
            status: job.status(),
            role,
        })
    }
}

fn job_matches_list_filter(
    job: &crate::domain::job::MigrationJob,
    status: Option<JobStatus>,
    account_id: Option<AccountId>,
) -> bool {
    let status_ok = match status {
        Some(expected) => job.status() == expected,
        None => true,
    };
    let account_ok = match account_id {
        Some(id) => job.source_account_id() == id || job.target_account_id() == id,
        None => true,
    };
    status_ok && account_ok
}

pub struct JobService<A, J>
where
    A: AccountStorePort + Send + Sync + 'static,
    J: JobStorePort + ItemStorePort + 'static,
{
    account_store: Arc<A>,
    job_store: Arc<J>,
    drive: Arc<dyn DrivePort>,
    token_provider: Arc<AccountTokenProvider<A>>,
    scan_pause_flags: Arc<tokio::sync::Mutex<HashMap<JobId, Arc<AtomicBool>>>>,
    scan_in_flight: Arc<std::sync::Mutex<HashSet<JobId>>>,
    transfer_lease: Arc<std::sync::Mutex<Option<JobId>>>,
    transfer_pause_flags: Arc<tokio::sync::Mutex<HashMap<JobId, Arc<AtomicBool>>>>,
    transfer_cancel_flags: Arc<tokio::sync::Mutex<HashMap<JobId, Arc<AtomicBool>>>>,
    sleeper: Arc<dyn Sleeper>,
    jitter: Arc<dyn JitterSource>,
    instance_id: String,
    events: Arc<dyn JobEventSink>,
}

impl<A, J> Clone for JobService<A, J>
where
    A: AccountStorePort + Send + Sync + 'static,
    J: JobStorePort + ItemStorePort + 'static,
{
    fn clone(&self) -> Self {
        Self {
            account_store: Arc::clone(&self.account_store),
            job_store: Arc::clone(&self.job_store),
            drive: Arc::clone(&self.drive),
            token_provider: Arc::clone(&self.token_provider),
            scan_pause_flags: Arc::clone(&self.scan_pause_flags),
            scan_in_flight: Arc::clone(&self.scan_in_flight),
            transfer_lease: Arc::clone(&self.transfer_lease),
            transfer_pause_flags: Arc::clone(&self.transfer_pause_flags),
            transfer_cancel_flags: Arc::clone(&self.transfer_cancel_flags),
            sleeper: Arc::clone(&self.sleeper),
            jitter: Arc::clone(&self.jitter),
            instance_id: self.instance_id.clone(),
            events: Arc::clone(&self.events),
        }
    }
}

impl<A, J> JobService<A, J>
where
    A: AccountStorePort + Send + Sync + 'static,
    J: JobStorePort + ItemStorePort + 'static,
{
    pub fn new(
        account_store: Arc<A>,
        job_store: Arc<J>,
        drive: Arc<dyn DrivePort>,
        token_provider: Arc<AccountTokenProvider<A>>,
    ) -> Self {
        Self {
            account_store,
            job_store,
            drive,
            token_provider,
            scan_pause_flags: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            scan_in_flight: Arc::new(std::sync::Mutex::new(HashSet::new())),
            transfer_lease: Arc::new(std::sync::Mutex::new(None)),
            transfer_pause_flags: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            transfer_cancel_flags: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            sleeper: Arc::new(TokioSleeper),
            jitter: Arc::new(SystemJitter),
            instance_id: format!("gdom-{}", next_entity_id()),
            events: Arc::new(NoopJobEventSink),
        }
    }

    pub fn with_sleeper(
        mut self,
        sleeper: Arc<dyn Sleeper>,
        jitter: Arc<dyn JitterSource>,
    ) -> Self {
        self.sleeper = sleeper;
        self.jitter = jitter;
        self
    }

    pub fn with_event_sink(mut self, events: Arc<dyn JobEventSink>) -> Self {
        self.events = events;
        self
    }

    pub async fn await_idle(&self, job_id: JobId) {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(15);
        loop {
            let lease_held = self.scan_is_in_flight(job_id) || self.transfer_is_in_flight(job_id);
            let status_busy = self.get_job(job_id).await.ok().is_some_and(|job| {
                matches!(
                    job.status(),
                    JobStatus::Scanning
                        | JobStatus::RunningCanary
                        | JobStatus::Running
                        | JobStatus::Pausing
                        | JobStatus::Cancelling
                )
            });
            if !lease_held && !status_busy {
                return;
            }
            if tokio::time::Instant::now() >= deadline {
                panic!("job {job_id} worker still in flight after 15s");
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    }

    fn emit(&self, event: JobRuntimeEvent) {
        self.events.emit(event);
    }

    fn emit_status(&self, job: &crate::domain::job::MigrationJob) {
        let status = job.status();
        self.emit(JobRuntimeEvent::JobStatusChanged {
            job_id: job.id(),
            status: status.as_str().to_string(),
        });
        match status {
            JobStatus::CanaryReview => {
                self.emit(JobRuntimeEvent::CanaryCompleted { job_id: job.id() })
            }
            JobStatus::Completed | JobStatus::CompletedWithErrors => {
                self.emit(JobRuntimeEvent::MigrationCompleted { job_id: job.id() });
            }
            _ => {}
        }
    }

    fn try_acquire_scan(&self, job_id: JobId) -> Result<ScanInFlightGuard, JobServiceError> {
        let mut slots = self
            .scan_in_flight
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !slots.insert(job_id) {
            return Err(JobServiceError::ScanInProgress);
        }
        drop(slots);
        Ok(ScanInFlightGuard {
            slots: Arc::clone(&self.scan_in_flight),
            job_id,
        })
    }

    fn scan_is_in_flight(&self, job_id: JobId) -> bool {
        self.scan_in_flight
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .contains(&job_id)
    }

    async fn try_acquire_transfer(
        &self,
        job_id: JobId,
    ) -> Result<TransferLeaseGuard, JobServiceError> {
        let guard = try_acquire_transfer_slot(&self.transfer_lease, job_id)?;
        let now = iso_now();
        match self
            .job_store
            .acquire_mutation_lease(job_id, &self.instance_id, &now)
            .await
        {
            Ok(()) => Ok(guard),
            Err(err) => Err(err.into()),
        }
    }

    async fn release_durable_lease(&self, job_id: JobId) {
        let _ = self
            .job_store
            .release_mutation_lease(job_id, &self.instance_id)
            .await;
    }

    fn transfer_is_in_flight(&self, job_id: JobId) -> bool {
        *self
            .transfer_lease
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            == Some(job_id)
    }

    fn clear_memory_lease(&self, job_id: JobId) {
        let mut slot = self
            .transfer_lease
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if *slot == Some(job_id) {
            *slot = None;
        }
    }

    fn status_event(
        job: &crate::domain::job::MigrationJob,
        previous: Option<&str>,
        event_type: &str,
    ) -> MigrationEvent {
        MigrationEvent {
            id: next_entity_id().to_string(),
            job_id: job.id(),
            file_id: None,
            account_id: None,
            event_type: event_type.to_string(),
            previous_state: previous.map(ToOwned::to_owned),
            new_state: Some(job.status().as_str().to_string()),
            sanitized_detail_json: None,
            created_at: iso_now(),
        }
    }

    async fn persist_status(
        &self,
        job: &crate::domain::job::MigrationJob,
        previous: Option<&str>,
        event_type: &str,
    ) -> Result<(), JobServiceError> {
        let event = Self::status_event(job, previous, event_type);
        self.job_store.persist_job_with_event(job, &event).await?;
        self.emit_status(job);
        Ok(())
    }

    pub async fn live_progress(
        &self,
        job_id: JobId,
    ) -> Result<(Option<TransferProgress>, Vec<ItemFailure>), JobServiceError> {
        let aggregates = self.job_store.item_aggregates(job_id).await?;
        let items = self.job_store.list_items_for_transfer(job_id).await?;
        let progress = if aggregates.total == 0 {
            None
        } else {
            let current_path = items
                .iter()
                .find(|item| item.state.is_transfer_active() && !item.state.is_eligible())
                .or_else(|| items.iter().find(|item| item.state.is_eligible()))
                .map(|item| item.name.clone());
            Some(TransferProgress {
                completed: aggregates.completed,
                failed: aggregates.failed,
                skipped: aggregates.skipped,
                total: aggregates.total,
                current_path,
            })
        };
        let mut errors: Vec<ItemFailure> = items
            .iter()
            .filter(|item| item.state == crate::domain::item::ItemState::RetryableFailed)
            .map(|item| ItemFailure {
                item_id: item.file_id.clone(),
                message: item.state.as_str().to_string(),
                at: item.updated_at.clone(),
            })
            .collect();
        if let Ok(job) = self.get_job(job_id).await
            && let Some(message) = job.last_error()
        {
            errors.insert(
                0,
                ItemFailure {
                    item_id: "job".to_string(),
                    message: message.to_string(),
                    at: job
                        .completed_at()
                        .or(job.started_at())
                        .unwrap_or(job.created_at())
                        .to_string(),
                },
            );
        }
        Ok((progress, errors))
    }

    async fn set_control_flag(
        flags: &tokio::sync::Mutex<HashMap<JobId, Arc<AtomicBool>>>,
        job_id: JobId,
        value: bool,
    ) -> Arc<AtomicBool> {
        let mut map = flags.lock().await;
        let flag = map
            .entry(job_id)
            .or_insert_with(|| Arc::new(AtomicBool::new(value)));
        flag.store(value, Ordering::SeqCst);
        Arc::clone(flag)
    }

    async fn get_or_init_flag(
        flags: &tokio::sync::Mutex<HashMap<JobId, Arc<AtomicBool>>>,
        job_id: JobId,
    ) -> Arc<AtomicBool> {
        let mut map = flags.lock().await;
        Arc::clone(
            map.entry(job_id)
                .or_insert_with(|| Arc::new(AtomicBool::new(false))),
        )
    }

    fn emails_match(left: &str, right: &str) -> bool {
        left.trim().eq_ignore_ascii_case(right.trim()) && !left.trim().is_empty()
    }

    async fn run_transfer(
        &self,
        job: &mut crate::domain::job::MigrationJob,
        canary: bool,
    ) -> Result<TransferHalt, JobServiceError> {
        let source_token = self
            .token_provider
            .get_access_token(job.source_account_id())
            .await
            .map_err(|e| JobServiceError::TokenError(e.to_string()))?;
        let target_token = self
            .token_provider
            .get_access_token(job.target_account_id())
            .await
            .map_err(|e| JobServiceError::TokenError(e.to_string()))?;
        let source_perm = job.snapshots().source.permission_id.clone();
        let target_perm = job.snapshots().target.permission_id.clone();
        let target_email = job.snapshots().target.email.clone();
        let pause = Self::get_or_init_flag(&self.transfer_pause_flags, job.id()).await;
        let cancel = Self::get_or_init_flag(&self.transfer_cancel_flags, job.id()).await;
        let progress_total = self
            .job_store
            .list_items_for_transfer(job.id())
            .await?
            .len() as u64;
        let run = TransferRun {
            drive: self.drive.as_ref() as &dyn DriveTransferPort,
            store: &*self.job_store,
            sleeper: self.sleeper.as_ref(),
            jitter: self.jitter.as_ref(),
            source_token: &source_token,
            target_token: &target_token,
            source_permission_id: &source_perm,
            target_permission_id: &target_perm,
            target_email: &target_email,
            pause: Some(pause.as_ref()),
            cancel: Some(cancel.as_ref()),
            job_id: job.id(),
            events: Some(self.events.as_ref()),
            progress_total,
        };
        if canary {
            Ok(execute_canary(&run, job).await?)
        } else {
            Ok(execute_bulk(&run, job).await?)
        }
    }

    pub async fn run_transfer_auto(
        &self,
        job: &mut crate::domain::job::MigrationJob,
    ) -> Result<crate::application::transfer::TransferHalt, JobServiceError> {
        let source_token = self
            .token_provider
            .get_access_token(job.source_account_id())
            .await
            .map_err(|e| JobServiceError::TokenError(e.to_string()))?;
        let target_token = self
            .token_provider
            .get_access_token(job.target_account_id())
            .await
            .map_err(|e| JobServiceError::TokenError(e.to_string()))?;
        let source_perm = job.snapshots().source.permission_id.clone();
        let target_perm = job.snapshots().target.permission_id.clone();
        let target_email = job.snapshots().target.email.clone();
        let pause = Self::get_or_init_flag(&self.transfer_pause_flags, job.id()).await;
        let cancel = Self::get_or_init_flag(&self.transfer_cancel_flags, job.id()).await;
        let progress_total = self
            .job_store
            .list_items_for_transfer(job.id())
            .await?
            .len() as u64;
        let run = crate::application::transfer::TransferRun {
            drive: self.drive.as_ref()
                as &dyn crate::application::drive_transfer::DriveTransferPort,
            store: &*self.job_store,
            sleeper: self.sleeper.as_ref(),
            jitter: self.jitter.as_ref(),
            source_token: &source_token,
            target_token: &target_token,
            source_permission_id: &source_perm,
            target_permission_id: &target_perm,
            target_email: &target_email,
            pause: Some(pause.as_ref()),
            cancel: Some(cancel.as_ref()),
            job_id: job.id(),
            events: Some(self.events.as_ref()),
            progress_total,
        };
        let halt = crate::application::transfer::execute_auto_transfer(&run, job).await?;
        if job.status() == JobStatus::CanaryReview {
            let cohort = self.job_store.list_canary_cohort(job.id()).await?;
            if !cohort.is_empty()
                && cohort
                    .iter()
                    .all(|item| item.state == crate::domain::item::ItemState::Verified)
            {
                job.start_bulk()?;
                self.persist_status(job, Some(JobStatus::CanaryReview.as_str()), "JOB_STATUS")
                    .await?;
                return Ok(execute_bulk(&run, job).await?);
            }
        }
        Ok(halt)
    }

    async fn persist_paused(
        &self,
        job_id: JobId,
        error: impl Into<String>,
    ) -> Result<(), JobServiceError> {
        let mut job = self.get_job(job_id).await?;
        job.set_last_error(error);
        if job.status() == JobStatus::Scanning {
            job.pause_scanning()?;
        }
        self.job_store.update_job(&job).await?;
        Ok(())
    }

    async fn get_account_snapshot(
        &self,
        account_id: AccountId,
    ) -> Result<AccountSnapshot, JobServiceError> {
        let account = self
            .account_store
            .find_by_id(account_id)
            .await
            .map_err(|e| JobServiceError::StoreError(e.to_string()))?
            .ok_or(JobServiceError::SourceAccountNotFound(account_id))?;

        if !account.is_active() || account.auth_status() == AuthStatus::Disconnected {
            return Err(JobServiceError::AccountNotActive(account_id));
        }

        Ok(AccountSnapshot {
            account_id,
            email: account.email().to_string(),
            display_name: account.display_name().to_string(),
            permission_id: account.google_permission_id().clone(),
        })
    }

    pub async fn create_job(
        &self,
        source_id: AccountId,
        target_id: AccountId,
    ) -> Result<MigrationJob, JobServiceError> {
        let job = self.prepare_job(source_id, target_id).await?;
        self.job_store.create_job(&job).await?;
        self.emit(JobRuntimeEvent::JobListChanged { job_id: job.id() });
        Ok(job)
    }

    async fn prepare_job(
        &self,
        source_id: AccountId,
        target_id: AccountId,
    ) -> Result<MigrationJob, JobServiceError> {
        if source_id.value() == target_id.value() {
            return Err(JobServiceError::SameSourceAndTarget);
        }

        let source = self.get_account_snapshot(source_id).await?;
        let target = self
            .get_account_snapshot(target_id)
            .await
            .map_err(|e| match e {
                JobServiceError::SourceAccountNotFound(id) => {
                    JobServiceError::TargetAccountNotFound(id)
                }
                other => other,
            })?;

        let job_id = JobId::new(next_entity_id());
        let created_at = iso_now();

        Ok(MigrationJob::new(job_id, source, target, created_at)?)
    }

    pub async fn update_draft_job_accounts(
        &self,
        job_id: JobId,
        source_id: AccountId,
        target_id: AccountId,
    ) -> Result<MigrationJob, JobServiceError> {
        if source_id.value() == target_id.value() {
            return Err(JobServiceError::SameSourceAndTarget);
        }

        let mut job = self
            .job_store
            .find_job_by_id(job_id)
            .await?
            .ok_or(JobServiceError::JobNotFound(job_id))?;

        let source = self.get_account_snapshot(source_id).await?;
        let target = self
            .get_account_snapshot(target_id)
            .await
            .map_err(|e| match e {
                JobServiceError::SourceAccountNotFound(id) => {
                    JobServiceError::TargetAccountNotFound(id)
                }
                other => other,
            })?;

        job.change_accounts(source, target)?;
        self.job_store.update_draft_job(&job).await?;
        self.get_job(job_id).await
    }

    pub async fn get_job(&self, job_id: JobId) -> Result<MigrationJob, JobServiceError> {
        self.job_store
            .find_job_by_id(job_id)
            .await?
            .ok_or(JobServiceError::JobNotFound(job_id))
    }

    pub async fn list_jobs(&self) -> Result<Vec<MigrationJob>, JobServiceError> {
        self.job_store.list_jobs().await.map_err(Into::into)
    }

    pub async fn list_jobs_filtered(
        &self,
        status: Option<JobStatus>,
        account_id: Option<AccountId>,
    ) -> Result<Vec<MigrationJob>, JobServiceError> {
        let jobs = self.list_jobs().await?;
        Ok(jobs
            .into_iter()
            .filter(|job| job_matches_list_filter(job, status, account_id))
            .collect())
    }

    pub async fn job_run_phase(
        &self,
        job: &crate::domain::job::MigrationJob,
    ) -> Result<JobRunPhase, JobServiceError> {
        match job.status() {
            JobStatus::Draft | JobStatus::Scanning | JobStatus::ReadyForReview => {
                Ok(JobRunPhase::Scan)
            }
            JobStatus::RunningCanary | JobStatus::CanaryReview => Ok(JobRunPhase::Canary),
            JobStatus::Running | JobStatus::Pausing | JobStatus::Cancelling => {
                Ok(JobRunPhase::Bulk)
            }
            JobStatus::Queued => {
                if self.should_resume_as_canary(job).await? {
                    Ok(JobRunPhase::Canary)
                } else {
                    Ok(JobRunPhase::Bulk)
                }
            }
            JobStatus::Paused => {
                if !self.looks_like_transfer_pause(job).await? {
                    Ok(JobRunPhase::Scan)
                } else if self.should_resume_as_canary(job).await? {
                    Ok(JobRunPhase::Canary)
                } else {
                    Ok(JobRunPhase::Bulk)
                }
            }
            JobStatus::AuthRequired
            | JobStatus::SourceRateLimited
            | JobStatus::WaitingForQuota
            | JobStatus::Cancelled
            | JobStatus::Completed
            | JobStatus::CompletedWithErrors
            | JobStatus::Failed => {
                if self.should_resume_as_canary(job).await? {
                    Ok(JobRunPhase::Canary)
                } else if self.looks_like_transfer_pause(job).await?
                    || self.bulk_mutation_has_started(job).await?
                {
                    Ok(JobRunPhase::Bulk)
                } else {
                    Ok(JobRunPhase::Scan)
                }
            }
        }
    }

    pub async fn delete_draft_job(&self, job_id: JobId) -> Result<(), JobServiceError> {
        let job = self.get_job(job_id).await?;
        if job.status() != JobStatus::Draft {
            return Err(JobServiceError::IllegalTransition);
        }
        self.job_store.delete_draft_job(job_id).await?;
        self.emit(JobRuntimeEvent::JobListChanged { job_id });
        Ok(())
    }

    pub async fn account_references(
        &self,
        account_id: AccountId,
    ) -> Result<Vec<AccountJobReference>, JobServiceError> {
        self.account_store
            .find_by_id(account_id)
            .await
            .map_err(|e| JobServiceError::StoreError(e.to_string()))?
            .ok_or(JobServiceError::SourceAccountNotFound(account_id))?;

        let jobs = self.list_jobs().await?;
        Ok(jobs
            .into_iter()
            .filter_map(|job| AccountJobReference::from_job(&job, account_id))
            .collect())
    }

    pub async fn validate_root(
        &self,
        job_id: JobId,
        input: &str,
    ) -> Result<DriveFolderMetadata, JobServiceError> {
        let job = self.get_job(job_id).await?;
        if job.status() != crate::domain::job::JobStatus::Draft {
            return Err(JobServiceError::RootsLocked);
        }
        let folder_id = parse_root_input(input).map_err(JobServiceError::ParseError)?;

        let source_id = job.source_account_id();
        let token = self
            .token_provider
            .get_access_token(source_id)
            .await
            .map_err(|e| JobServiceError::TokenError(e.to_string()))?;

        let metadata = self
            .drive
            .get_folder_metadata(&token, &folder_id)
            .await
            .map_err(|e| match e {
                DriveFolderLookupError::NotFound => JobServiceError::FolderNotFound,
                DriveFolderLookupError::Unauthorized | DriveFolderLookupError::Forbidden => {
                    JobServiceError::TokenError(e.to_string())
                }
                other => JobServiceError::DriveError(other.to_string()),
            })?;

        if metadata.trashed {
            return Err(JobServiceError::FolderTrashed);
        }

        if metadata.mime_type != "application/vnd.google-apps.folder" {
            return Err(JobServiceError::NotAFolder);
        }

        if metadata.drive_id.is_some() {
            return Err(JobServiceError::SharedDriveNotSupported);
        }

        let source_perm = &job.snapshots().source.permission_id;
        let is_owned = metadata
            .owners
            .iter()
            .any(|o| &o.permission_id == source_perm);

        if !is_owned {
            return Err(JobServiceError::NotOwnedBySourceAccount);
        }

        Ok(metadata)
    }

    pub async fn add_root(
        &self,
        job_id: JobId,
        input: &str,
    ) -> Result<MigrationJob, JobServiceError> {
        let metadata = self.validate_root(job_id, input).await?;
        let mut job = self.get_job(job_id).await?;

        let root = MigrationRoot {
            id: RootId::new(next_entity_id()),
            job_id,
            root_file_id: metadata.id,
            root_name: metadata.name,
            validation_status: RootValidationStatus::Validated,
            created_at: iso_now(),
        };

        job.add_root(root.clone())?;
        self.job_store.add_root(&root).await?;
        self.get_job(job_id).await
    }

    pub async fn remove_root(
        &self,
        job_id: JobId,
        root_id: RootId,
    ) -> Result<MigrationJob, JobServiceError> {
        let mut job = self.get_job(job_id).await?;
        job.remove_root(root_id)?;
        self.job_store.remove_root(job_id, root_id).await?;
        self.get_job(job_id).await
    }

    pub async fn start_transfer_operation(
        &self,
        source_id: AccountId,
        target_id: AccountId,
        root_file_ids: Vec<String>,
        recursive: bool,
    ) -> Result<MigrationJob, JobServiceError> {
        if source_id.value() == target_id.value() {
            return Err(JobServiceError::SameSourceAndTarget);
        }
        if root_file_ids.is_empty() {
            return Err(JobServiceError::NoValidatedRoots);
        }

        let mut job = self.prepare_job(source_id, target_id).await?;
        let source_token = self
            .token_provider
            .get_access_token(source_id)
            .await
            .map_err(|e| JobServiceError::TokenError(e.to_string()))?;

        for file_id in root_file_ids {
            let metadata = self
                .drive
                .get_folder_metadata(&source_token, &file_id)
                .await
                .map_err(|e| match e {
                    DriveFolderLookupError::NotFound => JobServiceError::FolderNotFound,
                    DriveFolderLookupError::Unauthorized | DriveFolderLookupError::Forbidden => {
                        JobServiceError::TokenError(e.to_string())
                    }
                    other => JobServiceError::DriveError(other.to_string()),
                })?;

            if metadata.trashed {
                return Err(JobServiceError::FolderTrashed);
            }
            if metadata.drive_id.is_some() {
                return Err(JobServiceError::SharedDriveNotSupported);
            }
            let source_perm = &job.snapshots().source.permission_id;
            let is_owned = metadata
                .owners
                .iter()
                .any(|o| &o.permission_id == source_perm);
            if !is_owned {
                return Err(JobServiceError::NotOwnedBySourceAccount);
            }

            let root = MigrationRoot {
                id: RootId::new(next_entity_id()),
                job_id: job.id(),
                root_file_id: metadata.id,
                root_name: metadata.name,
                validation_status: RootValidationStatus::Validated,
                created_at: iso_now(),
            };
            job.add_root(root.clone())?;
        }

        let pause = AtomicBool::new(false);
        let root_batch = crate::application::scanner::collect_roots(
            &ScanRun {
                drive: Arc::clone(&self.drive),
                store: &*self.job_store,
                job_id: job.id(),
                roots: job.roots(),
                source_token: &source_token,
                source_permission_id: &job.snapshots().source.permission_id,
                target_permission_id: &job.snapshots().target.permission_id,
                pause: &pause,
                concurrency: DEFAULT_SCAN_CONCURRENCY,
                events: None,
            },
            recursive,
        )
        .await?;
        job.start_scanning(iso_now())?;
        self.job_store.create_seeded_scan(&job, &root_batch).await?;
        self.emit(JobRuntimeEvent::JobListChanged { job_id: job.id() });
        self.start_scan_with_auto(job.id(), true).await
    }

    pub async fn start_scan(&self, job_id: JobId) -> Result<MigrationJob, JobServiceError> {
        self.start_scan_with_auto(job_id, false).await
    }

    pub async fn start_scan_with_auto(
        &self,
        job_id: JobId,
        auto_continue: bool,
    ) -> Result<MigrationJob, JobServiceError> {
        let mut job = self.get_job(job_id).await?;
        if job.status() == JobStatus::Paused && self.looks_like_transfer_pause(&job).await? {
            return Err(JobServiceError::IllegalTransition);
        }
        let lease = self.try_acquire_scan(job_id)?;

        let pause = {
            let mut flags = self.scan_pause_flags.lock().await;
            let flag = flags
                .entry(job_id)
                .or_insert_with(|| Arc::new(AtomicBool::new(false)));
            flag.store(false, Ordering::SeqCst);
            Arc::clone(flag)
        };

        let source_id = job.source_account_id();
        let source_token = match self.token_provider.get_access_token(source_id).await {
            Ok(token) => token,
            Err(err) => {
                drop(lease);
                if job.status() == JobStatus::Scanning {
                    self.persist_paused(job_id, format!("failed to obtain OAuth token: {err}"))
                        .await?;
                    self.emit_status(&self.get_job(job_id).await?);
                }
                return Err(JobServiceError::TokenError(err.to_string()));
            }
        };

        job.start_scanning(iso_now())?;
        self.job_store.update_job(&job).await?;
        self.emit_status(&job);

        let worker = self.clone();
        tokio::spawn(async move {
            let _lease = lease;
            worker
                .execute_scan(job_id, source_token, pause, auto_continue)
                .await;
        });

        Ok(job)
    }

    async fn execute_scan(
        &self,
        job_id: JobId,
        source_token: crate::application::AccessToken,
        pause: Arc<AtomicBool>,
        auto_continue: bool,
    ) {
        let Ok(job) = self.get_job(job_id).await else {
            return;
        };
        let target_id = job.target_account_id();
        let source_perm = job.snapshots().source.permission_id.clone();
        let target_perm = job.snapshots().target.permission_id.clone();
        let roots = job.roots().to_vec();

        let outcome = run_scan(&ScanRun {
            drive: Arc::clone(&self.drive),
            store: &*self.job_store,
            job_id,
            roots: &roots,
            source_token: &source_token,
            source_permission_id: &source_perm,
            target_permission_id: &target_perm,
            pause: &pause,
            concurrency: DEFAULT_SCAN_CONCURRENCY,
            events: Some(Arc::clone(&self.events)),
        })
        .await;

        let mut job = match self.get_job(job_id).await {
            Ok(job) => job,
            Err(_) => {
                self.scan_pause_flags.lock().await.remove(&job_id);
                return;
            }
        };
        let mut scan_succeeded = false;
        match outcome {
            Ok(ScanOutcome::Completed) => {
                match self.token_provider.get_access_token(target_id).await {
                    Ok(target_token) => {
                        if let Err(err) = self.drive.get_storage_quota(&target_token).await {
                            job.set_last_error(format!("quota lookup failed: {err}"));
                        }
                    }
                    Err(err) => {
                        job.set_last_error(format!("quota lookup token failed: {err}"));
                    }
                }
                if let Err(err) = job.complete_scanning() {
                    job.set_last_error(err.to_string());
                } else {
                    scan_succeeded = true;
                }
            }
            Ok(ScanOutcome::Paused) => {
                if let Err(err) = job.pause_scanning() {
                    job.set_last_error(err.to_string());
                }
            }
            Err(err) if err.is_retryable() => {
                let mapped: JobServiceError = err.into();
                let _ = self.persist_paused(job_id, mapped.to_string()).await;
                self.scan_pause_flags.lock().await.remove(&job_id);
                if let Ok(paused) = self.get_job(job_id).await {
                    self.emit_status(&paused);
                }
                return;
            }
            Err(err) => {
                if let Err(fail_err) = job.fail_scanning(err.to_string()) {
                    job.set_last_error(fail_err.to_string());
                }
            }
        }

        let _ = self.job_store.update_job(&job).await;
        self.emit_status(&job);
        self.scan_pause_flags.lock().await.remove(&job_id);

        if scan_succeeded && auto_continue {
            let auto_runner = self.clone();
            tokio::spawn(async move {
                let _ = auto_runner.run_auto_mutation_if_ready(job_id).await;
            });
        }
    }

    pub async fn run_auto_mutation_if_ready(&self, job_id: JobId) -> Result<(), JobServiceError> {
        let mut job = self.get_job(job_id).await?;
        if job.status() != JobStatus::ReadyForReview {
            return Ok(());
        }
        let lease = match self.try_acquire_transfer(job_id).await {
            Ok(lease) => lease,
            Err(JobServiceError::TransferInProgress) => {
                let _ = self.queue_or_busy(job_id).await;
                return Ok(());
            }
            Err(err) => return Err(err),
        };

        let previous = job.status().as_str().to_string();
        if let Err(err) = job.start_canary() {
            drop(lease);
            self.release_durable_lease(job_id).await;
            return Err(err.into());
        }
        if let Err(err) = self
            .persist_status(&job, Some(&previous), "JOB_STATUS")
            .await
        {
            drop(lease);
            self.release_durable_lease(job_id).await;
            return Err(err);
        }

        let started = job.clone();
        let worker = self.clone();
        tokio::spawn(async move {
            let mut running = started;
            Self::set_control_flag(&worker.transfer_pause_flags, job_id, false).await;
            Self::set_control_flag(&worker.transfer_cancel_flags, job_id, false).await;
            let mut previous = running.status().as_str().to_string();
            let result = worker.run_transfer_auto(&mut running).await;
            if let Ok(persisted) = worker.get_job(job_id).await {
                previous = persisted.status().as_str().to_string();
            }
            if running.status() == JobStatus::Cancelled {
                let _ = worker.job_store.cancel_unstarted_items(job_id).await;
            }
            let _ = worker
                .persist_status(&running, Some(&previous), "JOB_STATUS")
                .await;
            drop(lease);
            worker.release_durable_lease(job_id).await;
            worker.transfer_pause_flags.lock().await.remove(&job_id);
            worker.transfer_cancel_flags.lock().await.remove(&job_id);
            let _ = result;
        });

        Ok(())
    }

    pub async fn pause_scan(&self, job_id: JobId) -> Result<MigrationJob, JobServiceError> {
        let job = self.get_job(job_id).await?;
        if job.status() != JobStatus::Scanning && job.status() != JobStatus::Paused {
            return Err(JobServiceError::IllegalTransition);
        }

        {
            let mut flags = self.scan_pause_flags.lock().await;
            let flag = flags
                .entry(job_id)
                .or_insert_with(|| Arc::new(AtomicBool::new(true)));
            flag.store(true, Ordering::SeqCst);
        }

        if !self.scan_is_in_flight(job_id) {
            let mut job = job;
            job.pause_scanning()?;
            self.job_store.update_job(&job).await?;
        }

        self.get_job(job_id).await
    }

    pub async fn list_job_items(
        &self,
        job_id: JobId,
        filter: Option<&str>,
        page: u32,
    ) -> Result<ItemPage, JobServiceError> {
        let _job = self.get_job(job_id).await?;
        self.job_store
            .list_items_page(job_id, filter, page, 50)
            .await
            .map_err(Into::into)
    }

    pub async fn preflight(&self, job_id: JobId) -> Result<PreflightSummary, JobServiceError> {
        let job = self.get_job(job_id).await?;
        let target_token = self
            .token_provider
            .get_access_token(job.target_account_id())
            .await
            .map_err(|e| JobServiceError::TokenError(e.to_string()))?;
        let quota = self
            .drive
            .get_storage_quota(&target_token)
            .await
            .map_err(|e| JobServiceError::DriveError(e.to_string()))?;
        let aggregates = self.job_store.item_aggregates(job_id).await?;
        Ok(PreflightSummary::from_aggregates(&aggregates, &quota))
    }

    pub async fn export_dry_run(
        &self,
        job_id: JobId,
        destination: &str,
    ) -> Result<String, JobServiceError> {
        let path = Path::new(destination);
        if destination.trim().is_empty() {
            return Err(JobServiceError::ExportFailed(
                "destination path is required".to_string(),
            ));
        }

        let job = self.get_job(job_id).await?;
        let report = if destination_is_csv(destination) {
            self.render_dry_run_csv(job_id).await?
        } else {
            let summary = self.preflight(job_id).await?;
            let roots: Vec<String> = job
                .roots()
                .iter()
                .map(|root| root.root_name.clone())
                .collect();
            summary.render_report(
                &job.id().to_string(),
                &job.snapshots().source.email,
                &job.snapshots().target.email,
                &roots,
            )
        };

        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).map_err(|e| {
                JobServiceError::ExportFailed(format!(
                    "could not create destination directory: {e}"
                ))
            })?;
        }

        std::fs::write(path, report.as_bytes())
            .map_err(|e| JobServiceError::ExportFailed(e.to_string()))?;
        Ok(destination.to_string())
    }

    async fn render_dry_run_csv(&self, job_id: JobId) -> Result<String, JobServiceError> {
        let mut page = 1_u32;
        let mut items = Vec::new();
        loop {
            let chunk = self
                .job_store
                .list_items_page(job_id, None, page, 500)
                .await?;
            let chunk_len = chunk.items.len();
            items.extend(chunk.items);
            if items.len() as u64 >= chunk.total || chunk_len == 0 || page >= 10_000 {
                break;
            }
            page += 1;
        }

        Ok(render_items_csv(items.iter().map(|item| DryRunCsvRow {
            file_id: &item.file_id,
            name: &item.name,
            mime_type: &item.mime_type,
            depth: item.depth,
            state: item.state.as_str(),
            quota_bytes_used: item.quota_bytes_used,
        })))
    }

    pub async fn scan_summary(
        &self,
        job_id: JobId,
    ) -> Result<Option<PreflightSummary>, JobServiceError> {
        let aggregates = self.job_store.item_aggregates(job_id).await?;
        if aggregates.total == 0 {
            return Ok(None);
        }
        match self.preflight(job_id).await {
            Ok(summary) => Ok(Some(summary)),
            Err(JobServiceError::TokenError(_)) | Err(JobServiceError::DriveError(_)) => {
                Ok(Some(PreflightSummary::from_aggregates(
                    &aggregates,
                    &crate::application::StorageQuota {
                        limit_bytes: None,
                        usage_bytes: 0,
                    },
                )))
            }
            Err(err) => Err(err),
        }
    }

    pub async fn start_canary(
        &self,
        job_id: JobId,
        confirmation_email: &str,
    ) -> Result<MigrationJob, JobServiceError> {
        let mut job = self.get_job(job_id).await?;
        if !Self::emails_match(confirmation_email, &job.snapshots().target.email) {
            return Err(JobServiceError::ConfirmationMismatch);
        }
        match job.status() {
            JobStatus::ReadyForReview | JobStatus::RunningCanary | JobStatus::Queued => {}
            _ => return Err(JobServiceError::IllegalTransition),
        }
        let lease = match self.try_acquire_transfer(job_id).await {
            Ok(lease) => lease,
            Err(JobServiceError::TransferInProgress) => {
                return self.queue_or_busy(job_id).await;
            }
            Err(err) => return Err(err),
        };
        self.prepare_and_run_mutation(job_id, &mut job, lease, true, |job| {
            job.start_canary().map_err(Into::into)
        })
        .await
    }

    pub async fn continue_migration(&self, job_id: JobId) -> Result<MigrationJob, JobServiceError> {
        let mut job = self.get_job(job_id).await?;
        let as_canary = self.should_resume_as_canary(&job).await?;
        match job.status() {
            JobStatus::CanaryReview | JobStatus::Running => {}
            JobStatus::Queued if !as_canary => {}
            _ => return Err(JobServiceError::IllegalTransition),
        }
        let lease = match self.try_acquire_transfer(job_id).await {
            Ok(lease) => lease,
            Err(JobServiceError::TransferInProgress) => {
                return self.queue_or_busy(job_id).await;
            }
            Err(err) => return Err(err),
        };
        self.prepare_and_run_mutation(job_id, &mut job, lease, false, |job| {
            if job.status() == JobStatus::Queued {
                job.resume_transfer(false).map_err(Into::into)
            } else {
                job.start_bulk().map_err(Into::into)
            }
        })
        .await
    }

    async fn prepare_and_run_mutation(
        &self,
        job_id: JobId,
        job: &mut crate::domain::job::MigrationJob,
        lease: TransferLeaseGuard,
        canary: bool,
        prepare: impl FnOnce(&mut crate::domain::job::MigrationJob) -> Result<(), JobServiceError>,
    ) -> Result<MigrationJob, JobServiceError> {
        let previous = job.status().as_str().to_string();
        if let Err(err) = prepare(job) {
            drop(lease);
            self.release_durable_lease(job_id).await;
            return Err(err);
        }
        if let Err(err) = self
            .persist_status(job, Some(&previous), "JOB_STATUS")
            .await
        {
            drop(lease);
            self.release_durable_lease(job_id).await;
            return Err(err);
        }
        let started = job.clone();
        let worker = self.clone();
        tokio::spawn(async move {
            let mut running = started;
            let _ = worker
                .run_mutation(job_id, &mut running, canary, lease)
                .await;
        });
        Ok(job.clone())
    }

    async fn run_mutation(
        &self,
        job_id: JobId,
        job: &mut crate::domain::job::MigrationJob,
        canary: bool,
        lease: TransferLeaseGuard,
    ) -> Result<MigrationJob, JobServiceError> {
        Self::set_control_flag(&self.transfer_pause_flags, job_id, false).await;
        Self::set_control_flag(&self.transfer_cancel_flags, job_id, false).await;
        let previous = job.status().as_str().to_string();
        let result = self.run_transfer(job, canary).await;
        if job.status() == JobStatus::Cancelled {
            let _ = self.job_store.cancel_unstarted_items(job_id).await;
        }
        let persist = self
            .persist_status(job, Some(&previous), "JOB_STATUS")
            .await;
        drop(lease);
        self.release_durable_lease(job_id).await;
        self.transfer_pause_flags.lock().await.remove(&job_id);
        self.transfer_cancel_flags.lock().await.remove(&job_id);
        match result {
            Ok(_) => {
                persist?;
                self.get_job(job_id).await
            }
            Err(err) => {
                let _ = persist;
                Err(err)
            }
        }
    }

    async fn queue_or_busy(&self, job_id: JobId) -> Result<MigrationJob, JobServiceError> {
        if self.transfer_is_in_flight(job_id) {
            return Err(JobServiceError::TransferInProgress);
        }
        if let Some(lease) = self.job_store.current_mutation_lease().await?
            && lease.job_id == job_id
        {
            return Err(JobServiceError::TransferInProgress);
        }
        let job = self.get_job(job_id).await?;
        if job.status() == JobStatus::Running || job.status() == JobStatus::RunningCanary {
            return Err(JobServiceError::TransferInProgress);
        }
        self.queue_job(job_id, None).await
    }

    pub async fn pause_migration(&self, job_id: JobId) -> Result<MigrationJob, JobServiceError> {
        let job = self.get_job(job_id).await?;
        match job.status() {
            JobStatus::RunningCanary
            | JobStatus::Running
            | JobStatus::Pausing
            | JobStatus::Paused => {}
            _ => return Err(JobServiceError::IllegalTransition),
        }

        Self::set_control_flag(&self.transfer_pause_flags, job_id, true).await;

        if !self.transfer_is_in_flight(job_id) {
            let previous = job.status().as_str().to_string();
            let mut job = job;
            job.pause_transfer()?;
            self.persist_status(&job, Some(&previous), "JOB_STATUS")
                .await?;
            self.release_durable_lease(job_id).await;
            self.clear_memory_lease(job_id);
            self.transfer_pause_flags.lock().await.remove(&job_id);
        }

        self.get_job(job_id).await
    }

    pub async fn resume_migration(&self, job_id: JobId) -> Result<MigrationJob, JobServiceError> {
        let mut job = self.get_job(job_id).await?;
        if !job.status().is_transfer_resumable()
            || self.job_run_phase(&job).await? == JobRunPhase::Scan
        {
            return Err(JobServiceError::IllegalTransition);
        }
        let as_canary = self.should_resume_as_canary(&job).await?;
        let lease = match self.try_acquire_transfer(job_id).await {
            Ok(lease) => lease,
            Err(JobServiceError::TransferInProgress) => {
                return self.queue_or_busy(job_id).await;
            }
            Err(err) => return Err(err),
        };

        if let Err(err) = self.revalidate_tokens(&job).await {
            drop(lease);
            self.release_durable_lease(job_id).await;
            return Err(err);
        }
        if let Err(err) = self.reconcile_job_items(&job).await {
            drop(lease);
            self.release_durable_lease(job_id).await;
            return Err(err);
        }

        self.prepare_and_run_mutation(job_id, &mut job, lease, as_canary, |job| {
            job.resume_transfer(as_canary).map_err(Into::into)
        })
        .await
    }

    pub async fn cancel_migration(&self, job_id: JobId) -> Result<MigrationJob, JobServiceError> {
        let mut job = self.get_job(job_id).await?;
        Self::set_control_flag(&self.transfer_pause_flags, job_id, true).await;
        Self::set_control_flag(&self.transfer_cancel_flags, job_id, true).await;

        if self.transfer_is_in_flight(job_id) {
            return self.get_job(job_id).await;
        }

        let previous = job.status().as_str().to_string();
        let _cancelled = self.job_store.cancel_unstarted_items(job_id).await?;
        job.cancel_job(iso_now())?;
        self.persist_status(&job, Some(&previous), "JOB_STATUS")
            .await?;
        self.release_durable_lease(job_id).await;
        self.clear_memory_lease(job_id);
        self.get_job(job_id).await
    }

    pub async fn retry_failed_items(&self, job_id: JobId) -> Result<MigrationJob, JobServiceError> {
        let job = self.get_job(job_id).await?;
        let _reset = self.job_store.retry_failed_items(job_id).await?;
        if job.status().is_transfer_resumable() {
            return self.resume_migration(job_id).await;
        }
        self.get_job(job_id).await
    }

    pub async fn queue_job(
        &self,
        job_id: JobId,
        position: Option<i64>,
    ) -> Result<MigrationJob, JobServiceError> {
        let jobs = self.job_store.list_jobs().await?;
        let mut target = jobs
            .iter()
            .find(|job| job.id() == job_id)
            .cloned()
            .ok_or(JobServiceError::JobNotFound(job_id))?;

        let mut queued: Vec<MigrationJob> = jobs
            .iter()
            .filter(|job| job.status() == JobStatus::Queued && job.id() != job_id)
            .cloned()
            .collect();
        queued.sort_by_key(|job| job.queue_position().unwrap_or(i64::MAX));

        let insert_at = position
            .map(|pos| (pos.max(1) as usize).saturating_sub(1))
            .unwrap_or(queued.len())
            .min(queued.len());
        let previous = target.status().as_str().to_string();
        target.enqueue((insert_at as i64) + 1)?;
        queued.insert(insert_at, target);

        for (index, job) in queued.iter_mut().enumerate() {
            job.set_queue_position(Some((index as i64) + 1));
            let event_previous = if job.id() == job_id {
                Some(previous.as_str())
            } else {
                Some(job.status().as_str())
            };
            self.persist_status(job, event_previous, "JOB_STATUS")
                .await?;
        }

        self.get_job(job_id).await
    }

    pub async fn reconcile_on_startup(&self) -> Result<(), JobServiceError> {
        self.job_store.clear_mutation_leases().await?;
        {
            let mut slot = self
                .transfer_lease
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *slot = None;
        }

        let jobs = self.job_store.list_jobs().await?;
        for mut job in jobs {
            if job.status() == JobStatus::Scanning {
                let previous = job.status().as_str().to_string();
                job.pause_scanning()?;
                self.persist_status(&job, Some(&previous), "STARTUP_RECONCILE")
                    .await?;
                continue;
            }
            if !job.status().is_unfinished_mutation() {
                continue;
            }
            let previous = job.status().as_str().to_string();
            if let Err(err) = self.revalidate_tokens(&job).await {
                job.require_auth(err.to_string())?;
                self.persist_status(&job, Some(&previous), "STARTUP_RECONCILE")
                    .await?;
                continue;
            }
            if let Err(err) = self.reconcile_job_items(&job).await {
                job.set_last_error(err.to_string());
            }
            job.pause_transfer()?;
            self.persist_status(&job, Some(&previous), "STARTUP_RECONCILE")
                .await?;
        }
        Ok(())
    }

    async fn revalidate_tokens(
        &self,
        job: &crate::domain::job::MigrationJob,
    ) -> Result<(), JobServiceError> {
        self.token_provider
            .get_access_token(job.source_account_id())
            .await
            .map_err(|e| JobServiceError::TokenError(e.to_string()))?;
        self.token_provider
            .get_access_token(job.target_account_id())
            .await
            .map_err(|e| JobServiceError::TokenError(e.to_string()))?;
        Ok(())
    }

    async fn reconcile_job_items(
        &self,
        job: &crate::domain::job::MigrationJob,
    ) -> Result<(), JobServiceError> {
        let source_token = self
            .token_provider
            .get_access_token(job.source_account_id())
            .await
            .map_err(|e| JobServiceError::TokenError(e.to_string()))?;
        let target_token = self
            .token_provider
            .get_access_token(job.target_account_id())
            .await
            .map_err(|e| JobServiceError::TokenError(e.to_string()))?;
        let source_perm = job.snapshots().source.permission_id.clone();
        let target_perm = job.snapshots().target.permission_id.clone();
        let target_email = job.snapshots().target.email.clone();
        let items = self.job_store.list_items_for_transfer(job.id()).await?;
        let run = TransferRun {
            drive: self.drive.as_ref() as &dyn DriveTransferPort,
            store: &*self.job_store,
            sleeper: self.sleeper.as_ref(),
            jitter: self.jitter.as_ref(),
            source_token: &source_token,
            target_token: &target_token,
            source_permission_id: &source_perm,
            target_permission_id: &target_perm,
            target_email: &target_email,
            pause: None,
            cancel: None,
            job_id: job.id(),
            events: None,
            progress_total: items.len() as u64,
        };
        crate::application::transfer::reconcile_checkpoint(&run, &items).await?;
        Ok(())
    }

    async fn should_resume_as_canary(
        &self,
        job: &crate::domain::job::MigrationJob,
    ) -> Result<bool, JobServiceError> {
        if let Some(event) = self.job_store.latest_job_event(job.id()).await? {
            if event.previous_state.as_deref() == Some(JobStatus::Running.as_str())
                || event.new_state.as_deref() == Some(JobStatus::Running.as_str())
                || event.previous_state.as_deref() == Some(JobStatus::CanaryReview.as_str())
            {
                return Ok(false);
            }
            if event.previous_state.as_deref() == Some(JobStatus::RunningCanary.as_str())
                || event.new_state.as_deref() == Some(JobStatus::RunningCanary.as_str())
            {
                return Ok(true);
            }
        }
        Ok(!self.bulk_mutation_has_started(job).await?)
    }

    async fn bulk_mutation_has_started(
        &self,
        job: &crate::domain::job::MigrationJob,
    ) -> Result<bool, JobServiceError> {
        let items = self.job_store.list_items_for_transfer(job.id()).await?;
        Ok(items.iter().any(|item| {
            !item.canary_selected
                && item.state != crate::domain::item::ItemState::Eligible
                && item.state.is_transfer_active()
        }))
    }

    async fn looks_like_transfer_pause(
        &self,
        job: &crate::domain::job::MigrationJob,
    ) -> Result<bool, JobServiceError> {
        if let Some(event) = self.job_store.latest_job_event(job.id()).await?
            && matches!(
                event.previous_state.as_deref(),
                Some("RUNNING_CANARY" | "RUNNING" | "PAUSING" | "CANCELLING")
            )
        {
            return Ok(true);
        }
        let cohort = self.job_store.list_canary_cohort(job.id()).await?;
        Ok(!cohort.is_empty() || self.bulk_mutation_has_started(job).await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transfer_lease_is_exclusive_including_same_job() {
        let slot = Arc::new(std::sync::Mutex::new(None));
        let first = JobId::new(1);
        let second = JobId::new(2);

        let guard = try_acquire_transfer_slot(&slot, first).expect("first acquire");
        assert!(matches!(
            try_acquire_transfer_slot(&slot, first),
            Err(JobServiceError::TransferInProgress)
        ));
        assert!(matches!(
            try_acquire_transfer_slot(&slot, second),
            Err(JobServiceError::TransferInProgress)
        ));
        assert_eq!(
            *slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner()),
            Some(first)
        );

        drop(guard);
        assert_eq!(
            *slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner()),
            None
        );

        let later = try_acquire_transfer_slot(&slot, second).expect("acquire after release");
        drop(later);
        assert_eq!(
            *slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner()),
            None
        );
    }
}
