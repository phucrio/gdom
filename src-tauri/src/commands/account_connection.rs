use std::{future::Future, sync::Arc};

use tauri::{AppHandle, Emitter};
use tokio::sync::{Mutex, OwnedMutexGuard, watch};

use crate::{
    domain::AccountId,
    infrastructure::google_oauth::DesktopOAuthSession,
    state::{AppState, OAuthConfig},
};

use super::{dto::AccountDto, error::CommandError};

const MAX_ATTEMPT_ID_LENGTH: usize = 128;

#[derive(Default)]
pub(crate) struct AccountConnections {
    pending: Mutex<Option<PendingConnection>>,
}

struct PendingConnection {
    attempt_id: String,
    cancellation: watch::Sender<bool>,
    completion: watch::Receiver<()>,
    reservation: Option<ConnectionAttempt>,
}

pub(crate) struct ConnectionAttempt {
    // Field order releases the authentication lock before signalling completion.
    _authentication_lock: OwnedMutexGuard<()>,
    _completion: watch::Sender<()>,
    cancellation: watch::Receiver<bool>,
}

impl AccountConnections {
    async fn begin(
        &self,
        attempt_id: String,
        authentication_lock: &Arc<Mutex<()>>,
    ) -> Result<(), CommandError> {
        if attempt_id.is_empty() || attempt_id.len() > MAX_ATTEMPT_ID_LENGTH {
            return Err(CommandError::OAuth(
                "Invalid account connection attempt ID".into(),
            ));
        }
        let mut pending = self.pending.lock().await;
        if let Some(connection) = pending.as_mut() {
            drop(connection.reservation.take());
        }
        let authentication_lock =
            Arc::clone(authentication_lock)
                .try_lock_owned()
                .map_err(|_| {
                    CommandError::OAuth("Another account connection is already in progress".into())
                })?;
        let (cancellation, cancellation_receiver) = watch::channel(false);
        let (completion_sender, completion) = watch::channel(());
        *pending = Some(PendingConnection {
            attempt_id,
            cancellation,
            completion,
            reservation: Some(ConnectionAttempt {
                _authentication_lock: authentication_lock,
                _completion: completion_sender,
                cancellation: cancellation_receiver,
            }),
        });
        Ok(())
    }

    async fn take(&self, attempt_id: &str) -> Result<ConnectionAttempt, CommandError> {
        self.pending
            .lock()
            .await
            .as_mut()
            .filter(|pending| pending.attempt_id == attempt_id)
            .and_then(|pending| pending.reservation.take())
            .ok_or_else(|| {
                CommandError::OAuth("Account connection attempt is no longer active".into())
            })
    }

    async fn cancel(&self, attempt_id: &str) {
        let completion = {
            let mut pending = self.pending.lock().await;
            let Some(connection) = pending
                .as_mut()
                .filter(|pending| pending.attempt_id == attempt_id)
            else {
                return;
            };
            connection.cancellation.send_replace(true);
            drop(connection.reservation.take());
            connection.completion.clone()
        };
        // Closure is the acknowledgement, including any token persistence already in flight.
        let mut completion = completion;
        let _ = completion.changed().await;
    }
}

impl ConnectionAttempt {
    async fn wait_for_authorization<T>(
        &mut self,
        authorization: impl Future<Output = Result<T, CommandError>>,
    ) -> Result<T, CommandError> {
        tokio::select! {
            biased;
            _ = self.cancellation.wait_for(|cancelled| *cancelled) => Err(CommandError::OAuth("Account connection cancelled".into())),
            result = authorization => result,
        }
    }
}

pub(super) async fn launch_authorization_browser(
    authorization_url: &str,
) -> Result<(), CommandError> {
    let authorization_url = authorization_url.to_owned();
    tokio::task::spawn_blocking(move || open::that(authorization_url))
        .await
        .map_err(|_| CommandError::BrowserLaunchFailed("Could not start browser launcher".into()))?
        .map_err(|_| {
            CommandError::BrowserLaunchFailed("Could not open the authorization browser".into())
        })
}

#[tauri::command]
pub async fn begin_account_connection(
    attempt_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), CommandError> {
    state
        .account_connections
        .begin(attempt_id, &state.connect_account_lock)
        .await
}

#[tauri::command]
pub async fn cancel_account_connection(
    attempt_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), CommandError> {
    state.account_connections.cancel(&attempt_id).await;
    Ok(())
}

#[tauri::command]
pub async fn connect_account(
    attempt_id: String,
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<AccountDto, CommandError> {
    let mut attempt = state.account_connections.take(&attempt_id).await?;
    let grant = attempt
        .wait_for_authorization(async {
            super::account::recover_custom_oauth_secret(&state).await?;
            let config = state
                .oauth_config
                .read()
                .await
                .clone()
                .unwrap_or_else(OAuthConfig::default_config);
            super::account::require_desktop_client_secret(&config)?;
            let session = DesktopOAuthSession::start(&config.client_id)
                .await
                .map_err(|error| CommandError::OAuth(error.to_string()))?;
            launch_authorization_browser(session.authorization_url()).await?;
            session
                .receive_callback()
                .await
                .map_err(|error| CommandError::OAuth(error.to_string()))
        })
        .await?;

    // Once Google returned the grant, finish exchange and persistence even if the dialog closes.
    let fallback_id = AccountId::new(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(1),
    );
    let account = state
        .connect_account_use_case
        .connect_account(grant, fallback_id)
        .await?;
    let _ = app.emit("account-registry-changed", ());
    Ok(AccountDto::from(account))
}

#[cfg(test)]
#[path = "account_connection_test.rs"]
mod tests;
