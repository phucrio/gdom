use std::error::Error;
use std::fmt;
use std::sync::Arc;

use crate::application::account_token_provider::AccountTokenProvider;
use crate::application::connect_account::AccountStorePort;
use crate::application::drive_folder::{
    DriveFolderLookupError, DriveFolderLookupPort, DriveFolderMetadata,
};
use crate::application::job_store::{JobStorePort, JobStorePortError};
use crate::application::root_parser::{RootParseError, parse_root_input};
use crate::domain::job::{
    AccountSnapshot, JobError, JobId, MigrationJob, MigrationRoot, RootId, RootValidationStatus,
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
            JobError::IllegalTransition => {
                Self::StoreError("illegal job status transition".to_string())
            }
            JobError::NoValidatedRoots => {
                Self::StoreError("scan requires at least one root".to_string())
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

pub struct JobService<A, J>
where
    A: AccountStorePort + Send + Sync + 'static,
    J: JobStorePort + 'static,
{
    account_store: Arc<A>,
    job_store: Arc<J>,
    drive: Arc<dyn DriveFolderLookupPort>,
    token_provider: Arc<AccountTokenProvider<A>>,
}

impl<A, J> JobService<A, J>
where
    A: AccountStorePort + Send + Sync + 'static,
    J: JobStorePort + 'static,
{
    pub fn new(
        account_store: Arc<A>,
        job_store: Arc<J>,
        drive: Arc<dyn DriveFolderLookupPort>,
        token_provider: Arc<AccountTokenProvider<A>>,
    ) -> Self {
        Self {
            account_store,
            job_store,
            drive,
            token_provider,
        }
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
        let created_at = chrono_iso_now();

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
            created_at: chrono_iso_now(),
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
}

fn next_entity_id() -> u128 {
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQ: AtomicU64 = AtomicU64::new(1);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(1);
    let seq = u128::from(SEQ.fetch_add(1, Ordering::Relaxed));
    (nanos << 16) | (seq & 0xFFFF)
}

fn chrono_iso_now() -> String {
    use std::time::SystemTime;
    let now = SystemTime::now();
    let duration = now
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    let millis = duration.subsec_millis();

    let days = (secs / 86400) as i64;
    let rem_secs = secs % 86400;
    let hours = rem_secs / 3600;
    let minutes = (rem_secs % 3600) / 60;
    let seconds = rem_secs % 60;

    let (year, month, day) = days_to_ymd(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        year, month, day, hours, minutes, seconds, millis
    )
}

fn days_to_ymd(days: i64) -> (i64, i64, i64) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let y = y + if m <= 2 { 1 } else { 0 };
    (y, m, d)
}
