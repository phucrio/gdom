#[path = "update_policy.rs"]
mod policy;
#[path = "update_status.rs"]
mod status;

use std::{sync::Arc, time::Duration};
use tauri::{AppHandle, State};
use tauri_plugin_updater::{Update, UpdaterExt};
use tokio::sync::{Mutex, watch};

use super::error::CommandError;
use crate::{application::JobServiceError, state::AppState};
pub use status::{UpdateConfirmation, UpdatePhase, UpdateStatus};

const CHECK_TIMEOUT: Duration = Duration::from_secs(20);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(15 * 60);

enum UpdateSession {
    Empty,
    Available(Update),
    Ready(Update, Arc<[u8]>),
}

pub struct UpdateState {
    session: Mutex<UpdateSession>,
    status: watch::Sender<UpdateStatus>,
}

impl UpdateState {
    pub fn new(installed_version: String) -> Self {
        Self {
            session: Mutex::new(UpdateSession::Empty),
            status: watch::channel(UpdateStatus::new(installed_version)).0,
        }
    }

    fn snapshot(&self) -> UpdateStatus {
        self.status.borrow().clone()
    }

    fn set_phase(&self, phase: UpdatePhase, error: Option<&str>) -> UpdateStatus {
        self.status.send_modify(|status| {
            status.phase = phase;
            status.error = error.map(str::to_owned);
        });
        self.snapshot()
    }
}

fn require_confirmation(input: &UpdateConfirmation) -> Result<(), CommandError> {
    if !input.confirmed {
        return Err(CommandError::ConfirmationRequired(
            "Confirm this update action first.".into(),
        ));
    }
    Ok(())
}

fn busy_error() -> CommandError {
    CommandError::Internal("Another update action is in progress. Please wait.".into())
}

#[tauri::command]
pub fn get_update_status(updates: State<'_, UpdateState>) -> UpdateStatus {
    updates.snapshot()
}

#[tauri::command]
pub async fn check_for_updates(
    app: AppHandle,
    updates: State<'_, UpdateState>,
) -> Result<UpdateStatus, CommandError> {
    let mut session = updates.session.try_lock().map_err(|_| busy_error())?;
    let Some(public_key) =
        option_env!("GDOM_UPDATER_PUBLIC_KEY").filter(|key| !key.trim().is_empty())
    else {
        return Ok(updates.set_phase(
            UpdatePhase::Unavailable,
            Some("Updates are not configured for this build."),
        ));
    };
    *session = UpdateSession::Empty;
    updates.status.send_modify(|status| {
        *status = UpdateStatus::new(status.installed_version.clone());
        status.phase = UpdatePhase::Checking;
    });
    let result = async {
        let updater = app
            .updater_builder()
            .pubkey(public_key)
            .timeout(CHECK_TIMEOUT)
            .configure_client(|client| client.https_only(true))
            .version_comparator(|current, release| {
                policy::is_newer_stable(&current, &release.version)
            })
            .build()?;
        updater.check().await
    }
    .await;
    match result {
        Ok(Some(mut update)) => {
            if !policy::is_release_artifact(&update.download_url, &update.version)
                || update.signature.trim().is_empty()
            {
                return Ok(updates.set_phase(UpdatePhase::Error, Some("The update manifest contains an invalid release artifact or signature. Try again later.")));
            }
            update.timeout = Some(DOWNLOAD_TIMEOUT);
            updates.status.send_modify(|status| {
                status.phase = UpdatePhase::Available;
                status.target_version = Some(update.version.clone());
                status.notes = update.body.clone();
            });
            *session = UpdateSession::Available(update);
        }
        Ok(None) => {
            updates.set_phase(UpdatePhase::UpToDate, None);
        }
        Err(error) => {
            updates.set_phase(UpdatePhase::Error, Some(status::update_error(&error)));
        }
    }
    Ok(updates.snapshot())
}

#[tauri::command]
pub async fn download_update(
    updates: State<'_, UpdateState>,
    input: UpdateConfirmation,
) -> Result<UpdateStatus, CommandError> {
    require_confirmation(&input)?;
    let mut session = updates.session.try_lock().map_err(|_| busy_error())?;
    let update = match &*session {
        UpdateSession::Available(update) => update.clone(),
        UpdateSession::Ready(_, _) => return Ok(updates.snapshot()),
        UpdateSession::Empty => {
            return Err(CommandError::Internal(
                "Check for an update before downloading.".into(),
            ));
        }
    };
    updates.status.send_modify(|status| {
        status.phase = UpdatePhase::Downloading;
        status.error = None;
        status.downloaded_bytes = 0;
        status.total_bytes = None;
    });
    let result = update
        .download(
            |bytes, total| {
                updates.status.send_modify(|status| {
                    status.downloaded_bytes = status
                        .downloaded_bytes
                        .saturating_add(u64::try_from(bytes).unwrap_or(u64::MAX));
                    status.total_bytes = total;
                });
            },
            || {},
        )
        .await;
    match result {
        Ok(bytes) => {
            // Only the plugin's verified download can enter the installable state.
            *session = UpdateSession::Ready(update, bytes.into());
            Ok(updates.set_phase(UpdatePhase::Ready, None))
        }
        Err(error) => Ok(updates.set_phase(UpdatePhase::Error, Some(status::update_error(&error)))),
    }
}

#[tauri::command]
pub async fn install_update(
    app: AppHandle,
    state: State<'_, AppState>,
    updates: State<'_, UpdateState>,
    input: UpdateConfirmation,
) -> Result<UpdateStatus, CommandError> {
    require_confirmation(&input)?;
    let session = updates.session.try_lock().map_err(|_| busy_error())?;
    let (update, bytes) = match &*session {
        UpdateSession::Ready(update, bytes) => (update.clone(), Arc::clone(bytes)),
        UpdateSession::Empty | UpdateSession::Available(_) => {
            return Err(CommandError::Internal(
                "Download and verify the update before installing.".into(),
            ));
        }
    };
    let guard = match state.job_service.try_begin_update_installation().await {
        Ok(guard) => guard,
        Err(JobServiceError::UpdateInstallationBusy | JobServiceError::UpdateInstallationInProgress) =>
            return Ok(updates.set_phase(UpdatePhase::Deferred, Some("A job is still active. Wait for it to finish or pause it, then confirm installation again."))),
        Err(_) => return Ok(updates.set_phase(UpdatePhase::Error, Some("Could not verify saved job checkpoints. Retry after resolving the job error."))),
    };
    updates.set_phase(UpdatePhase::Installing, None);
    let result =
        tauri::async_runtime::spawn_blocking(move || -> Result<(), tauri_plugin_updater::Error> {
            // Retain the exclusive gate even if the IPC future is dropped mid-install.
            let _guard = guard;
            update.install(bytes)?;
            app.restart()
        })
        .await;
    match result {
        Ok(Err(error)) => {
            Ok(updates.set_phase(UpdatePhase::Error, Some(status::update_error(&error))))
        }
        Err(_) => Ok(updates.set_phase(
            UpdatePhase::Error,
            Some("Installation stopped unexpectedly. Restart this app and try again."),
        )),
        Ok(Ok(())) => Ok(updates.snapshot()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_and_install_require_explicit_confirmation() {
        assert!(require_confirmation(&UpdateConfirmation { confirmed: false }).is_err());
        assert!(require_confirmation(&UpdateConfirmation { confirmed: true }).is_ok());
    }

    #[test]
    fn progress_is_visible_without_waiting_for_session_lock() {
        let state = UpdateState::new("0.1.0".into());
        let _operation = state.session.try_lock().unwrap();
        state.set_phase(UpdatePhase::Downloading, None);
        assert_eq!(state.snapshot().phase, UpdatePhase::Downloading);
        assert!(state.session.try_lock().is_err());
    }
}

#[cfg(test)]
#[path = "update_transport_test.rs"]
mod transport_tests;
