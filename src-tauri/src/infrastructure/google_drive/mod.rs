use std::{error::Error, fmt, time::Duration};

use reqwest::StatusCode;
use serde::Deserialize;

mod permissions;

use crate::{
    application::{
        AccessToken, AccountIdentity, DriveChild, DriveChildPage, DriveFolderLookupError,
        DriveFolderLookupPort, DriveFolderMetadata as AppFolderMetadata, DriveFolderOwner,
        DriveListFuture, DriveQuotaFuture, DriveQuotaPort, DriveTreePort, IdentityLookupError,
        IdentityLookupPort, StorageQuota,
    },
    domain::GooglePermissionId,
};

const API_BASE_URL: &str = "https://www.googleapis.com";
const ABOUT_PATH: &str =
    "/drive/v3/about?fields=user%28permissionId%2CemailAddress%2CdisplayName%29";
const ABOUT_QUOTA_PATH: &str = "/drive/v3/about?fields=storageQuota";
const LIST_FIELDS: &str = "nextPageToken,files(id,name,mimeType,parents,owners(permissionId,emailAddress),driveId,size,quotaBytesUsed,trashed,shortcutDetails,capabilities)";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const USER_AGENT: &str = concat!("gdom/", env!("CARGO_PKG_VERSION"));
const LIST_PAGE_SIZE: &str = "1000";

#[derive(Clone)]
pub struct GoogleDriveClient {
    client: reqwest::Client,
    base_url: String,
}

impl GoogleDriveClient {
    pub fn new() -> Result<Self, GoogleDriveError> {
        Self::build(API_BASE_URL.to_owned(), true)
    }

    #[cfg(test)]
    pub(crate) fn for_test(base_url: String) -> Result<Self, GoogleDriveError> {
        Self::build(base_url, false)
    }

    fn build(base_url: String, https_only: bool) -> Result<Self, GoogleDriveError> {
        let client = reqwest::Client::builder()
            .tls_backend_rustls()
            .https_only(https_only)
            .timeout(REQUEST_TIMEOUT)
            .user_agent(USER_AGENT)
            .build()
            .map_err(|_| GoogleDriveError::Transport)?;

        Ok(Self { client, base_url })
    }

    pub async fn account_identity(
        &self,
        token: &AccessToken,
    ) -> Result<DriveAccountIdentity, GoogleDriveError> {
        let response = self
            .client
            .get(format!("{}{ABOUT_PATH}", self.base_url))
            .bearer_auth(token.expose_secret())
            .send()
            .await
            .map_err(|_| GoogleDriveError::Transport)?;

        if !response.status().is_success() {
            return Err(Self::error_from_response(response).await);
        }

        let response = response
            .json::<AboutResponse>()
            .await
            .map_err(|_| GoogleDriveError::InvalidResponse)?;

        Ok(DriveAccountIdentity {
            permission_id: GooglePermissionId::new(response.user.permission_id),
            email: response.user.email_address,
            display_name: response.user.display_name,
        })
    }

    pub async fn get_folder_metadata(
        &self,
        token: &AccessToken,
        folder_id: &str,
    ) -> Result<DriveFolderMetadata, GoogleDriveError> {
        let fields = "id,name,mimeType,parents,owners(permissionId,emailAddress),driveId,trashed";
        let url = format!(
            "{}/drive/v3/files/{}?supportsAllDrives=true&fields={}",
            self.base_url,
            encode_path_segment(folder_id),
            fields
        );

        let response = self
            .client
            .get(url)
            .bearer_auth(token.expose_secret())
            .send()
            .await
            .map_err(|_| GoogleDriveError::Transport)?;

        if !response.status().is_success() {
            return Err(Self::error_from_response(response).await);
        }

        let raw = response
            .json::<RawFileResponse>()
            .await
            .map_err(|_| GoogleDriveError::InvalidResponse)?;

        let owners = raw
            .owners
            .unwrap_or_default()
            .into_iter()
            .map(|o| DriveFileOwner {
                permission_id: GooglePermissionId::new(o.permission_id),
                email_address: o.email_address,
            })
            .collect();

        Ok(DriveFolderMetadata {
            id: raw.id,
            name: raw.name,
            mime_type: raw.mime_type,
            trashed: raw.trashed.unwrap_or(false),
            drive_id: raw.drive_id,
            parents: raw.parents.unwrap_or_default(),
            owners,
        })
    }

    pub async fn list_children(
        &self,
        token: &AccessToken,
        folder_id: &str,
        page_token: Option<&str>,
    ) -> Result<DriveChildPage, GoogleDriveError> {
        let query = children_query(folder_id);
        let query_string = {
            let mut encoded = url::form_urlencoded::Serializer::new(String::new());
            encoded.append_pair("q", &query);
            encoded.append_pair("spaces", "drive");
            encoded.append_pair("pageSize", LIST_PAGE_SIZE);
            encoded.append_pair("supportsAllDrives", "true");
            encoded.append_pair("fields", LIST_FIELDS);
            if let Some(page_token) = page_token.filter(|token| !token.is_empty()) {
                encoded.append_pair("pageToken", page_token);
            }
            encoded.finish()
        };
        let url = format!("{}/drive/v3/files?{query_string}", self.base_url);

        let response = self
            .client
            .get(url)
            .bearer_auth(token.expose_secret())
            .send()
            .await
            .map_err(|_| GoogleDriveError::Transport)?;

        if !response.status().is_success() {
            return Err(Self::error_from_response(response).await);
        }

        let raw = response
            .json::<RawFileListResponse>()
            .await
            .map_err(|_| GoogleDriveError::InvalidResponse)?;

        Ok(DriveChildPage {
            files: raw
                .files
                .unwrap_or_default()
                .into_iter()
                .map(drive_child_from_raw)
                .collect(),
            next_page_token: raw.next_page_token,
        })
    }

    pub async fn storage_quota(
        &self,
        token: &AccessToken,
    ) -> Result<StorageQuota, GoogleDriveError> {
        let response = self
            .client
            .get(format!("{}{ABOUT_QUOTA_PATH}", self.base_url))
            .bearer_auth(token.expose_secret())
            .send()
            .await
            .map_err(|_| GoogleDriveError::Transport)?;

        if !response.status().is_success() {
            return Err(Self::error_from_response(response).await);
        }

        let raw = response
            .json::<AboutQuotaResponse>()
            .await
            .map_err(|_| GoogleDriveError::InvalidResponse)?;

        let quota = raw.storage_quota.unwrap_or(RawStorageQuota {
            limit: None,
            usage: None,
        });
        Ok(StorageQuota {
            limit_bytes: parse_u64_string(quota.limit),
            usage_bytes: parse_u64_string(quota.usage).unwrap_or(0),
        })
    }

    async fn error_from_response(response: reqwest::Response) -> GoogleDriveError {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        GoogleDriveError::from_status_and_body(status, &body)
    }
}

fn children_query(folder_id: &str) -> String {
    let escaped = folder_id.replace('\\', "\\\\").replace('\'', "\\'");
    format!("'{escaped}' in parents and trashed=false")
}

fn parse_i64_string(value: Option<String>) -> Option<i64> {
    value.and_then(|raw| raw.parse().ok())
}

fn parse_u64_string(value: Option<String>) -> Option<u64> {
    value.and_then(|raw| raw.parse().ok())
}

fn google_error_reason(body: &str) -> Option<String> {
    let parsed: RawGoogleErrorBody = serde_json::from_str(body).ok()?;
    parsed.error.and_then(|error| {
        error
            .errors
            .into_iter()
            .find_map(|item| item.reason.filter(|reason| !reason.is_empty()))
    })
}

fn drive_child_from_raw(raw: RawFileResponse) -> DriveChild {
    DriveChild {
        id: raw.id,
        name: raw.name,
        mime_type: raw.mime_type,
        parents: raw.parents.unwrap_or_default(),
        owners: raw
            .owners
            .unwrap_or_default()
            .into_iter()
            .map(|owner| DriveFolderOwner {
                permission_id: GooglePermissionId::new(owner.permission_id),
                email_address: owner.email_address,
            })
            .collect(),
        drive_id: raw.drive_id,
        quota_bytes_used: parse_i64_string(raw.quota_bytes_used.or(raw.size)),
        trashed: raw.trashed.unwrap_or(false),
        shortcut_target_id: raw.shortcut_details.and_then(|details| details.target_id),
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct DriveFileOwner {
    pub permission_id: GooglePermissionId,
    pub email_address: Option<String>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct DriveFolderMetadata {
    pub id: String,
    pub name: String,
    pub mime_type: String,
    pub trashed: bool,
    pub drive_id: Option<String>,
    pub parents: Vec<String>,
    pub owners: Vec<DriveFileOwner>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawFileResponse {
    id: String,
    name: String,
    mime_type: String,
    #[serde(default)]
    trashed: Option<bool>,
    #[serde(default)]
    drive_id: Option<String>,
    #[serde(default)]
    parents: Option<Vec<String>>,
    #[serde(default)]
    owners: Option<Vec<RawOwner>>,
    #[serde(default)]
    size: Option<String>,
    #[serde(default)]
    quota_bytes_used: Option<String>,
    #[serde(default)]
    shortcut_details: Option<RawShortcutDetails>,
    #[serde(default)]
    pub(super) permissions: Option<Vec<RawPermission>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawShortcutDetails {
    #[serde(default)]
    target_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawFileListResponse {
    #[serde(default)]
    next_page_token: Option<String>,
    #[serde(default)]
    files: Option<Vec<RawFileResponse>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AboutQuotaResponse {
    #[serde(default)]
    storage_quota: Option<RawStorageQuota>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawStorageQuota {
    #[serde(default)]
    limit: Option<String>,
    #[serde(default)]
    usage: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawOwner {
    permission_id: String,
    #[serde(default)]
    email_address: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPermission {
    id: String,
    #[serde(default, rename = "type")]
    type_: Option<String>,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    email_address: Option<String>,
    #[serde(default)]
    pending_owner: Option<bool>,
}

#[derive(Deserialize)]
struct RawGoogleErrorBody {
    #[serde(default)]
    error: Option<RawGoogleErrorInner>,
}

#[derive(Deserialize)]
struct RawGoogleErrorInner {
    #[serde(default)]
    errors: Vec<RawGoogleErrorItem>,
}

#[derive(Deserialize)]
struct RawGoogleErrorItem {
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct DriveAccountIdentity {
    permission_id: GooglePermissionId,
    email: String,
    display_name: String,
}

impl DriveAccountIdentity {
    pub fn new(
        permission_id: GooglePermissionId,
        email: impl Into<String>,
        display_name: impl Into<String>,
    ) -> Self {
        Self {
            permission_id,
            email: email.into(),
            display_name: display_name.into(),
        }
    }

    pub const fn permission_id(&self) -> &GooglePermissionId {
        &self.permission_id
    }

    pub fn email(&self) -> &str {
        &self.email
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GoogleDriveError {
    Unauthorized,
    Forbidden,
    NotFound,
    RateLimited,
    SharingRateLimitExceeded,
    StorageQuotaExceeded,
    ServerUnavailable,
    UnexpectedStatus(u16),
    Transport,
    InvalidResponse,
}

impl GoogleDriveError {
    fn from_status(status: StatusCode) -> Self {
        match status.as_u16() {
            401 => Self::Unauthorized,
            403 => Self::Forbidden,
            404 => Self::NotFound,
            429 => Self::RateLimited,
            500..=599 => Self::ServerUnavailable,
            code => Self::UnexpectedStatus(code),
        }
    }

    fn from_status_and_body(status: StatusCode, body: &str) -> Self {
        if let Some(reason) = google_error_reason(body) {
            match reason.as_str() {
                "sharingRateLimitExceeded" => return Self::SharingRateLimitExceeded,
                "storageQuotaExceeded" => return Self::StorageQuotaExceeded,
                "rateLimitExceeded" | "userRateLimitExceeded" => return Self::RateLimited,
                _ => {}
            }
        }
        Self::from_status(status)
    }
}

impl fmt::Display for GoogleDriveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unauthorized => formatter.write_str("Google Drive rejected the access token"),
            Self::Forbidden => formatter.write_str("Google Drive denied this request"),
            Self::NotFound => formatter.write_str("Google Drive file or folder not found"),
            Self::RateLimited => formatter.write_str("Google Drive rate limit reached"),
            Self::SharingRateLimitExceeded => {
                formatter.write_str("Google Drive sharing rate limit exceeded")
            }
            Self::StorageQuotaExceeded => {
                formatter.write_str("Google Drive storage quota exceeded")
            }
            Self::ServerUnavailable => formatter.write_str("Google Drive is unavailable"),
            Self::UnexpectedStatus(status) => {
                write!(
                    formatter,
                    "Google Drive returned unexpected status {status}"
                )
            }
            Self::Transport => formatter.write_str("Google Drive request failed"),
            Self::InvalidResponse => {
                formatter.write_str("Google Drive returned an invalid response")
            }
        }
    }
}

impl Error for GoogleDriveError {}

fn encode_path_segment(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(char::from(byte));
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

impl DriveTreePort for GoogleDriveClient {
    fn list_children<'a>(
        &'a self,
        token: &'a AccessToken,
        folder_id: &'a str,
        page_token: Option<&'a str>,
    ) -> DriveListFuture<'a> {
        Box::pin(async move { Ok(self.list_children(token, folder_id, page_token).await?) })
    }
}

impl DriveQuotaPort for GoogleDriveClient {
    fn get_storage_quota<'a>(&'a self, token: &'a AccessToken) -> DriveQuotaFuture<'a> {
        Box::pin(async move { Ok(self.storage_quota(token).await?) })
    }
}

impl DriveFolderLookupPort for GoogleDriveClient {
    fn get_folder_metadata<'a>(
        &'a self,
        token: &'a AccessToken,
        folder_id: &'a str,
    ) -> crate::application::drive_folder::DriveFolderLookupFuture<'a> {
        Box::pin(async move {
            let metadata = self.get_folder_metadata(token, folder_id).await?;
            Ok(AppFolderMetadata {
                id: metadata.id,
                name: metadata.name,
                mime_type: metadata.mime_type,
                trashed: metadata.trashed,
                drive_id: metadata.drive_id,
                owners: metadata
                    .owners
                    .into_iter()
                    .map(|owner| DriveFolderOwner {
                        permission_id: owner.permission_id,
                        email_address: owner.email_address,
                    })
                    .collect(),
            })
        })
    }
}

impl From<GoogleDriveError> for DriveFolderLookupError {
    fn from(error: GoogleDriveError) -> Self {
        match error {
            GoogleDriveError::Unauthorized => Self::Unauthorized,
            GoogleDriveError::Forbidden => Self::Forbidden,
            GoogleDriveError::NotFound => Self::NotFound,
            GoogleDriveError::RateLimited | GoogleDriveError::SharingRateLimitExceeded => {
                Self::RateLimited
            }
            GoogleDriveError::StorageQuotaExceeded => Self::Forbidden,
            GoogleDriveError::ServerUnavailable => Self::Unavailable,
            GoogleDriveError::Transport => Self::Transport,
            GoogleDriveError::InvalidResponse => Self::InvalidResponse,
            GoogleDriveError::UnexpectedStatus(status) => Self::UnexpectedStatus(status),
        }
    }
}

#[derive(Deserialize)]
struct AboutResponse {
    user: AboutUser,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AboutUser {
    permission_id: String,
    email_address: String,
    display_name: String,
}

impl IdentityLookupPort for GoogleDriveClient {
    async fn account_identity(
        &self,
        token: &AccessToken,
    ) -> Result<AccountIdentity, IdentityLookupError> {
        let identity = self.account_identity(token).await?;
        Ok(AccountIdentity::new(
            identity.permission_id().clone(),
            identity.email(),
            identity.display_name(),
        ))
    }
}

impl From<GoogleDriveError> for IdentityLookupError {
    fn from(error: GoogleDriveError) -> Self {
        match error {
            GoogleDriveError::Unauthorized => Self::Unauthorized,
            GoogleDriveError::Forbidden => Self::Forbidden,
            GoogleDriveError::NotFound => Self::UnexpectedStatus(404),
            GoogleDriveError::RateLimited | GoogleDriveError::SharingRateLimitExceeded => {
                Self::RateLimited
            }
            GoogleDriveError::StorageQuotaExceeded => Self::Forbidden,
            GoogleDriveError::ServerUnavailable => Self::Unavailable,
            GoogleDriveError::Transport => Self::Transport,
            GoogleDriveError::InvalidResponse => Self::InvalidResponse,
            GoogleDriveError::UnexpectedStatus(status) => Self::UnexpectedStatus(status),
        }
    }
}
