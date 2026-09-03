use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use oauth2::basic::BasicClient;
use oauth2::{
    AuthUrl, ClientId, CsrfToken, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope,
};
use tokio::net::TcpListener;
#[cfg(test)]
use tokio::sync::mpsc;
use tokio::time::timeout;

use super::oauth_connection::{CallbackContext, TerminalCallback};
use super::oauth_listener;

const AUTHORIZATION_ENDPOINT: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const DRIVE_SCOPE: &str = "https://www.googleapis.com/auth/drive";
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(300);
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(2);
/// A continuously hostile same-host client can still deny OAuth by racing every replacement.
pub(super) const MAX_IN_FLIGHT_CALLBACKS: usize = 16;

pub struct DesktopOAuthSession {
    listener: TcpListener,
    authorization_url: String,
    redirect_uri: String,
    expected_state: CsrfToken,
    pkce_verifier: PkceCodeVerifier,
    callback_timeout: Duration,
    connection_timeout: Duration,
    #[cfg(test)]
    fail_response_write: bool,
    #[cfg(test)]
    handler_started: Option<mpsc::UnboundedSender<()>>,
    #[cfg(test)]
    response_write_delay: Duration,
    #[cfg(test)]
    response_write_started: Option<mpsc::UnboundedSender<()>>,
}

impl DesktopOAuthSession {
    pub async fn start(client_id: &str) -> Result<Self, DesktopOAuthError> {
        Self::bind(client_id, CALLBACK_TIMEOUT, CONNECTION_TIMEOUT).await
    }

    #[cfg(test)]
    pub(super) async fn start_for_test_with_timeouts(
        client_id: &str,
        callback_timeout: Duration,
        connection_timeout: Duration,
    ) -> Result<Self, DesktopOAuthError> {
        Self::bind(client_id, callback_timeout, connection_timeout).await
    }

    async fn bind(
        client_id: &str,
        callback_timeout: Duration,
        connection_timeout: Duration,
    ) -> Result<Self, DesktopOAuthError> {
        if client_id.trim().is_empty() {
            return Err(DesktopOAuthError::InvalidConfiguration);
        }

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|_| DesktopOAuthError::ListenerUnavailable)?;
        let redirect_uri = format!(
            "http://{}",
            listener
                .local_addr()
                .map_err(|_| DesktopOAuthError::ListenerUnavailable)?
        );
        let client = BasicClient::new(ClientId::new(client_id.to_owned()))
            .set_auth_uri(
                AuthUrl::new(AUTHORIZATION_ENDPOINT.to_owned())
                    .map_err(|_| DesktopOAuthError::InvalidConfiguration)?,
            )
            .set_redirect_uri(
                RedirectUrl::new(redirect_uri.clone())
                    .map_err(|_| DesktopOAuthError::InvalidConfiguration)?,
            );
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
        let (authorization_url, expected_state) = client
            .authorize_url(CsrfToken::new_random)
            .add_scope(Scope::new(DRIVE_SCOPE.to_owned()))
            .add_extra_param("access_type", "offline")
            .add_extra_param("prompt", "select_account")
            .set_pkce_challenge(pkce_challenge)
            .url();

        Ok(Self {
            listener,
            authorization_url: authorization_url.to_string(),
            redirect_uri,
            expected_state,
            pkce_verifier,
            callback_timeout,
            connection_timeout,
            #[cfg(test)]
            fail_response_write: false,
            #[cfg(test)]
            handler_started: None,
            #[cfg(test)]
            response_write_delay: Duration::ZERO,
            #[cfg(test)]
            response_write_started: None,
        })
    }

    pub fn authorization_url(&self) -> &str {
        &self.authorization_url
    }

    pub fn redirect_uri(&self) -> &str {
        &self.redirect_uri
    }

    #[cfg(test)]
    pub(super) fn fail_response_write_for_test(mut self) -> Self {
        self.fail_response_write = true;
        self
    }

    #[cfg(test)]
    pub(super) fn notify_handler_started_for_test(mut self) -> (Self, mpsc::UnboundedReceiver<()>) {
        let (handler_started, receiver) = mpsc::unbounded_channel();
        self.handler_started = Some(handler_started);
        (self, receiver)
    }

    #[cfg(test)]
    pub(super) fn delay_response_write_for_test(
        mut self,
        delay: Duration,
    ) -> (Self, mpsc::UnboundedReceiver<()>) {
        let (response_write_started, receiver) = mpsc::unbounded_channel();
        self.response_write_delay = delay;
        self.response_write_started = Some(response_write_started);
        (self, receiver)
    }

    pub async fn receive_callback(self) -> Result<OAuthGrant, DesktopOAuthError> {
        let callback_timeout = self.callback_timeout;
        match timeout(callback_timeout, self.receive()).await {
            Ok(Ok(callback)) => callback.respond().await,
            Ok(Err(error)) => Err(error),
            Err(_) => Err(DesktopOAuthError::Timeout),
        }
    }

    async fn receive(self) -> Result<TerminalCallback, DesktopOAuthError> {
        let context = Arc::new(CallbackContext {
            connection_timeout: self.connection_timeout,
            expected_state: self.expected_state,
            pkce_verifier: self.pkce_verifier,
            redirect_uri: self.redirect_uri,
            #[cfg(test)]
            fail_response_write: self.fail_response_write,
            #[cfg(test)]
            handler_started: self.handler_started,
            #[cfg(test)]
            response_write_delay: self.response_write_delay,
            #[cfg(test)]
            response_write_started: self.response_write_started,
        });
        oauth_listener::receive(self.listener, context).await
    }
}

pub struct OAuthGrant {
    authorization_code: String,
    pkce_verifier: String,
    redirect_uri: String,
}

impl OAuthGrant {
    pub(super) fn new(
        authorization_code: String,
        pkce_verifier: String,
        redirect_uri: String,
    ) -> Self {
        Self {
            authorization_code,
            pkce_verifier,
            redirect_uri,
        }
    }

    pub fn authorization_code(&self) -> &str {
        &self.authorization_code
    }

    pub fn pkce_verifier(&self) -> &str {
        &self.pkce_verifier
    }

    pub fn redirect_uri(&self) -> &str {
        &self.redirect_uri
    }
}

impl fmt::Debug for OAuthGrant {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("OAuthGrant([REDACTED])")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopOAuthError {
    InvalidConfiguration,
    ListenerUnavailable,
    Timeout,
    InvalidRequest,
    StateMismatch,
    AccessDenied,
    ProviderFailure,
}

impl fmt::Display for DesktopOAuthError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfiguration => formatter.write_str("OAuth configuration is invalid"),
            Self::ListenerUnavailable => formatter.write_str("OAuth callback listener failed"),
            Self::Timeout => formatter.write_str("OAuth callback timed out"),
            Self::InvalidRequest => formatter.write_str("OAuth callback request is invalid"),
            Self::StateMismatch => formatter.write_str("OAuth callback state did not match"),
            Self::AccessDenied => formatter.write_str("Google authorization was denied"),
            Self::ProviderFailure => formatter.write_str("Google authorization failed"),
        }
    }
}

impl std::error::Error for DesktopOAuthError {}
