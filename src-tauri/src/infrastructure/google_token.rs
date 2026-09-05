use std::{error::Error, fmt, time::Duration};

use reqwest::StatusCode;
use serde::Deserialize;
use url::form_urlencoded;

use crate::application::{
    AccessToken, OAuthGrant, RefreshFuture, RefreshToken, TokenExchangeError, TokenExchangePort,
    TokenRefreshError, TokenRefreshPort, TokenResponse,
};

const TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com";
const TOKEN_PATH: &str = "/token";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const USER_AGENT: &str = concat!("gdom/", env!("CARGO_PKG_VERSION"));

pub struct GoogleTokenClient {
    client: reqwest::Client,
    base_url: String,
    client_id: String,
    client_secret: Option<String>,
}

impl GoogleTokenClient {
    pub fn new(client_id: String, client_secret: Option<String>) -> Result<Self, GoogleTokenError> {
        Self::build(TOKEN_ENDPOINT.to_owned(), client_id, client_secret, true)
    }

    #[cfg(test)]
    pub(super) fn for_test(
        base_url: String,
        client_id: String,
        client_secret: Option<String>,
    ) -> Result<Self, GoogleTokenError> {
        Self::build(base_url, client_id, client_secret, false)
    }

    fn build(
        base_url: String,
        client_id: String,
        client_secret: Option<String>,
        https_only: bool,
    ) -> Result<Self, GoogleTokenError> {
        let client = reqwest::Client::builder()
            .tls_backend_rustls()
            .https_only(https_only)
            .timeout(REQUEST_TIMEOUT)
            .user_agent(USER_AGENT)
            .build()
            .map_err(|_| GoogleTokenError::Transport)?;

        Ok(Self {
            client,
            base_url,
            client_id,
            client_secret,
        })
    }

    pub async fn exchange_code(
        &self,
        grant: OAuthGrant,
    ) -> Result<GoogleTokenResponse, GoogleTokenError> {
        let body = {
            let mut form = form_urlencoded::Serializer::new(String::new());
            form.append_pair("grant_type", "authorization_code");
            form.append_pair("code", grant.authorization_code());
            form.append_pair("code_verifier", grant.pkce_verifier());
            form.append_pair("client_id", &self.client_id);
            form.append_pair("redirect_uri", grant.redirect_uri());

            if let Some(secret) = &self.client_secret {
                form.append_pair("client_secret", secret);
            }

            form.finish()
        };

        let response = self
            .client
            .post(format!("{}{TOKEN_PATH}", self.base_url))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .map_err(|_| GoogleTokenError::Transport)?;

        let status = response.status();
        if !status.is_success() {
            if let Ok(error_body) = response.json::<OAuthErrorResponse>().await {
                return Err(GoogleTokenError::from_oauth_error(
                    &error_body.error,
                    status,
                ));
            }
            return Err(GoogleTokenError::from_status(status));
        }

        let raw_token = response
            .json::<RawTokenResponse>()
            .await
            .map_err(|_| GoogleTokenError::InvalidResponse)?;

        Ok(GoogleTokenResponse {
            access_token: AccessToken::new(raw_token.access_token),
            expires_in: Duration::from_secs(raw_token.expires_in),
            refresh_token: raw_token.refresh_token.map(RefreshToken::new),
            token_type: raw_token.token_type,
            scope: raw_token.scope,
        })
    }

    pub async fn refresh_token(
        &self,
        refresh_token: &RefreshToken,
    ) -> Result<GoogleTokenResponse, GoogleTokenError> {
        let body = {
            let mut form = form_urlencoded::Serializer::new(String::new());
            form.append_pair("grant_type", "refresh_token");
            form.append_pair("refresh_token", refresh_token.expose_secret());
            form.append_pair("client_id", &self.client_id);

            if let Some(secret) = &self.client_secret {
                form.append_pair("client_secret", secret);
            }

            form.finish()
        };

        let response = self
            .client
            .post(format!("{}{TOKEN_PATH}", self.base_url))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .map_err(|_| GoogleTokenError::Transport)?;

        let status = response.status();
        if !status.is_success() {
            if let Ok(error_body) = response.json::<OAuthErrorResponse>().await {
                return Err(GoogleTokenError::from_oauth_error(
                    &error_body.error,
                    status,
                ));
            }
            return Err(GoogleTokenError::from_status(status));
        }

        let raw_token = response
            .json::<RawTokenResponse>()
            .await
            .map_err(|_| GoogleTokenError::InvalidResponse)?;

        Ok(GoogleTokenResponse {
            access_token: AccessToken::new(raw_token.access_token),
            expires_in: Duration::from_secs(raw_token.expires_in),
            refresh_token: raw_token.refresh_token.map(RefreshToken::new),
            token_type: raw_token.token_type,
            scope: raw_token.scope,
        })
    }
}

pub struct GoogleTokenResponse {
    pub access_token: AccessToken,
    pub expires_in: Duration,
    pub refresh_token: Option<RefreshToken>,
    pub token_type: String,
    pub scope: Option<String>,
}

impl fmt::Debug for GoogleTokenResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GoogleTokenResponse")
            .field("access_token", &"[REDACTED]")
            .field("expires_in", &self.expires_in)
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "[REDACTED]"),
            )
            .field("token_type", &self.token_type)
            .field("scope", &self.scope)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GoogleTokenError {
    InvalidGrant,
    InvalidClient,
    RateLimited,
    ServerUnavailable,
    UnexpectedStatus(u16),
    Transport,
    InvalidResponse,
}

impl GoogleTokenError {
    fn from_oauth_error(error: &str, status: StatusCode) -> Self {
        match error {
            "invalid_grant" => Self::InvalidGrant,
            "invalid_client" => Self::InvalidClient,
            _ => Self::from_status(status),
        }
    }

    fn from_status(status: StatusCode) -> Self {
        match status.as_u16() {
            401 => Self::InvalidClient,
            429 => Self::RateLimited,
            500..=599 => Self::ServerUnavailable,
            code => Self::UnexpectedStatus(code),
        }
    }
}

impl fmt::Display for GoogleTokenError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidGrant => {
                formatter.write_str("Google rejected the authorization code or PKCE verifier")
            }
            Self::InvalidClient => {
                formatter.write_str("Google rejected the OAuth client credentials")
            }
            Self::RateLimited => formatter.write_str("Google token endpoint rate limit reached"),
            Self::ServerUnavailable => formatter.write_str("Google token endpoint is unavailable"),
            Self::UnexpectedStatus(status) => {
                write!(
                    formatter,
                    "Google token endpoint returned unexpected status {status}"
                )
            }
            Self::Transport => formatter.write_str("Google token request failed"),
            Self::InvalidResponse => {
                formatter.write_str("Google token endpoint returned an invalid response")
            }
        }
    }
}

impl Error for GoogleTokenError {}

#[derive(Deserialize)]
struct RawTokenResponse {
    access_token: String,
    expires_in: u64,
    refresh_token: Option<String>,
    token_type: String,
    scope: Option<String>,
}

#[derive(Deserialize)]
struct OAuthErrorResponse {
    error: String,
}

impl TokenExchangePort for GoogleTokenClient {
    async fn exchange_code(&self, grant: OAuthGrant) -> Result<TokenResponse, TokenExchangeError> {
        let response = self.exchange_code(grant).await?;
        Ok(TokenResponse::new(
            response.access_token,
            response.refresh_token,
        ))
    }
}

impl From<GoogleTokenError> for TokenExchangeError {
    fn from(error: GoogleTokenError) -> Self {
        match error {
            GoogleTokenError::InvalidGrant => Self::InvalidGrant,
            GoogleTokenError::InvalidClient => Self::InvalidClient,
            GoogleTokenError::RateLimited => Self::RateLimited,
            GoogleTokenError::ServerUnavailable => Self::Unavailable,
            GoogleTokenError::Transport => Self::Transport,
            GoogleTokenError::InvalidResponse => Self::InvalidResponse,
            GoogleTokenError::UnexpectedStatus(status) => Self::UnexpectedStatus(status),
        }
    }
}

impl From<GoogleTokenError> for TokenRefreshError {
    fn from(err: GoogleTokenError) -> Self {
        match err {
            GoogleTokenError::InvalidGrant => Self::InvalidGrant,
            GoogleTokenError::InvalidClient => Self::InvalidClient,
            GoogleTokenError::RateLimited => Self::RateLimited,
            GoogleTokenError::ServerUnavailable => Self::Unavailable,
            GoogleTokenError::Transport => Self::Transport,
            GoogleTokenError::InvalidResponse => Self::InvalidResponse,
            GoogleTokenError::UnexpectedStatus(code) => Self::UnexpectedStatus(code),
        }
    }
}

pub struct DynamicGoogleTokenClient {
    oauth_config: std::sync::Arc<tokio::sync::RwLock<Option<crate::state::OAuthConfig>>>,
}

impl DynamicGoogleTokenClient {
    pub fn new(
        oauth_config: std::sync::Arc<tokio::sync::RwLock<Option<crate::state::OAuthConfig>>>,
    ) -> Self {
        Self { oauth_config }
    }
}

impl TokenExchangePort for DynamicGoogleTokenClient {
    async fn exchange_code(&self, grant: OAuthGrant) -> Result<TokenResponse, TokenExchangeError> {
        let config = {
            let guard = self.oauth_config.read().await;
            guard.clone()
        };
        let config = config.ok_or(TokenExchangeError::InvalidClient)?;
        let client = GoogleTokenClient::new(config.client_id, config.client_secret)
            .map_err(|_| TokenExchangeError::Transport)?;
        TokenExchangePort::exchange_code(&client, grant).await
    }
}

impl TokenRefreshPort for DynamicGoogleTokenClient {
    fn refresh_token(&self, refresh_token: &RefreshToken) -> RefreshFuture<'_> {
        let refresh_token = refresh_token.clone();
        Box::pin(async move {
            let config = {
                let guard = self.oauth_config.read().await;
                guard.clone()
            };
            let config = config.ok_or(TokenRefreshError::InvalidClient)?;
            let client = GoogleTokenClient::new(config.client_id, config.client_secret)
                .map_err(|_| TokenRefreshError::Transport)?;
            let response = client
                .refresh_token(&refresh_token)
                .await
                .map_err(TokenRefreshError::from)?;
            Ok((response.access_token, response.expires_in))
        })
    }
}
