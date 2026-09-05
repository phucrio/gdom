use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
use std::str::FromStr;

use super::account::{AccountId, GooglePermissionId};

pub const DEFAULT_TRANSFER_CONCURRENCY: usize = 1;
pub const DEFAULT_CANARY_SIZE: usize = 5;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct JobId(pub u128);

impl JobId {
    pub const fn new(value: u128) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u128 {
        self.0
    }
}

impl fmt::Display for JobId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for JobId {
    type Err = std::num::ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<u128>().map(Self)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct RootId(pub u128);

impl RootId {
    pub const fn new(value: u128) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u128 {
        self.0
    }
}

impl fmt::Display for RootId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for RootId {
    type Err = std::num::ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<u128>().map(Self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JobStatus {
    Draft,
    Scanning,
    ReadyForReview,
    RunningCanary,
    CanaryReview,
    Queued,
    Running,
    Pausing,
    Paused,
    Cancelling,
    Cancelled,
    Completed,
    CompletedWithErrors,
    Failed,
    AuthRequired,
    SourceRateLimited,
    WaitingForQuota,
}

impl JobStatus {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "DRAFT",
            Self::Scanning => "SCANNING",
            Self::ReadyForReview => "READY_FOR_REVIEW",
            Self::RunningCanary => "RUNNING_CANARY",
            Self::CanaryReview => "CANARY_REVIEW",
            Self::Queued => "QUEUED",
            Self::Running => "RUNNING",
            Self::Pausing => "PAUSING",
            Self::Paused => "PAUSED",
            Self::Cancelling => "CANCELLING",
            Self::Cancelled => "CANCELLED",
            Self::Completed => "COMPLETED",
            Self::CompletedWithErrors => "COMPLETED_WITH_ERRORS",
            Self::Failed => "FAILED",
            Self::AuthRequired => "AUTH_REQUIRED",
            Self::SourceRateLimited => "SOURCE_RATE_LIMITED",
            Self::WaitingForQuota => "WAITING_FOR_QUOTA",
        }
    }

    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::CompletedWithErrors | Self::Cancelled | Self::Failed
        )
    }

    const fn require_draft_pair(self) -> Result<(), JobError> {
        match self {
            Self::Draft => Ok(()),
            Self::Scanning
            | Self::ReadyForReview
            | Self::RunningCanary
            | Self::CanaryReview
            | Self::Queued
            | Self::Running
            | Self::Pausing
            | Self::Paused
            | Self::Cancelling
            | Self::Cancelled
            | Self::Completed
            | Self::CompletedWithErrors
            | Self::Failed
            | Self::AuthRequired
            | Self::SourceRateLimited
            | Self::WaitingForQuota => Err(JobError::AccountPairLocked),
        }
    }

    const fn require_draft_roots(self) -> Result<(), JobError> {
        match self {
            Self::Draft => Ok(()),
            Self::Scanning
            | Self::ReadyForReview
            | Self::RunningCanary
            | Self::CanaryReview
            | Self::Queued
            | Self::Running
            | Self::Pausing
            | Self::Paused
            | Self::Cancelling
            | Self::Cancelled
            | Self::Completed
            | Self::CompletedWithErrors
            | Self::Failed
            | Self::AuthRequired
            | Self::SourceRateLimited
            | Self::WaitingForQuota => Err(JobError::RootsLocked),
        }
    }
}

impl fmt::Display for JobStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for JobStatus {
    type Err = JobError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "DRAFT" => Ok(Self::Draft),
            "SCANNING" => Ok(Self::Scanning),
            "READY_FOR_REVIEW" => Ok(Self::ReadyForReview),
            "RUNNING_CANARY" => Ok(Self::RunningCanary),
            "CANARY_REVIEW" => Ok(Self::CanaryReview),
            "QUEUED" => Ok(Self::Queued),
            "RUNNING" => Ok(Self::Running),
            "PAUSING" => Ok(Self::Pausing),
            "PAUSED" => Ok(Self::Paused),
            "CANCELLING" => Ok(Self::Cancelling),
            "CANCELLED" => Ok(Self::Cancelled),
            "COMPLETED" => Ok(Self::Completed),
            "COMPLETED_WITH_ERRORS" => Ok(Self::CompletedWithErrors),
            "FAILED" => Ok(Self::Failed),
            "AUTH_REQUIRED" => Ok(Self::AuthRequired),
            "SOURCE_RATE_LIMITED" => Ok(Self::SourceRateLimited),
            "WAITING_FOR_QUOTA" => Ok(Self::WaitingForQuota),
            _ => Err(JobError::InvalidJobStatus),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RootValidationStatus {
    Validated,
    Pending,
    Failed,
}

impl RootValidationStatus {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Validated => "VALIDATED",
            Self::Pending => "PENDING",
            Self::Failed => "FAILED",
        }
    }
}

impl fmt::Display for RootValidationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for RootValidationStatus {
    type Err = JobError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "VALIDATED" => Ok(Self::Validated),
            "PENDING" => Ok(Self::Pending),
            "FAILED" => Ok(Self::Failed),
            _ => Err(JobError::InvalidRootValidationStatus),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MigrationRoot {
    pub id: RootId,
    pub job_id: JobId,
    pub root_file_id: String,
    pub root_name: String,
    pub validation_status: RootValidationStatus,
    pub created_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountSnapshot {
    pub account_id: AccountId,
    pub email: String,
    pub display_name: String,
    pub permission_id: GooglePermissionId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JobAccountSnapshots {
    pub source: AccountSnapshot,
    pub target: AccountSnapshot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountPair {
    source: AccountId,
    target: AccountId,
}

impl AccountPair {
    pub const fn new(source: AccountId, target: AccountId) -> Result<Self, JobError> {
        if source.value() == target.value() {
            return Err(JobError::SameSourceAndTarget);
        }

        Ok(Self { source, target })
    }

    pub const fn source(&self) -> AccountId {
        self.source
    }

    pub const fn target(&self) -> AccountId {
        self.target
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JobError {
    SameSourceAndTarget,
    AccountPairLocked,
    RootsLocked,
    DuplicateRoot(String),
    RootNotFound(RootId),
    InvalidJobStatus,
    InvalidRootValidationStatus,
    IllegalTransition,
    NoValidatedRoots,
}

impl fmt::Display for JobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SameSourceAndTarget => write!(f, "Source and target account cannot be identical"),
            Self::AccountPairLocked => write!(
                f,
                "Account pair cannot be changed once job is no longer draft"
            ),
            Self::RootsLocked => write!(
                f,
                "Roots cannot be added or removed once job is no longer draft"
            ),
            Self::DuplicateRoot(file_id) => write!(
                f,
                "Root file ID already exists in this migration job: {file_id}"
            ),
            Self::RootNotFound(id) => write!(f, "Root not found in job: {id}"),
            Self::InvalidJobStatus => write!(f, "Invalid job status string"),
            Self::InvalidRootValidationStatus => write!(f, "Invalid root validation status string"),
            Self::IllegalTransition => write!(f, "Illegal job status transition"),
            Self::NoValidatedRoots => write!(f, "Scan requires at least one validated root"),
        }
    }
}

impl Error for JobError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationJob {
    id: JobId,
    accounts: AccountPair,
    snapshots: JobAccountSnapshots,
    status: JobStatus,
    queue_position: Option<i64>,
    canary_size: usize,
    created_at: String,
    started_at: Option<String>,
    completed_at: Option<String>,
    last_error: Option<String>,
    roots: Vec<MigrationRoot>,
}

impl MigrationJob {
    pub fn new(
        id: JobId,
        source: AccountSnapshot,
        target: AccountSnapshot,
        created_at: String,
    ) -> Result<Self, JobError> {
        let accounts = AccountPair::new(source.account_id, target.account_id)?;
        Ok(Self {
            id,
            accounts,
            snapshots: JobAccountSnapshots { source, target },
            status: JobStatus::Draft,
            queue_position: None,
            canary_size: DEFAULT_CANARY_SIZE,
            created_at,
            started_at: None,
            completed_at: None,
            last_error: None,
            roots: Vec::new(),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn reconstitute(
        id: JobId,
        accounts: AccountPair,
        snapshots: JobAccountSnapshots,
        status: JobStatus,
        queue_position: Option<i64>,
        canary_size: usize,
        created_at: String,
        started_at: Option<String>,
        completed_at: Option<String>,
        last_error: Option<String>,
        roots: Vec<MigrationRoot>,
    ) -> Self {
        Self {
            id,
            accounts,
            snapshots,
            status,
            queue_position,
            canary_size,
            created_at,
            started_at,
            completed_at,
            last_error,
            roots,
        }
    }

    pub fn change_accounts(
        &mut self,
        source: AccountSnapshot,
        target: AccountSnapshot,
    ) -> Result<(), JobError> {
        self.status.require_draft_pair()?;
        let accounts = AccountPair::new(source.account_id, target.account_id)?;
        let source_changed = self.accounts.source != accounts.source;
        self.accounts = accounts;
        self.snapshots = JobAccountSnapshots { source, target };
        if source_changed {
            self.roots.clear();
        }
        Ok(())
    }

    pub fn add_root(&mut self, root: MigrationRoot) -> Result<(), JobError> {
        self.status.require_draft_roots()?;
        if self
            .roots
            .iter()
            .any(|r| r.root_file_id == root.root_file_id)
        {
            return Err(JobError::DuplicateRoot(root.root_file_id));
        }
        self.roots.push(root);
        Ok(())
    }

    pub fn remove_root(&mut self, root_id: RootId) -> Result<(), JobError> {
        self.status.require_draft_roots()?;
        let pos = self
            .roots
            .iter()
            .position(|r| r.id == root_id)
            .ok_or(JobError::RootNotFound(root_id))?;
        self.roots.remove(pos);
        Ok(())
    }

    pub fn start_scanning(&mut self, started_at: String) -> Result<(), JobError> {
        match self.status {
            JobStatus::Draft => {
                if self.roots.is_empty() {
                    return Err(JobError::NoValidatedRoots);
                }
                self.status = JobStatus::Scanning;
                self.started_at = Some(started_at);
                Ok(())
            }
            JobStatus::Scanning
            | JobStatus::ReadyForReview
            | JobStatus::RunningCanary
            | JobStatus::CanaryReview
            | JobStatus::Queued
            | JobStatus::Running
            | JobStatus::Pausing
            | JobStatus::Paused
            | JobStatus::Cancelling
            | JobStatus::Cancelled
            | JobStatus::Completed
            | JobStatus::CompletedWithErrors
            | JobStatus::Failed
            | JobStatus::AuthRequired
            | JobStatus::SourceRateLimited
            | JobStatus::WaitingForQuota => Err(JobError::IllegalTransition),
        }
    }

    pub const fn id(&self) -> JobId {
        self.id
    }

    pub const fn accounts(&self) -> AccountPair {
        self.accounts
    }

    pub const fn source_account_id(&self) -> AccountId {
        self.accounts.source
    }

    pub const fn target_account_id(&self) -> AccountId {
        self.accounts.target
    }

    pub fn snapshots(&self) -> &JobAccountSnapshots {
        &self.snapshots
    }

    pub const fn status(&self) -> JobStatus {
        self.status
    }

    pub const fn queue_position(&self) -> Option<i64> {
        self.queue_position
    }

    pub const fn canary_size(&self) -> usize {
        self.canary_size
    }

    pub fn created_at(&self) -> &str {
        &self.created_at
    }

    pub fn started_at(&self) -> Option<&str> {
        self.started_at.as_deref()
    }

    pub fn completed_at(&self) -> Option<&str> {
        self.completed_at.as_deref()
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    pub fn roots(&self) -> &[MigrationRoot] {
        &self.roots
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_snapshot(id: u128, email: &str, perm: &str) -> AccountSnapshot {
        AccountSnapshot {
            account_id: AccountId::new(id),
            email: email.to_string(),
            display_name: format!("User {id}"),
            permission_id: GooglePermissionId::new(perm),
        }
    }

    #[test]
    fn job_rejects_identical_source_and_target() {
        // Given
        let source = sample_snapshot(1, "source@gmail.com", "perm_1");
        let target = sample_snapshot(1, "source@gmail.com", "perm_1");

        // When
        let result = MigrationJob::new(
            JobId::new(100),
            source,
            target,
            "2026-09-05T00:00:00Z".to_string(),
        );

        // Then
        assert_eq!(result, Err(JobError::SameSourceAndTarget));
    }

    #[test]
    fn job_allows_account_pair_change_while_draft() {
        // Given
        let source = sample_snapshot(1, "source@gmail.com", "perm_1");
        let target = sample_snapshot(2, "target@gmail.com", "perm_2");
        let mut job = MigrationJob::new(
            JobId::new(100),
            source.clone(),
            target,
            "2026-09-05T00:00:00Z".to_string(),
        )
        .expect("different accounts form a valid job");

        // When
        let new_target = sample_snapshot(3, "target3@gmail.com", "perm_3");
        let result = job.change_accounts(source, new_target);

        // Then
        assert_eq!(result, Ok(()));
        assert_eq!(job.target_account_id(), AccountId::new(3));
    }

    #[test]
    fn job_rejects_account_pair_change_after_scanning_starts() {
        // Given
        let source = sample_snapshot(1, "source@gmail.com", "perm_1");
        let target = sample_snapshot(2, "target@gmail.com", "perm_2");
        let mut job = MigrationJob::new(
            JobId::new(100),
            source.clone(),
            target,
            "2026-09-05T00:00:00Z".to_string(),
        )
        .expect("different accounts form a valid job");
        job.add_root(MigrationRoot {
            id: RootId::new(501),
            job_id: job.id(),
            root_file_id: "folder_abc".to_string(),
            root_name: "My Folder".to_string(),
            validation_status: RootValidationStatus::Validated,
            created_at: "2026-09-05T00:00:00Z".to_string(),
        })
        .expect("root added");
        job.start_scanning("2026-09-05T01:00:00Z".to_string())
            .expect("scan starts");

        // When
        let new_target = sample_snapshot(3, "target3@gmail.com", "perm_3");
        let result = job.change_accounts(source, new_target);

        // Then
        assert_eq!(result, Err(JobError::AccountPairLocked));
        assert_eq!(job.target_account_id(), AccountId::new(2));
    }

    #[test]
    fn changing_draft_source_clears_validated_roots() {
        let source = sample_snapshot(1, "source@gmail.com", "perm_1");
        let target = sample_snapshot(2, "target@gmail.com", "perm_2");
        let mut job = MigrationJob::new(
            JobId::new(100),
            source,
            target.clone(),
            "2026-09-05T00:00:00Z".to_string(),
        )
        .expect("valid job");
        job.add_root(MigrationRoot {
            id: RootId::new(501),
            job_id: job.id(),
            root_file_id: "folder_abc".to_string(),
            root_name: "My Folder".to_string(),
            validation_status: RootValidationStatus::Validated,
            created_at: "2026-09-05T00:00:00Z".to_string(),
        })
        .expect("root added");

        let new_source = sample_snapshot(3, "source3@gmail.com", "perm_3");
        job.change_accounts(new_source, target)
            .expect("draft source can change");

        assert!(job.roots().is_empty());
        assert_eq!(job.source_account_id(), AccountId::new(3));
    }

    #[test]
    fn changing_draft_target_keeps_roots() {
        let source = sample_snapshot(1, "source@gmail.com", "perm_1");
        let target = sample_snapshot(2, "target@gmail.com", "perm_2");
        let mut job = MigrationJob::new(
            JobId::new(100),
            source.clone(),
            target,
            "2026-09-05T00:00:00Z".to_string(),
        )
        .expect("valid job");
        job.add_root(MigrationRoot {
            id: RootId::new(501),
            job_id: job.id(),
            root_file_id: "folder_abc".to_string(),
            root_name: "My Folder".to_string(),
            validation_status: RootValidationStatus::Validated,
            created_at: "2026-09-05T00:00:00Z".to_string(),
        })
        .expect("root added");

        let new_target = sample_snapshot(3, "target3@gmail.com", "perm_3");
        job.change_accounts(source, new_target)
            .expect("draft target can change");

        assert_eq!(job.roots().len(), 1);
    }

    #[test]
    fn start_scanning_requires_a_root_and_rejects_repeat() {
        let source = sample_snapshot(1, "source@gmail.com", "perm_1");
        let target = sample_snapshot(2, "target@gmail.com", "perm_2");
        let mut job = MigrationJob::new(
            JobId::new(100),
            source,
            target,
            "2026-09-05T00:00:00Z".to_string(),
        )
        .expect("valid job");

        assert_eq!(
            job.start_scanning("2026-09-05T01:00:00Z".to_string()),
            Err(JobError::NoValidatedRoots)
        );

        job.add_root(MigrationRoot {
            id: RootId::new(501),
            job_id: job.id(),
            root_file_id: "folder_abc".to_string(),
            root_name: "My Folder".to_string(),
            validation_status: RootValidationStatus::Validated,
            created_at: "2026-09-05T00:00:00Z".to_string(),
        })
        .expect("root added");
        job.start_scanning("2026-09-05T01:00:00Z".to_string())
            .expect("scan starts");
        assert_eq!(
            job.start_scanning("2026-09-05T01:01:00Z".to_string()),
            Err(JobError::IllegalTransition)
        );
    }

    #[test]
    fn job_manages_roots_in_draft_and_locks_in_scanning() {
        // Given
        let source = sample_snapshot(1, "source@gmail.com", "perm_1");
        let target = sample_snapshot(2, "target@gmail.com", "perm_2");
        let mut job = MigrationJob::new(
            JobId::new(100),
            source,
            target,
            "2026-09-05T00:00:00Z".to_string(),
        )
        .expect("valid job");

        let root = MigrationRoot {
            id: RootId::new(501),
            job_id: job.id(),
            root_file_id: "folder_abc".to_string(),
            root_name: "My Folder".to_string(),
            validation_status: RootValidationStatus::Validated,
            created_at: "2026-09-05T00:00:00Z".to_string(),
        };

        // When: add root
        assert_eq!(job.add_root(root.clone()), Ok(()));
        assert_eq!(job.roots().len(), 1);

        // Duplicate root rejected
        assert_eq!(
            job.add_root(root.clone()),
            Err(JobError::DuplicateRoot("folder_abc".to_string()))
        );

        // Lock in scanning
        job.start_scanning("2026-09-05T01:00:00Z".to_string())
            .expect("scan starts");
        assert_eq!(
            job.add_root(MigrationRoot {
                id: RootId::new(502),
                job_id: job.id(),
                root_file_id: "folder_def".to_string(),
                root_name: "Second Folder".to_string(),
                validation_status: RootValidationStatus::Validated,
                created_at: "2026-09-05T01:00:00Z".to_string(),
            }),
            Err(JobError::RootsLocked)
        );

        assert_eq!(
            job.remove_root(RootId::new(501)),
            Err(JobError::RootsLocked)
        );
    }

    #[test]
    fn transfer_concurrency_defaults_to_one() {
        assert_eq!(DEFAULT_TRANSFER_CONCURRENCY, 1);
    }
}
