use std::error::Error;
use std::fmt;
use std::pin::Pin;

use crate::application::AccessToken;
use crate::application::drive_folder::DriveFolderOwner;
use crate::domain::ItemErrorDetails;

const MAX_TRANSFER_ERROR_CHARS: usize = 2_048;

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
    pub quota_bytes_used: Option<i64>,
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

    pub const fn http_status(self) -> Option<u16> {
        match self {
            Self::Unauthorized => Some(401),
            Self::Forbidden | Self::SharingRateLimitExceeded | Self::StorageQuotaExceeded => {
                Some(403)
            }
            Self::NotFound => Some(404),
            Self::RateLimited => Some(429),
            Self::UnexpectedStatus(status) => Some(status),
            Self::ServerUnavailable | Self::Transport | Self::InvalidResponse => None,
        }
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::Unauthorized => "unauthorized",
            Self::Forbidden => "forbidden",
            Self::NotFound => "not_found",
            Self::RateLimited => "rate_limited",
            Self::SharingRateLimitExceeded => "sharing_rate_limit_exceeded",
            Self::StorageQuotaExceeded => "storage_quota_exceeded",
            Self::ServerUnavailable => "server_unavailable",
            Self::Transport => "transport",
            Self::InvalidResponse => "invalid_response",
            Self::UnexpectedStatus(_) => "unexpected_status",
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct DriveTransferFailure {
    kind: DriveTransferError,
    http_status: Option<u16>,
    google_status: Option<String>,
    reasons: Vec<String>,
    message: Option<String>,
}

impl DriveTransferFailure {
    pub fn from_api_response(
        kind: DriveTransferError,
        http_status: u16,
        google_status: Option<String>,
        reasons: Vec<String>,
        message: Option<String>,
    ) -> Self {
        Self {
            kind,
            http_status: Some(http_status),
            google_status: google_status.map(|value| sanitize_error_detail(&value)),
            reasons: reasons
                .into_iter()
                .take(16)
                .map(|value| sanitize_error_detail(&value))
                .filter(|value| !value.is_empty())
                .collect(),
            message: message
                .map(|value| sanitize_error_detail(&value))
                .filter(|value| !value.is_empty()),
        }
    }

    pub const fn kind(&self) -> DriveTransferError {
        self.kind
    }

    pub fn item_error_details(&self) -> ItemErrorDetails {
        let code = self
            .http_status
            .or_else(|| self.kind.http_status())
            .map(|status| status.to_string());
        let mut reasons = Vec::with_capacity(self.reasons.len() + 1);
        if let Some(status) = &self.google_status {
            reasons.push(status.as_str());
        }
        reasons.extend(self.reasons.iter().map(String::as_str));
        let reason = Some(if reasons.is_empty() {
            self.kind.code().to_owned()
        } else {
            reasons.join(": ")
        });
        let message = Some(
            self.message
                .clone()
                .unwrap_or_else(|| self.kind.to_string()),
        );
        ItemErrorDetails {
            code,
            reason,
            message,
        }
    }
}

impl fmt::Debug for DriveTransferFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DriveTransferFailure")
            .field("kind", &self.kind)
            .field("http_status", &self.http_status)
            .field("google_status_present", &self.google_status.is_some())
            .field("reason_count", &self.reasons.len())
            .field("message_present", &self.message.is_some())
            .finish()
    }
}

impl From<DriveTransferError> for DriveTransferFailure {
    fn from(kind: DriveTransferError) -> Self {
        Self {
            kind,
            http_status: kind.http_status(),
            google_status: None,
            reasons: Vec::new(),
            message: None,
        }
    }
}

impl fmt::Display for DriveTransferFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.message {
            Some(message) => f.write_str(message),
            None => self.kind.fmt(f),
        }
    }
}

impl Error for DriveTransferFailure {}

fn sanitize_error_detail(value: &str) -> String {
    gdom_logs::redact_secrets(value)
        .chars()
        .take(MAX_TRANSFER_ERROR_CHARS)
        .collect()
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
    Pin<Box<dyn Future<Output = Result<DriveFileSnapshot, DriveTransferFailure>> + Send + 'a>>;

pub type DrivePermissionFuture<'a> =
    Pin<Box<dyn Future<Output = Result<DrivePermission, DriveTransferFailure>> + Send + 'a>>;

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
#[cfg(test)]
mod tests {
    use super::{DriveTransferError, DriveTransferFailure};

    #[test]
    fn drive_transfer_failure_debug_redacts_diagnostics() {
        let failure = DriveTransferFailure::from_api_response(
            DriveTransferError::Forbidden,
            403,
            Some("PERMISSION_DENIED".into()),
            vec!["insufficientFilePermissions".into()],
            Some("access_token=hidden-token".into()),
        );

        let debug = format!("{failure:?}");

        assert!(debug.contains("http_status: Some(403)"));
        assert!(!debug.contains("PERMISSION_DENIED"));
        assert!(!debug.contains("insufficientFilePermissions"));
        assert!(!debug.contains("hidden-token"));
    }
}
