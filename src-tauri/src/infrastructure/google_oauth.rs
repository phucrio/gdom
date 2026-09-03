use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use oauth2::basic::BasicClient;
use oauth2::{
    AuthUrl, ClientId, CsrfToken, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope,
};
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinSet;
use tokio::time::timeout;

use super::oauth_callback::{self, CallbackOutcome};

const AUTHORIZATION_ENDPOINT: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const DRIVE_SCOPE: &str = "https://www.googleapis.com/auth/drive";
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(300);
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_IN_FLIGHT_CALLBACKS: usize = 16;

pub struct DesktopOAuthSession {
    listener: TcpListener,
    authorization_url: String,
    redirect_uri: String,
    expected_state: CsrfToken,
    pkce_verifier: PkceCodeVerifier,
    callback_timeout: Duration,
    connection_timeout: Duration,
}

impl DesktopOAuthSession {
    pub async fn start(client_id: &str) -> Result<Self, DesktopOAuthError> {
        Self::bind(client_id, CALLBACK_TIMEOUT, CONNECTION_TIMEOUT).await
    }

    #[cfg(test)]
    pub(super) async fn start_for_test(
        client_id: &str,
        callback_timeout: Duration,
    ) -> Result<Self, DesktopOAuthError> {
        Self::bind(client_id, callback_timeout, CONNECTION_TIMEOUT).await
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
        })
    }

    pub fn authorization_url(&self) -> &str {
        &self.authorization_url
    }

    pub fn redirect_uri(&self) -> &str {
        &self.redirect_uri
    }

    pub async fn receive_callback(self) -> Result<OAuthGrant, DesktopOAuthError> {
        let callback_timeout = self.callback_timeout;
        match timeout(callback_timeout, self.receive()).await {
            Ok(result) => result,
            Err(_) => Err(DesktopOAuthError::Timeout),
        }
    }

    async fn receive(self) -> Result<OAuthGrant, DesktopOAuthError> {
        let expected_state = Arc::new(self.expected_state);
        let pkce_verifier = Arc::new(self.pkce_verifier);
        let redirect_uri: Arc<str> = self.redirect_uri.into();
        let mut connections = JoinSet::new();

        loop {
            let completed = if connections.len() == MAX_IN_FLIGHT_CALLBACKS {
                connections.join_next().await
            } else {
                tokio::select! {
                    biased;
                    completed = connections.join_next(), if !connections.is_empty() => completed,
                    accepted = self.listener.accept() => {
                        let (stream, _) = accepted
                            .map_err(|_| DesktopOAuthError::ListenerUnavailable)?;
                        connections.spawn(handle_connection(
                            stream,
                            self.connection_timeout,
                            Arc::clone(&expected_state),
                            Arc::clone(&pkce_verifier),
                            Arc::clone(&redirect_uri),
                        ));
                        continue;
                    }
                }
            };

            let result = match completed {
                Some(Ok(result)) => result,
                Some(Err(_)) | None => return Err(DesktopOAuthError::ListenerUnavailable),
            };
            match result {
                Err(DesktopOAuthError::InvalidRequest | DesktopOAuthError::StateMismatch) => {}
                result => return result,
            }
        }
    }
}

async fn handle_connection(
    mut stream: TcpStream,
    connection_timeout: Duration,
    expected_state: Arc<CsrfToken>,
    pkce_verifier: Arc<PkceCodeVerifier>,
    redirect_uri: Arc<str>,
) -> Result<OAuthGrant, DesktopOAuthError> {
    let result = match timeout(connection_timeout, async {
        let request = oauth_callback::read_request(&mut stream).await?;
        let parameters = oauth_callback::parse(&request)?;
        let received_state = CsrfToken::new(parameters.state);
        if &received_state != expected_state.as_ref() {
            return Err(DesktopOAuthError::StateMismatch);
        }

        match parameters.outcome {
            CallbackOutcome::AuthorizationCode(authorization_code) => Ok(OAuthGrant {
                authorization_code,
                pkce_verifier: pkce_verifier.secret().to_owned(),
                redirect_uri: redirect_uri.to_string(),
            }),
            CallbackOutcome::ProviderError(error) if error == "access_denied" => {
                Err(DesktopOAuthError::AccessDenied)
            }
            CallbackOutcome::ProviderError(_) => Err(DesktopOAuthError::ProviderFailure),
        }
    })
    .await
    {
        Ok(result) => result,
        Err(_) => Err(DesktopOAuthError::InvalidRequest),
    };
    let response = match &result {
        Ok(_) => oauth_callback::success_response(),
        Err(_) => oauth_callback::error_response(),
    };
    drop(
        timeout(
            connection_timeout,
            stream.write_all(response.as_bytes()),
        )
        .await,
    );
    result
}

pub struct OAuthGrant {
    authorization_code: String,
    pkce_verifier: String,
    redirect_uri: String,
}

impl OAuthGrant {
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
