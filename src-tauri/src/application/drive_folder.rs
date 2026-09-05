use std::error::Error;
use std::fmt;
use std::future::Future;
use std::pin::Pin;

use crate::application::AccessToken;
use crate::domain::GooglePermissionId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DriveFolderOwner {
    pub permission_id: GooglePermissionId,
    pub email_address: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DriveFolderMetadata {
    pub id: String,
    pub name: String,
    pub mime_type: String,
    pub trashed: bool,
    pub drive_id: Option<String>,
    pub owners: Vec<DriveFolderOwner>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DriveFolderLookupError {
    NotFound,
    Unauthorized,
    Forbidden,
    RateLimited,
    Unavailable,
    Transport,
    InvalidResponse,
    UnexpectedStatus(u16),
}

impl fmt::Display for DriveFolderLookupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(f, "folder not found on Google Drive"),
            Self::Unauthorized => write!(f, "Google Drive rejected the access token"),
            Self::Forbidden => write!(f, "Google Drive denied this request"),
            Self::RateLimited => write!(f, "Google Drive rate limit reached"),
            Self::Unavailable => write!(f, "Google Drive is unavailable"),
            Self::Transport => write!(f, "Google Drive request failed"),
            Self::InvalidResponse => write!(f, "Google Drive returned an invalid response"),
            Self::UnexpectedStatus(status) => {
                write!(f, "Google Drive returned unexpected status {status}")
            }
        }
    }
}

impl Error for DriveFolderLookupError {}

pub type DriveFolderLookupFuture<'a> =
    Pin<Box<dyn Future<Output = Result<DriveFolderMetadata, DriveFolderLookupError>> + Send + 'a>>;

pub trait DriveFolderLookupPort: Send + Sync {
    fn get_folder_metadata<'a>(
        &'a self,
        token: &'a AccessToken,
        folder_id: &'a str,
    ) -> DriveFolderLookupFuture<'a>;
}
