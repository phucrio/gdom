use std::future::Future;
use std::pin::Pin;

use crate::application::AccessToken;
use crate::application::drive_folder::{
    DriveFolderLookupError, DriveFolderLookupPort, DriveFolderOwner,
};
use crate::application::drive_transfer::DriveTransferPort;

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
    pub modified_time: Option<String>,
    pub web_view_link: Option<String>,
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

    fn list_browse_children<'a>(
        &'a self,
        token: &'a AccessToken,
        folder_id: Option<&'a str>,
        page_token: Option<&'a str>,
        _page_size: Option<u32>,
        _order_by: Option<&'a str>,
    ) -> DriveListFuture<'a> {
        let fid = folder_id.unwrap_or("root");
        self.list_children(token, fid, page_token)
    }

    fn rename_file<'a>(
        &'a self,
        token: &'a AccessToken,
        file_id: &'a str,
        new_name: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), DriveTreeError>> + Send + 'a>> {
        let _ = (token, file_id, new_name);
        Box::pin(async move { Ok(()) })
    }

    fn trash_file<'a>(
        &'a self,
        token: &'a AccessToken,
        file_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), DriveTreeError>> + Send + 'a>> {
        let _ = (token, file_id);
        Box::pin(async move { Ok(()) })
    }
}

pub trait DriveQuotaPort: Send + Sync {
    fn get_storage_quota<'a>(&'a self, token: &'a AccessToken) -> DriveQuotaFuture<'a>;
}

pub trait DrivePort:
    DriveFolderLookupPort + DriveTreePort + DriveQuotaPort + DriveTransferPort
{
}

impl<T> DrivePort for T where
    T: DriveFolderLookupPort + DriveTreePort + DriveQuotaPort + DriveTransferPort
{
}
