use std::sync::Arc;
use std::time::Duration;

use oauth2::{CsrfToken, PkceCodeVerifier};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
#[cfg(test)]
use tokio::sync::mpsc;
#[cfg(test)]
use tokio::time::sleep;
use tokio::time::timeout;

use super::google_oauth::{DesktopOAuthError, OAuthGrant};
use super::oauth_callback::{self, CallbackOutcome};

pub(super) struct CallbackContext {
    pub(super) connection_timeout: Duration,
    pub(super) expected_state: CsrfToken,
    pub(super) pkce_verifier: PkceCodeVerifier,
    pub(super) redirect_uri: String,
    #[cfg(test)]
    pub(super) fail_response_write: bool,
    #[cfg(test)]
    pub(super) handler_started: Option<mpsc::UnboundedSender<()>>,
    #[cfg(test)]
    pub(super) response_write_delay: Duration,
    #[cfg(test)]
    pub(super) response_write_started: Option<mpsc::UnboundedSender<()>>,
}

pub(super) enum ConnectionOutcome {
    Ignored,
    Terminal(TerminalCallback),
}

pub(super) struct TerminalCallback {
    stream: TcpStream,
    response: String,
    context: Arc<CallbackContext>,
    outcome: Result<OAuthGrant, DesktopOAuthError>,
}

impl TerminalCallback {
    pub(super) async fn respond(self) -> Result<OAuthGrant, DesktopOAuthError> {
        let Self {
            mut stream,
            response,
            context,
            outcome,
        } = self;
        preserve_outcome_after_write(outcome, write_response(&mut stream, &response, &context))
            .await
    }
}

pub(super) async fn handle_connection(
    mut stream: TcpStream,
    context: Arc<CallbackContext>,
) -> ConnectionOutcome {
    #[cfg(test)]
    if let Some(handler_started) = &context.handler_started {
        let _ = handler_started.send(());
    }

    let result = match timeout(context.connection_timeout, async {
        let request = oauth_callback::read_request(&mut stream).await?;
        let parameters = oauth_callback::parse(&request)?;
        let received_state = CsrfToken::new(parameters.state);
        if received_state != context.expected_state {
            return Err(DesktopOAuthError::StateMismatch);
        }

        match parameters.outcome {
            CallbackOutcome::AuthorizationCode(authorization_code) => Ok(OAuthGrant::new(
                authorization_code,
                context.pkce_verifier.secret().to_owned(),
                context.redirect_uri.clone(),
            )),
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
    if matches!(
        &result,
        Err(DesktopOAuthError::InvalidRequest | DesktopOAuthError::StateMismatch)
    ) {
        let _ = write_response(&mut stream, &response, &context).await;
        return ConnectionOutcome::Ignored;
    }
    ConnectionOutcome::Terminal(TerminalCallback {
        stream,
        response,
        context,
        outcome: result,
    })
}

async fn write_response(
    stream: &mut TcpStream,
    response: &str,
    context: &CallbackContext,
) -> Result<(), ()> {
    #[cfg(test)]
    if let Some(response_write_started) = &context.response_write_started {
        let _ = response_write_started.send(());
    }
    let fail_response_write = {
        #[cfg(test)]
        {
            context.fail_response_write
        }
        #[cfg(not(test))]
        {
            false
        }
    };
    timeout(context.connection_timeout, async {
        #[cfg(test)]
        sleep(context.response_write_delay).await;
        if fail_response_write {
            return Err(());
        }
        stream.write_all(response.as_bytes()).await.map_err(|_| ())
    })
    .await
    .map_err(|_| ())?
}

async fn preserve_outcome_after_write<T, E>(
    outcome: Result<T, E>,
    response_write: impl std::future::Future,
) -> Result<T, E> {
    let _ = response_write.await;
    outcome
}
