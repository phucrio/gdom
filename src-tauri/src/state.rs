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

/// Google OAuth client credentials loaded at startup.
///
/// `client_secret` is deliberately redacted from `Debug` output to prevent
/// accidental exposure in logs.
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

    /// Build from environment variables. Returns `Some` when
    /// `GDOM_GOOGLE_CLIENT_ID` is set and non-empty.
    pub fn from_env() -> Option<Self> {
        Self::from_env_lookup(|key| env::var(key))
    }

    fn from_env_lookup(lookup: impl Fn(&str) -> Result<String, env::VarError>) -> Option<Self> {
        let client_id = lookup("GDOM_GOOGLE_CLIENT_ID").ok()?;
        if client_id.is_empty() {
            return None;
        }
        let client_secret = lookup("GDOM_GOOGLE_CLIENT_SECRET")
            .ok()
            .filter(|s| !s.is_empty());
        Some(Self::new(client_id, client_secret))
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
}

impl AppState {
    #[cfg(target_os = "windows")]
    pub fn new(
        account_store: Arc<SqliteAccountStore>,
        credential_store: Arc<WindowsCredentialStore>,
        oauth_config: Arc<RwLock<Option<OAuthConfig>>>,
        connect_account_use_case: Arc<dyn crate::application::ConnectAccountUseCase + 'static>,
    ) -> Self {
        Self {
            account_store,
            credential_store,
            oauth_config,
            connect_account_lock: tokio::sync::Mutex::new(()),
            connect_account_use_case,
        }
    }

    #[cfg(not(target_os = "windows"))]
    pub fn new(
        account_store: Arc<SqliteAccountStore>,
        credential_store: Arc<dyn RefreshTokenStore + Send + Sync>,
        oauth_config: Arc<RwLock<Option<OAuthConfig>>>,
        connect_account_use_case: Arc<dyn crate::application::ConnectAccountUseCase + 'static>,
    ) -> Self {
        Self {
            account_store,
            credential_store,
            oauth_config,
            connect_account_lock: tokio::sync::Mutex::new(()),
            connect_account_use_case,
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

    #[tokio::test]
    async fn app_state_new_holds_oauth_config() {
        let store = SqliteAccountStore::open_in_memory()
            .await
            .expect("in-memory store");
        let account_store = Arc::new(store);
        let cred_store = crate::infrastructure::secrets::WindowsCredentialStore::new_mock();
        let oauth = OAuthConfig::new("test-id", None);
        let oauth_lock = Arc::new(RwLock::new(Some(oauth)));
        let use_case: Arc<dyn crate::application::ConnectAccountUseCase> =
            Arc::new(DummyConnectAccountUseCase);

        let state = AppState::new(account_store, Arc::new(cred_store), oauth_lock, use_case);

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
        let cred_store = crate::infrastructure::secrets::WindowsCredentialStore::new_mock();
        let oauth_lock = Arc::new(RwLock::new(None));
        let use_case: Arc<dyn crate::application::ConnectAccountUseCase> =
            Arc::new(DummyConnectAccountUseCase);

        let state = AppState::new(account_store, Arc::new(cred_store), oauth_lock, use_case);

        let guard = state.oauth_config.read().await;
        assert!(guard.is_none());
    }
}
