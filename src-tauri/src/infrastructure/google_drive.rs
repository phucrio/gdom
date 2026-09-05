use std::{error::Error, fmt, time::Duration};

use reqwest::StatusCode;
use serde::Deserialize;

use crate::{
    application::{AccessToken, AccountIdentity, IdentityLookupError, IdentityLookupPort},
    domain::GooglePermissionId,
};

const API_BASE_URL: &str = "https://www.googleapis.com";
const ABOUT_PATH: &str =
    "/drive/v3/about?fields=user%28permissionId%2CemailAddress%2CdisplayName%29";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const USER_AGENT: &str = concat!("gdom/", env!("CARGO_PKG_VERSION"));

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
    pub(super) fn for_test(base_url: String) -> Result<Self, GoogleDriveError> {
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
            return Err(GoogleDriveError::from_status(response.status()));
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
    RateLimited,
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
            429 => Self::RateLimited,
            500..=599 => Self::ServerUnavailable,
            code => Self::UnexpectedStatus(code),
        }
    }
}

impl fmt::Display for GoogleDriveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unauthorized => formatter.write_str("Google Drive rejected the access token"),
            Self::Forbidden => formatter.write_str("Google Drive denied this request"),
            Self::RateLimited => formatter.write_str("Google Drive rate limit reached"),
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
            GoogleDriveError::RateLimited => Self::RateLimited,
            GoogleDriveError::ServerUnavailable => Self::Unavailable,
            GoogleDriveError::Transport => Self::Transport,
            GoogleDriveError::InvalidResponse => Self::InvalidResponse,
            GoogleDriveError::UnexpectedStatus(status) => Self::UnexpectedStatus(status),
        }
    }
}
