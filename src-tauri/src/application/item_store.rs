use std::error::Error;
use std::fmt;
use std::future::Future;
use std::pin::Pin;

use crate::domain::item::{MigrationItem, ScanCheckpoint};
use crate::domain::job::JobId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ItemStoreError {
    BatchTooLarge { size: usize, max: usize },
    InvalidState,
    Database(String),
}

impl fmt::Display for ItemStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BatchTooLarge { size, max } => {
                write!(f, "scan batch of {size} items exceeds limit {max}")
            }
            Self::InvalidState => write!(f, "invalid item state in database"),
            Self::Database(msg) => write!(f, "database error: {msg}"),
        }
    }
}

impl Error for ItemStoreError {}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ItemBatchCommit {
    pub items: Vec<MigrationItem>,
    pub checkpoints_upsert: Vec<ScanCheckpoint>,
    pub checkpoints_delete: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ItemPage {
    pub items: Vec<MigrationItem>,
    pub page: u32,
    pub page_size: u32,
    pub total: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ItemAggregates {
    pub total: u64,
    pub eligible: u64,
    pub eligible_files: u64,
    pub eligible_folders: u64,
    pub skipped_already_owned_by_target: u64,
    pub skipped_not_owned_by_source: u64,
    pub skipped_shared_drive: u64,
    pub skipped_shortcuts: u64,
    pub skipped_trashed: u64,
    pub skipped_ineligible: u64,
    pub estimated_quota_bytes: u64,
}

pub type ItemStoreFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, ItemStoreError>> + Send + 'a>>;

pub trait ItemStorePort: Send + Sync {
    fn commit_scan_batch<'a>(
        &'a self,
        job_id: JobId,
        batch: &'a ItemBatchCommit,
    ) -> ItemStoreFuture<'a, usize>;

    fn list_committed_file_ids<'a>(&'a self, job_id: JobId) -> ItemStoreFuture<'a, Vec<String>>;

    fn list_scan_checkpoints<'a>(
        &'a self,
        job_id: JobId,
    ) -> ItemStoreFuture<'a, Vec<ScanCheckpoint>>;

    fn list_items_page<'a>(
        &'a self,
        job_id: JobId,
        filter: Option<&'a str>,
        page: u32,
        page_size: u32,
    ) -> ItemStoreFuture<'a, ItemPage>;

    fn item_aggregates<'a>(&'a self, job_id: JobId) -> ItemStoreFuture<'a, ItemAggregates>;

    fn list_items_for_transfer<'a>(
        &'a self,
        job_id: JobId,
    ) -> ItemStoreFuture<'a, Vec<MigrationItem>>;

    fn list_canary_cohort<'a>(&'a self, job_id: JobId) -> ItemStoreFuture<'a, Vec<MigrationItem>>;

    fn save_item<'a>(&'a self, item: &'a MigrationItem) -> ItemStoreFuture<'a, ()>;
}
