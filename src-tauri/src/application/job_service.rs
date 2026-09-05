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
use crate::application::job_store::{JobStorePort, JobStorePortError};
use crate::application::preflight::PreflightSummary;
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
        }
    }
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

pub struct JobService<A, J>
where
    A: AccountStorePort + Send + Sync + 'static,
    J: JobStorePort + ItemStorePort + 'static,
{
    account_store: Arc<A>,
    job_store: Arc<J>,
    drive: Arc<dyn DrivePort>,
    token_provider: Arc<AccountTokenProvider<A>>,
    scan_pause_flags: tokio::sync::Mutex<HashMap<JobId, Arc<AtomicBool>>>,
    scan_in_flight: Arc<std::sync::Mutex<HashSet<JobId>>>,
    transfer_lease: Arc<std::sync::Mutex<Option<JobId>>>,
    sleeper: Arc<dyn Sleeper>,
    jitter: Arc<dyn JitterSource>,
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
            scan_pause_flags: tokio::sync::Mutex::new(HashMap::new()),
            scan_in_flight: Arc::new(std::sync::Mutex::new(HashSet::new())),
            transfer_lease: Arc::new(std::sync::Mutex::new(None)),
            sleeper: Arc::new(TokioSleeper),
            jitter: Arc::new(SystemJitter),
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

    fn try_acquire_transfer(&self, job_id: JobId) -> Result<TransferLeaseGuard, JobServiceError> {
        let mut lease = self
            .transfer_lease
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(active) = *lease
            && active != job_id
        {
            return Err(JobServiceError::TransferInProgress);
        }
        *lease = Some(job_id);
        drop(lease);
        Ok(TransferLeaseGuard {
            slot: Arc::clone(&self.transfer_lease),
            job_id,
        })
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
        };
        if canary {
            Ok(execute_canary(&run, job).await?)
        } else {
            Ok(execute_bulk(&run, job).await?)
        }
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

        let job = MigrationJob::new(job_id, source, target, created_at)?;
        self.job_store.create_job(&job).await?;
        Ok(job)
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

    pub async fn start_scan(&self, job_id: JobId) -> Result<MigrationJob, JobServiceError> {
        let mut job = self.get_job(job_id).await?;
        let _lease = self.try_acquire_scan(job_id)?;

        let pause = {
            let mut flags = self.scan_pause_flags.lock().await;
            let flag = flags
                .entry(job_id)
                .or_insert_with(|| Arc::new(AtomicBool::new(false)));
            flag.store(false, Ordering::SeqCst);
            Arc::clone(flag)
        };

        let source_id = job.source_account_id();
        let target_id = job.target_account_id();
        let source_token = match self.token_provider.get_access_token(source_id).await {
            Ok(token) => token,
            Err(err) => {
                if job.status() == JobStatus::Scanning {
                    self.persist_paused(job_id, format!("failed to obtain OAuth token: {err}"))
                        .await?;
                }
                return Err(JobServiceError::TokenError(err.to_string()));
            }
        };

        job.start_scanning(iso_now())?;
        self.job_store.update_job(&job).await?;

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
        })
        .await;

        let mut job = self.get_job(job_id).await?;
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
                job.complete_scanning()?;
            }
            Ok(ScanOutcome::Paused) => {
                job.pause_scanning()?;
            }
            Err(err) if err.is_retryable() => {
                let mapped: JobServiceError = err.into();
                self.persist_paused(job_id, mapped.to_string()).await?;
                self.scan_pause_flags.lock().await.remove(&job_id);
                return Err(mapped);
            }
            Err(err) => {
                let message = err.to_string();
                let _ = job.fail_scanning(message.clone());
                self.job_store.update_job(&job).await?;
                self.scan_pause_flags.lock().await.remove(&job_id);
                return Err(err.into());
            }
        }

        self.job_store.update_job(&job).await?;
        self.scan_pause_flags.lock().await.remove(&job_id);
        self.get_job(job_id).await
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
        let summary = self.preflight(job_id).await?;
        let roots: Vec<String> = job
            .roots()
            .iter()
            .map(|root| root.root_name.clone())
            .collect();
        let report = summary.render_report(
            &job.id().to_string(),
            &job.snapshots().source.email,
            &job.snapshots().target.email,
            &roots,
        );

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
        let _lease = self.try_acquire_transfer(job_id)?;
        job.start_canary()?;
        self.job_store.update_job(&job).await?;
        match self.run_transfer(&mut job, true).await {
            Ok(_) => {
                self.job_store.update_job(&job).await?;
                self.get_job(job_id).await
            }
            Err(err) => {
                let _ = self.job_store.update_job(&job).await;
                Err(err)
            }
        }
    }

    pub async fn continue_migration(&self, job_id: JobId) -> Result<MigrationJob, JobServiceError> {
        let mut job = self.get_job(job_id).await?;
        let _lease = self.try_acquire_transfer(job_id)?;
        job.start_bulk()?;
        self.job_store.update_job(&job).await?;
        match self.run_transfer(&mut job, false).await {
            Ok(_) => {
                self.job_store.update_job(&job).await?;
                self.get_job(job_id).await
            }
            Err(err) => {
                let _ = self.job_store.update_job(&job).await;
                Err(err)
            }
        }
    }
}
