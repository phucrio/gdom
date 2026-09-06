use std::{env, fmt, sync::Arc};

use tokio::sync::RwLock;

use crate::infrastructure::account_store::SqliteAccountStore;

#[cfg(target_os = "windows")]
use crate::infrastructure::secrets::WindowsCredentialStore;

#[cfg(not(target_os = "windows"))]
use crate::application::RefreshTokenStore;

// ---------------------------------------------------------------------------
// OAuthConfig
// ---------------------------------------------------------------------------

/// Public desktop OAuth client ID shipped for one-click Google sign-in.
/// Override at compile time with `GDOM_DEFAULT_CLIENT_ID`.
pub const DEFAULT_GOOGLE_CLIENT_ID: &str =
    "1004841450211-1hhs43nbpqu8vklbe2d681t3rg2g9vso.apps.googleusercontent.com";

/// Google's token endpoint currently requires the Desktop-app client secret.
/// The secret must not live in git; import JSON, set `GDOM_GOOGLE_CLIENT_SECRET`,
/// or inject `GDOM_DEFAULT_CLIENT_SECRET` at compile time for release builds.
pub const DESKTOP_CLIENT_SECRET_REQUIRED: &str = "Google Desktop OAuth clients require a client secret. Import the Desktop client JSON from Google Cloud Console, or set GDOM_GOOGLE_CLIENT_SECRET. GDOM stores the secret in Windows Credential Manager; it is never kept in source or shown in the UI.";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OAuthClientSource {
    CustomOverride,
    Environment,
    EmbeddedDefault,
}

/// Google OAuth client credentials loaded at startup.
///
/// `client_secret` is deliberately redacted from `Debug` output to prevent
/// accidental exposure in logs. Desktop clients are public and typically have
/// no secret (RFC 8252).
#[derive(Clone)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: Option<String>,
}

impl fmt::Debug for OAuthConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OAuthConfig")
            .field("client_id", &self.client_id)
            .field("client_secret", &"[REDACTED]")
            .finish()
    }
}

impl OAuthConfig {
    pub fn new(client_id: impl Into<String>, client_secret: Option<String>) -> Self {
        Self {
            client_id: client_id.into(),
            client_secret,
        }
    }

    pub fn embedded_client_id() -> &'static str {
        match option_env!("GDOM_DEFAULT_CLIENT_ID") {
            Some(id) if !id.is_empty() => id,
            _ => DEFAULT_GOOGLE_CLIENT_ID,
        }
    }

    pub fn embedded_client_secret() -> Option<String> {
        match option_env!("GDOM_DEFAULT_CLIENT_SECRET") {
            Some(secret) if !secret.is_empty() => Some(secret.to_owned()),
            _ => None,
        }
    }

    pub fn default_config() -> Self {
        Self::new(Self::embedded_client_id(), Self::embedded_client_secret())
    }

    pub fn has_client_secret(&self) -> bool {
        self.client_secret
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
    }

    /// Custom SQLite override, then env, then the embedded default.
    ///
    /// `GDOM_GOOGLE_CLIENT_SECRET` may pair with an env client ID or, when the
    /// client ID is unset, with the embedded public client ID. The secret is
    /// never compiled into source unless `GDOM_DEFAULT_CLIENT_SECRET` is set at
    /// build time.
    pub fn resolve(
        stored_client_id: Option<&str>,
        stored_secret: Option<String>,
        env_lookup: impl Fn(&str) -> Result<String, env::VarError>,
    ) -> (Self, OAuthClientSource) {
        if let Some(id) = stored_client_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return (
                Self::new(id, stored_secret),
                OAuthClientSource::CustomOverride,
            );
        }
        if let Some(from_env) = Self::from_env_lookup(env_lookup) {
            return (from_env, OAuthClientSource::Environment);
        }
        (Self::default_config(), OAuthClientSource::EmbeddedDefault)
    }

    pub fn from_env() -> Option<Self> {
        Self::from_env_lookup(|key| env::var(key))
    }

    fn from_env_lookup(lookup: impl Fn(&str) -> Result<String, env::VarError>) -> Option<Self> {
        let client_id = lookup("GDOM_GOOGLE_CLIENT_ID")
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        let client_secret = lookup("GDOM_GOOGLE_CLIENT_SECRET")
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        match (client_id, client_secret) {
            (Some(client_id), client_secret) => Some(Self::new(client_id, client_secret)),
            (None, Some(client_secret)) => {
                Some(Self::new(Self::embedded_client_id(), Some(client_secret)))
            }
            (None, None) => None,
        }
    }
}

// ---------------------------------------------------------------------------
// AppState
// ---------------------------------------------------------------------------

pub struct AppState {
    pub account_store: Arc<SqliteAccountStore>,

    #[cfg(target_os = "windows")]
    pub credential_store: Arc<WindowsCredentialStore>,

    #[cfg(not(target_os = "windows"))]
    pub credential_store: Arc<dyn RefreshTokenStore + Send + Sync>,

    pub oauth_config: Arc<RwLock<Option<OAuthConfig>>>,

    pub connect_account_lock: tokio::sync::Mutex<()>,

    pub connect_account_use_case: Arc<dyn crate::application::ConnectAccountUseCase + 'static>,

    pub account_lifecycle_use_case: Arc<dyn crate::application::AccountLifecycleUseCase + 'static>,

    pub token_provider: Arc<crate::application::AccountTokenProvider<SqliteAccountStore>>,

    pub job_store: Arc<crate::infrastructure::SqliteJobStore>,

    pub job_service: Arc<
        crate::application::JobService<SqliteAccountStore, crate::infrastructure::SqliteJobStore>,
    >,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    #[cfg(target_os = "windows")]
    pub fn new(
        account_store: Arc<SqliteAccountStore>,
        credential_store: Arc<WindowsCredentialStore>,
        oauth_config: Arc<RwLock<Option<OAuthConfig>>>,
        connect_account_use_case: Arc<dyn crate::application::ConnectAccountUseCase + 'static>,
        account_lifecycle_use_case: Arc<dyn crate::application::AccountLifecycleUseCase + 'static>,
        token_provider: Arc<crate::application::AccountTokenProvider<SqliteAccountStore>>,
        job_store: Arc<crate::infrastructure::SqliteJobStore>,
        job_service: Arc<
            crate::application::JobService<
                SqliteAccountStore,
                crate::infrastructure::SqliteJobStore,
            >,
        >,
    ) -> Self {
        Self {
            account_store,
            credential_store,
            oauth_config,
            connect_account_lock: tokio::sync::Mutex::new(()),
            connect_account_use_case,
            account_lifecycle_use_case,
            token_provider,
            job_store,
            job_service,
        }
    }

    #[allow(clippy::too_many_arguments)]
    #[cfg(not(target_os = "windows"))]
    pub fn new(
        account_store: Arc<SqliteAccountStore>,
        credential_store: Arc<dyn RefreshTokenStore + Send + Sync>,
        oauth_config: Arc<RwLock<Option<OAuthConfig>>>,
        connect_account_use_case: Arc<dyn crate::application::ConnectAccountUseCase + 'static>,
        account_lifecycle_use_case: Arc<dyn crate::application::AccountLifecycleUseCase + 'static>,
        token_provider: Arc<crate::application::AccountTokenProvider<SqliteAccountStore>>,
        job_store: Arc<crate::infrastructure::SqliteJobStore>,
        job_service: Arc<
            crate::application::JobService<
                SqliteAccountStore,
                crate::infrastructure::SqliteJobStore,
            >,
        >,
    ) -> Self {
        Self {
            account_store,
            credential_store,
            oauth_config,
            connect_account_lock: tokio::sync::Mutex::new(()),
            connect_account_use_case,
            account_lifecycle_use_case,
            token_provider,
            job_store,
            job_service,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lookup(vars: &[(&str, &str)]) -> impl Fn(&str) -> Result<String, env::VarError> {
        let map: std::collections::HashMap<String, String> = vars
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        move |key: &str| map.get(key).cloned().ok_or(env::VarError::NotPresent)
    }

    // -- OAuthConfig --------------------------------------------------------

    #[test]
    fn oauth_config_new_stores_values() {
        let config = OAuthConfig::new("my-id", Some("my-secret".to_owned()));
        assert_eq!(config.client_id, "my-id");
        assert_eq!(config.client_secret.as_deref(), Some("my-secret"));
    }

    #[test]
    fn oauth_config_debug_redacts_secret() {
        let config = OAuthConfig::new("id", Some("super-secret-value".to_owned()));
        let debug = format!("{config:?}");
        assert!(debug.contains("id"));
        assert!(!debug.contains("super-secret-value"));
        assert!(debug.contains("[REDACTED]"));
    }

    #[test]
    fn from_env_returns_none_when_unset() {
        assert!(OAuthConfig::from_env_lookup(lookup(&[])).is_none());
    }

    #[test]
    fn from_env_returns_none_for_empty_id() {
        let result = OAuthConfig::from_env_lookup(lookup(&[("GDOM_GOOGLE_CLIENT_ID", "")]));
        assert!(result.is_none());
    }

    #[test]
    fn from_env_secret_only_uses_embedded_client_id() {
        let config = OAuthConfig::from_env_lookup(lookup(&[(
            "GDOM_GOOGLE_CLIENT_SECRET",
            "env-only-secret",
        )]))
        .expect("secret-only env should bind to the embedded client ID");
        assert_eq!(config.client_id, OAuthConfig::embedded_client_id());
        assert_eq!(config.client_secret.as_deref(), Some("env-only-secret"));
        assert!(config.has_client_secret());
    }

    #[test]
    fn from_env_reads_id_and_secret() {
        let config = OAuthConfig::from_env_lookup(lookup(&[
            ("GDOM_GOOGLE_CLIENT_ID", "env-id"),
            ("GDOM_GOOGLE_CLIENT_SECRET", "env-secret"),
        ]))
        .expect("should parse from env");
        assert_eq!(config.client_id, "env-id");
        assert_eq!(config.client_secret.as_deref(), Some("env-secret"));
    }

    #[test]
    fn from_env_treats_empty_secret_as_none() {
        let config = OAuthConfig::from_env_lookup(lookup(&[
            ("GDOM_GOOGLE_CLIENT_ID", "env-id"),
            ("GDOM_GOOGLE_CLIENT_SECRET", ""),
        ]))
        .expect("should parse from env");
        assert!(config.client_secret.is_none());
    }

    #[test]
    fn from_env_omits_secret_when_not_set() {
        let config = OAuthConfig::from_env_lookup(lookup(&[("GDOM_GOOGLE_CLIENT_ID", "env-id")]))
            .expect("should parse from env");
        assert!(config.client_secret.is_none());
    }

    #[test]
    fn resolve_prefers_stored_override_then_env_then_embedded() {
        let (stored, source) = OAuthConfig::resolve(
            Some(" stored-id "),
            None,
            lookup(&[("GDOM_GOOGLE_CLIENT_ID", "env-id")]),
        );
        assert_eq!(stored.client_id, "stored-id");
        assert_eq!(source, OAuthClientSource::CustomOverride);

        let (from_env, source) =
            OAuthConfig::resolve(None, None, lookup(&[("GDOM_GOOGLE_CLIENT_ID", "env-id")]));
        assert_eq!(from_env.client_id, "env-id");
        assert_eq!(source, OAuthClientSource::Environment);

        let (embedded, source) = OAuthConfig::resolve(Some("  "), None, lookup(&[]));
        assert_eq!(embedded.client_id, OAuthConfig::embedded_client_id());
        assert_eq!(source, OAuthClientSource::EmbeddedDefault);
        assert!(!embedded.client_id.is_empty());
        assert_eq!(
            embedded.client_secret,
            OAuthConfig::embedded_client_secret()
        );
    }

    // -- AppState -----------------------------------------------------------

    struct DummyConnectAccountUseCase;

    impl crate::application::ConnectAccountUseCase for DummyConnectAccountUseCase {
        fn connect_account(
            &self,
            _grant: crate::application::OAuthGrant,
            _fallback_account_id: crate::domain::AccountId,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<
                        Output = Result<
                            crate::domain::ConnectedAccount,
                            crate::application::ConnectAccountError,
                        >,
                    > + Send
                    + '_,
            >,
        > {
            Box::pin(async { unimplemented!() })
        }
    }

    struct DummyAccountLifecycleUseCase;

    impl crate::application::AccountLifecycleUseCase for DummyAccountLifecycleUseCase {
        fn list_accounts(
            &self,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<
                        Output = Result<
                            Vec<crate::domain::ConnectedAccount>,
                            crate::application::AccountLifecycleError,
                        >,
                    > + Send
                    + '_,
            >,
        > {
            Box::pin(async { unimplemented!() })
        }

        fn update_account_label(
            &self,
            _account_id: crate::domain::AccountId,
            _label: Option<String>,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<
                        Output = Result<
                            crate::domain::ConnectedAccount,
                            crate::application::AccountLifecycleError,
                        >,
                    > + Send
                    + '_,
            >,
        > {
            Box::pin(async { unimplemented!() })
        }

        fn disconnect_account(
            &self,
            _account_id: crate::domain::AccountId,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<
                        Output = Result<(), crate::application::AccountLifecycleError>,
                    > + Send
                    + '_,
            >,
        > {
            Box::pin(async { unimplemented!() })
        }

        fn reauthenticate_account(
            &self,
            _account_id: crate::domain::AccountId,
            _oauth_grant: crate::application::OAuthGrant,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<
                        Output = Result<
                            crate::domain::ConnectedAccount,
                            crate::application::AccountLifecycleError,
                        >,
                    > + Send
                    + '_,
            >,
        > {
            Box::pin(async { unimplemented!() })
        }

        fn remove_account(
            &self,
            _account_id: crate::domain::AccountId,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<
                        Output = Result<(), crate::application::AccountLifecycleError>,
                    > + Send
                    + '_,
            >,
        > {
            Box::pin(async { unimplemented!() })
        }

        fn delete_local_account_data(
            &self,
            _account_id: crate::domain::AccountId,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<
                        Output = Result<(), crate::application::AccountLifecycleError>,
                    > + Send
                    + '_,
            >,
        > {
            Box::pin(async { unimplemented!() })
        }
    }

    #[tokio::test]
    async fn app_state_new_holds_oauth_config() {
        let store = SqliteAccountStore::open_in_memory()
            .await
            .expect("in-memory store");
        let account_store = Arc::new(store);
        let cred_store =
            Arc::new(crate::infrastructure::secrets::WindowsCredentialStore::new_mock());
        let oauth = OAuthConfig::new("test-id", None);
        let oauth_lock = Arc::new(RwLock::new(Some(oauth)));
        let use_case: Arc<dyn crate::application::ConnectAccountUseCase> =
            Arc::new(DummyConnectAccountUseCase);
        let lifecycle_use_case: Arc<dyn crate::application::AccountLifecycleUseCase> =
            Arc::new(DummyAccountLifecycleUseCase);

        let refresh_port = Arc::new(
            crate::infrastructure::google_token::DynamicGoogleTokenClient::new(Arc::clone(
                &oauth_lock,
            )),
        );
        let token_provider = Arc::new(crate::application::AccountTokenProvider::new(
            refresh_port,
            cred_store.clone(),
            account_store.clone(),
        ));

        let job_store = Arc::new(crate::infrastructure::SqliteJobStore::new(
            account_store.pool().clone(),
        ));
        let drive_client: Arc<dyn crate::application::DrivePort> =
            Arc::new(crate::infrastructure::google_drive::GoogleDriveClient::new().unwrap());
        let job_service = Arc::new(crate::application::JobService::new(
            account_store.clone(),
            job_store.clone(),
            drive_client,
            token_provider.clone(),
        ));

        let state = AppState::new(
            account_store,
            cred_store,
            oauth_lock,
            use_case,
            lifecycle_use_case,
            token_provider,
            job_store,
            job_service,
        );

        let guard = state.oauth_config.read().await;
        let config = guard.as_ref().expect("should have config");
        assert_eq!(config.client_id, "test-id");
    }

    #[tokio::test]
    async fn app_state_oauth_config_defaults_to_none() {
        let store = SqliteAccountStore::open_in_memory()
            .await
            .expect("in-memory store");
        let account_store = Arc::new(store);
        let cred_store =
            Arc::new(crate::infrastructure::secrets::WindowsCredentialStore::new_mock());
        let oauth_lock = Arc::new(RwLock::new(None));
        let use_case: Arc<dyn crate::application::ConnectAccountUseCase> =
            Arc::new(DummyConnectAccountUseCase);
        let lifecycle_use_case: Arc<dyn crate::application::AccountLifecycleUseCase> =
            Arc::new(DummyAccountLifecycleUseCase);

        let refresh_port = Arc::new(
            crate::infrastructure::google_token::DynamicGoogleTokenClient::new(Arc::clone(
                &oauth_lock,
            )),
        );
        let token_provider = Arc::new(crate::application::AccountTokenProvider::new(
            refresh_port,
            cred_store.clone(),
            account_store.clone(),
        ));

        let job_store = Arc::new(crate::infrastructure::SqliteJobStore::new(
            account_store.pool().clone(),
        ));
        let drive_client: Arc<dyn crate::application::DrivePort> =
            Arc::new(crate::infrastructure::google_drive::GoogleDriveClient::new().unwrap());
        let job_service = Arc::new(crate::application::JobService::new(
            account_store.clone(),
            job_store.clone(),
            drive_client,
            token_provider.clone(),
        ));

        let state = AppState::new(
            account_store,
            cred_store,
            oauth_lock,
            use_case,
            lifecycle_use_case,
            token_provider,
            job_store,
            job_service,
        );

        let guard = state.oauth_config.read().await;
        assert!(guard.is_none());
    }
}
