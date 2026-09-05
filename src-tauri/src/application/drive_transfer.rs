use std::error::Error;
use std::fmt;
use std::future::Future;
use std::pin::Pin;

use crate::application::AccessToken;
use crate::application::drive_folder::DriveFolderOwner;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DrivePermission {
    pub id: String,
    pub role: String,
    pub type_: String,
    pub email_address: Option<String>,
    pub pending_owner: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DriveFileSnapshot {
    pub id: String,
    pub name: String,
    pub mime_type: String,
    pub parents: Vec<String>,
    pub owners: Vec<DriveFolderOwner>,
    pub trashed: bool,
    pub drive_id: Option<String>,
    pub permissions: Vec<DrivePermission>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DriveTransferError {
    Unauthorized,
    Forbidden,
    NotFound,
    RateLimited,
    SharingRateLimitExceeded,
    StorageQuotaExceeded,
    ServerUnavailable,
    Transport,
    InvalidResponse,
    UnexpectedStatus(u16),
}

impl DriveTransferError {
    pub const fn is_transient(self) -> bool {
        matches!(
            self,
            Self::RateLimited | Self::ServerUnavailable | Self::Transport
        )
    }
}

impl fmt::Display for DriveTransferError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unauthorized => write!(f, "Google Drive rejected the access token"),
            Self::Forbidden => write!(f, "Google Drive denied this request"),
            Self::NotFound => write!(f, "Google Drive file not found"),
            Self::RateLimited => write!(f, "Google Drive rate limit reached"),
            Self::SharingRateLimitExceeded => {
                write!(f, "Google Drive sharing rate limit exceeded")
            }
            Self::StorageQuotaExceeded => write!(f, "Google Drive storage quota exceeded"),
            Self::ServerUnavailable => write!(f, "Google Drive is unavailable"),
            Self::Transport => write!(f, "Google Drive request failed"),
            Self::InvalidResponse => write!(f, "Google Drive returned an invalid response"),
            Self::UnexpectedStatus(status) => {
                write!(f, "Google Drive returned unexpected status {status}")
            }
        }
    }
}

impl Error for DriveTransferError {}

pub type DriveFileFuture<'a> =
    Pin<Box<dyn Future<Output = Result<DriveFileSnapshot, DriveTransferError>> + Send + 'a>>;

pub type DrivePermissionFuture<'a> =
    Pin<Box<dyn Future<Output = Result<DrivePermission, DriveTransferError>> + Send + 'a>>;

pub trait DriveTransferPort: Send + Sync {
    fn get_file<'a>(&'a self, token: &'a AccessToken, file_id: &'a str) -> DriveFileFuture<'a>;

    fn create_pending_owner<'a>(
        &'a self,
        token: &'a AccessToken,
        file_id: &'a str,
        email: &'a str,
    ) -> DrivePermissionFuture<'a>;

    fn update_pending_owner<'a>(
        &'a self,
        token: &'a AccessToken,
        file_id: &'a str,
        permission_id: &'a str,
    ) -> DrivePermissionFuture<'a>;

    fn accept_ownership<'a>(
        &'a self,
        token: &'a AccessToken,
        file_id: &'a str,
        permission_id: &'a str,
    ) -> DrivePermissionFuture<'a>;
}
