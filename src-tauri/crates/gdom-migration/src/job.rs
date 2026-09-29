use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
use std::str::FromStr;

use super::account::{AccountId, GooglePermissionId};
mod status;
pub use status::JobStatus;
mod roots;
pub use roots::{MigrationRoot, RootId, RootValidationStatus};
mod scanning;
mod transfer;

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
mod tests;
