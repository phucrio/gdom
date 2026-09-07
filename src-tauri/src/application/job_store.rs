use std::error::Error;
use std::fmt;
use std::future::Future;
use std::pin::Pin;

use crate::domain::AccountId;
use crate::domain::job::{JobId, MigrationJob, MigrationRoot, RootId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerLease {
    pub job_id: JobId,
    pub owner_instance_id: String,
    pub acquired_at: String,
    pub heartbeat_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationEvent {
    pub id: String,
    pub job_id: JobId,
    pub file_id: Option<String>,
    pub account_id: Option<AccountId>,
    pub event_type: String,
    pub previous_state: Option<String>,
    pub new_state: Option<String>,
    pub sanitized_detail_json: Option<String>,
    pub created_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JobStorePortError {
    JobNotFound(JobId),
    RootNotFound(RootId),
    SameSourceAndTarget,
    AccountNotFound(AccountId),
    AccountHasActiveJobs(AccountId),
    DuplicateRoot(String),
    AccountPairLocked,
    RootsLocked,
    MutationLeaseHeld,
    NotDraftJob(JobId),
    Database(String),
}

impl fmt::Display for JobStorePortError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::JobNotFound(id) => write!(f, "job not found: {id}"),
            Self::RootNotFound(id) => write!(f, "root not found: {id}"),
            Self::SameSourceAndTarget => write!(f, "source and target account cannot be identical"),
            Self::AccountNotFound(id) => write!(f, "account not found: {}", id.value()),
            Self::AccountHasActiveJobs(id) => {
                write!(
                    f,
                    "account {} is referenced by active migration jobs",
                    id.value()
                )
            }
            Self::DuplicateRoot(file_id) => write!(f, "root file ID already exists: {file_id}"),
            Self::AccountPairLocked => {
                write!(f, "account pair cannot be changed after draft status")
            }
            Self::RootsLocked => write!(f, "roots cannot be changed after draft status"),
            Self::MutationLeaseHeld => {
                write!(f, "another migration job already holds the mutation lease")
            }
            Self::NotDraftJob(id) => {
                write!(f, "job {id} is not a draft and cannot be deleted")
            }
            Self::Database(msg) => write!(f, "database error: {msg}"),
        }
    }
}

impl Error for JobStorePortError {}

pub type JobStoreFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, JobStorePortError>> + Send + 'a>>;

pub trait JobStorePort: Send + Sync {
    fn create_job<'a>(&'a self, job: &'a MigrationJob) -> JobStoreFuture<'a, ()>;
    fn create_seeded_scan<'a>(
        &'a self,
        job: &'a MigrationJob,
        roots: &'a crate::application::item_store::ItemBatchCommit,
    ) -> JobStoreFuture<'a, ()>;
    fn update_job<'a>(&'a self, job: &'a MigrationJob) -> JobStoreFuture<'a, ()>;
    fn update_draft_job<'a>(&'a self, job: &'a MigrationJob) -> JobStoreFuture<'a, ()>;
    fn find_job_by_id<'a>(&'a self, job_id: JobId) -> JobStoreFuture<'a, Option<MigrationJob>>;
    fn list_jobs<'a>(&'a self) -> JobStoreFuture<'a, Vec<MigrationJob>>;
    fn delete_draft_job<'a>(&'a self, job_id: JobId) -> JobStoreFuture<'a, ()>;
    fn add_root<'a>(&'a self, root: &'a MigrationRoot) -> JobStoreFuture<'a, ()>;
    fn remove_root<'a>(&'a self, job_id: JobId, root_id: RootId) -> JobStoreFuture<'a, ()>;
    fn list_roots_for_job<'a>(&'a self, job_id: JobId) -> JobStoreFuture<'a, Vec<MigrationRoot>>;
    fn has_active_jobs_for_account<'a>(&'a self, account_id: AccountId)
    -> JobStoreFuture<'a, bool>;
    fn has_jobs_for_account<'a>(&'a self, account_id: AccountId) -> JobStoreFuture<'a, bool>;
    fn acquire_mutation_lease<'a>(
        &'a self,
        job_id: JobId,
        owner_instance_id: &'a str,
        acquired_at: &'a str,
    ) -> JobStoreFuture<'a, ()>;
    fn release_mutation_lease<'a>(
        &'a self,
        job_id: JobId,
        owner_instance_id: &'a str,
    ) -> JobStoreFuture<'a, ()>;
    fn clear_mutation_leases<'a>(&'a self) -> JobStoreFuture<'a, ()>;
    fn current_mutation_lease<'a>(&'a self) -> JobStoreFuture<'a, Option<WorkerLease>>;
    fn persist_job_with_event<'a>(
        &'a self,
        job: &'a MigrationJob,
        event: &'a MigrationEvent,
    ) -> JobStoreFuture<'a, ()>;
    fn append_migration_event<'a>(&'a self, event: &'a MigrationEvent) -> JobStoreFuture<'a, ()>;
    fn latest_job_event<'a>(&'a self, job_id: JobId) -> JobStoreFuture<'a, Option<MigrationEvent>>;
}
