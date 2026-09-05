use std::future::Future;
use std::pin::Pin;

use crate::application::AccessToken;
use crate::application::drive_folder::{
    DriveFolderLookupError, DriveFolderLookupPort, DriveFolderOwner,
};

pub const FOLDER_MIME_TYPE: &str = "application/vnd.google-apps.folder";
pub const SHORTCUT_MIME_TYPE: &str = "application/vnd.google-apps.shortcut";
pub const LIST_PAGE_SIZE: u32 = 1000;
pub const DEFAULT_SCAN_CONCURRENCY: usize = 4;
pub const SCAN_CHECKPOINT_BATCH_SIZE: usize = 100;

pub type DriveTreeError = DriveFolderLookupError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DriveChild {
    pub id: String,
    pub name: String,
    pub mime_type: String,
    pub parents: Vec<String>,
    pub owners: Vec<DriveFolderOwner>,
    pub drive_id: Option<String>,
    pub quota_bytes_used: Option<i64>,
    pub trashed: bool,
    pub shortcut_target_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DriveChildPage {
    pub files: Vec<DriveChild>,
    pub next_page_token: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StorageQuota {
    pub limit_bytes: Option<u64>,
    pub usage_bytes: u64,
}

pub type DriveListFuture<'a> =
    Pin<Box<dyn Future<Output = Result<DriveChildPage, DriveTreeError>> + Send + 'a>>;

pub type DriveQuotaFuture<'a> =
    Pin<Box<dyn Future<Output = Result<StorageQuota, DriveTreeError>> + Send + 'a>>;

pub trait DriveTreePort: Send + Sync {
    fn list_children<'a>(
        &'a self,
        token: &'a AccessToken,
        folder_id: &'a str,
        page_token: Option<&'a str>,
    ) -> DriveListFuture<'a>;
}

pub trait DriveQuotaPort: Send + Sync {
    fn get_storage_quota<'a>(&'a self, token: &'a AccessToken) -> DriveQuotaFuture<'a>;
}

pub trait DrivePort: DriveFolderLookupPort + DriveTreePort + DriveQuotaPort {}

impl<T> DrivePort for T where T: DriveFolderLookupPort + DriveTreePort + DriveQuotaPort {}
