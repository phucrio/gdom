use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum UpdatePhase {
    Idle,
    Checking,
    UpToDate,
    Available,
    Downloading,
    Ready,
    Deferred,
    Installing,
    Error,
    Unavailable,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    pub phase: UpdatePhase,
    pub installed_version: String,
    pub target_version: Option<String>,
    pub notes: Option<String>,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub error: Option<String>,
}

impl UpdateStatus {
    pub fn new(installed_version: String) -> Self {
        Self {
            phase: UpdatePhase::Idle,
            installed_version,
            target_version: None,
            notes: None,
            downloaded_bytes: 0,
            total_bytes: None,
            error: None,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateConfirmation {
    pub confirmed: bool,
}

pub(super) fn update_error(error: &tauri_plugin_updater::Error) -> &'static str {
    use tauri_plugin_updater::Error;
    match error {
        Error::Minisign(_) | Error::Base64(_) | Error::SignatureUtf8(_) => {
            "Update signature verification failed. Keep using this version and retry checking for updates."
        }
        Error::Reqwest(_) | Error::Network(_) | Error::ReleaseNotFound => {
            "Could not reach the update service. Check your connection and try again."
        }
        Error::Serialization(_)
        | Error::Semver(_)
        | Error::TargetNotFound(_)
        | Error::TargetsNotFound(_) => {
            "The release does not contain a valid update for this device. Try checking again later."
        }
        _ => {
            "The update could not be completed. Keep using this version and try again. If installation failed, check storage space and permissions."
        }
    }
}
