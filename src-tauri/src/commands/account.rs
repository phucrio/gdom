use tauri::{AppHandle, Emitter};

use crate::{
    application::OAuthGrant,
    domain::AccountId,
    infrastructure::google_oauth::DesktopOAuthSession,
    state::{AppState, OAuthConfig},
};

use super::dto::{AccountDto, ConfigureOAuthInput, OAuthConfigDto};
use super::error::CommandError;

// ---------------------------------------------------------------------------
// Core logic (testable without tauri::State)
// ---------------------------------------------------------------------------

async fn list_accounts_inner(state: &AppState) -> Result<Vec<AccountDto>, CommandError> {
    let accounts = state
        .account_store
        .list_all()
        .await
        .map_err(|e| CommandError::Database(e.to_string()))?;

    Ok(accounts.iter().map(AccountDto::from).collect())
}

async fn configure_oauth_inner(
    input: ConfigureOAuthInput,
    state: &AppState,
) -> Result<(), CommandError> {
    let client_id = input.client_id.trim();
    if client_id.is_empty() {
        return Err(CommandError::NotConfigured(
            "OAuth client ID must not be empty".into(),
        ));
    }

    let client_secret = input
        .client_secret
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty());

    state
        .account_store
        .save_oauth_config(client_id, client_secret.as_deref())
        .await
        .map_err(|e| CommandError::Database(e.to_string()))?;

    let mut guard = state.oauth_config.write().await;
    *guard = Some(OAuthConfig::new(client_id.to_owned(), client_secret));

    Ok(())
}

async fn get_oauth_config_inner(state: &AppState) -> Result<OAuthConfigDto, CommandError> {
    let guard = state.oauth_config.read().await;
    match guard.as_ref() {
        Some(config) => Ok(OAuthConfigDto {
            is_configured: true,
            client_id: Some(config.client_id.clone()),
        }),
        None => Ok(OAuthConfigDto {
            is_configured: false,
            client_id: None,
        }),
    }
}

// ---------------------------------------------------------------------------
// Tauri command handlers (thin wrappers)
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn list_accounts(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<AccountDto>, CommandError> {
    list_accounts_inner(&state).await
}

#[tauri::command]
pub async fn configure_oauth(
    input: ConfigureOAuthInput,
    state: tauri::State<'_, AppState>,
) -> Result<(), CommandError> {
    configure_oauth_inner(input, &state).await
}

#[tauri::command]
pub async fn get_oauth_config(
    state: tauri::State<'_, AppState>,
) -> Result<OAuthConfigDto, CommandError> {
    get_oauth_config_inner(&state).await
}

#[tauri::command]
pub async fn connect_account(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<AccountDto, CommandError> {
    let _lock = state.connect_account_lock.try_lock().map_err(|_| {
        CommandError::OAuth("Another account connection is already in progress".into())
    })?;

    let config = {
        let guard = state.oauth_config.read().await;
        guard.clone().ok_or_else(|| {
            CommandError::NotConfigured("OAuth client ID is not configured".into())
        })?
    };

    let session = DesktopOAuthSession::start(&config.client_id)
        .await
        .map_err(|e| CommandError::OAuth(e.to_string()))?;

    let authorization_url = session.authorization_url().to_owned();
    open::that_detached(&authorization_url)
        .map_err(|e| CommandError::BrowserLaunchFailed(e.to_string()))?;

    let grant = session
        .receive_callback()
        .await
        .map_err(|e| CommandError::OAuth(e.to_string()))?;

    let grant = OAuthGrant::new(
        grant.authorization_code().to_owned(),
        grant.pkce_verifier().to_owned(),
        grant.redirect_uri().to_owned(),
    );

    let duration_since_epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| CommandError::Internal(e.to_string()))?;
    let fallback_id = duration_since_epoch.as_nanos();

    let connected_account = state
        .connect_account_use_case
        .connect_account(grant, AccountId::new(fallback_id))
        .await?;

    let _ = app.emit("account-registry-changed", ());

    Ok(AccountDto::from(connected_account))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::{
        commands::dto::ConfigureOAuthInput,
        commands::error::CommandError,
        domain::{AccountId, AccountProfile, ConnectedAccount, GooglePermissionId},
        infrastructure::{account_store::SqliteAccountStore, secrets::WindowsCredentialStore},
        state::{AppState, OAuthConfig},
    };

    use super::{configure_oauth_inner, get_oauth_config_inner, list_accounts_inner};

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

    /// Build a minimal `AppState` with an in-memory SQLite store and mock
    /// credential store for command-level tests.
    async fn test_state(oauth: Option<OAuthConfig>) -> AppState {
        let store = SqliteAccountStore::open_in_memory()
            .await
            .expect("in-memory account store");
        let cred_store = WindowsCredentialStore::new_mock();
        let use_case: Arc<dyn crate::application::ConnectAccountUseCase> =
            Arc::new(DummyConnectAccountUseCase);
        let oauth_lock = Arc::new(tokio::sync::RwLock::new(oauth));
        AppState::new(Arc::new(store), Arc::new(cred_store), oauth_lock, use_case)
    }

    // -- list_accounts -------------------------------------------------------

    #[tokio::test]
    async fn list_accounts_returns_empty_when_no_accounts() {
        let state = test_state(None).await;
        let result = list_accounts_inner(&state).await;

        let accounts = result.expect("command succeeds");
        assert!(accounts.is_empty());
    }

    #[tokio::test]
    async fn list_accounts_returns_persisted_accounts() {
        let state = test_state(None).await;

        // Seed two accounts directly into the store.
        let account_a = ConnectedAccount::new(
            AccountId::new(1),
            GooglePermissionId::new("perm-a"),
            AccountProfile::new("a@gmail.com", "Alice"),
        );
        let account_b = ConnectedAccount::new(
            AccountId::new(2),
            GooglePermissionId::new("perm-b"),
            AccountProfile::new("b@gmail.com", "Bob"),
        );
        state
            .account_store
            .connect(&account_a)
            .await
            .expect("seed a");
        state
            .account_store
            .connect(&account_b)
            .await
            .expect("seed b");

        let result = list_accounts_inner(&state).await;

        let accounts = result.expect("command succeeds");
        assert_eq!(accounts.len(), 2);
        // Ordered by email ASC per SqliteAccountStore::list_all.
        assert_eq!(accounts[0].email, "a@gmail.com");
        assert_eq!(accounts[1].email, "b@gmail.com");
    }

    // -- configure_oauth -----------------------------------------------------

    #[tokio::test]
    async fn configure_oauth_stores_config() {
        let state = test_state(None).await;
        let input = ConfigureOAuthInput {
            client_id: "test-client-id".into(),
            client_secret: Some("test-secret".into()),
        };

        configure_oauth_inner(input, &state)
            .await
            .expect("command succeeds");

        let guard = state.oauth_config.read().await;
        let config = guard.as_ref().expect("config is set");
        assert_eq!(config.client_id, "test-client-id");
        assert_eq!(config.client_secret.as_deref(), Some("test-secret"));

        assert_eq!(
            state
                .account_store
                .get_setting("oauth.client_id")
                .await
                .unwrap(),
            Some("test-client-id".into())
        );
        assert_eq!(
            state
                .account_store
                .get_setting("oauth.client_secret")
                .await
                .unwrap(),
            Some("test-secret".into())
        );
    }

    #[tokio::test]
    async fn configure_oauth_rejects_empty_client_id() {
        let state = test_state(None).await;
        let input = ConfigureOAuthInput {
            client_id: "  ".into(),
            client_secret: None,
        };

        let result = configure_oauth_inner(input, &state).await;

        let error = result.expect_err("command rejects empty client ID");
        assert!(matches!(error, CommandError::NotConfigured(_)));
    }

    #[tokio::test]
    async fn configure_oauth_replaces_existing_config() {
        let initial = OAuthConfig::new("old-id", Some("old-secret".to_owned()));
        let state = test_state(Some(initial)).await;
        let input = ConfigureOAuthInput {
            client_id: "new-id".into(),
            client_secret: None,
        };

        configure_oauth_inner(input, &state)
            .await
            .expect("command succeeds");

        let guard = state.oauth_config.read().await;
        let config = guard.as_ref().expect("config is set");
        assert_eq!(config.client_id, "new-id");
        assert!(config.client_secret.is_none());

        assert_eq!(
            state
                .account_store
                .get_setting("oauth.client_id")
                .await
                .unwrap(),
            Some("new-id".into())
        );
        assert_eq!(
            state
                .account_store
                .get_setting("oauth.client_secret")
                .await
                .unwrap(),
            None
        );
    }

    // -- get_oauth_config ----------------------------------------------------

    #[tokio::test]
    async fn get_oauth_config_returns_not_configured_when_none() {
        let state = test_state(None).await;
        let result = get_oauth_config_inner(&state).await;

        let dto = result.expect("command succeeds");
        assert!(!dto.is_configured);
        assert!(dto.client_id.is_none());
    }

    #[tokio::test]
    async fn get_oauth_config_returns_configured_with_client_id() {
        let config = OAuthConfig::new("visible-id", Some("hidden-secret".to_owned()));
        let state = test_state(Some(config)).await;

        let result = get_oauth_config_inner(&state).await;

        let dto = result.expect("command succeeds");
        assert!(dto.is_configured);
        assert_eq!(dto.client_id.as_deref(), Some("visible-id"));
    }

    #[tokio::test]
    async fn get_oauth_config_never_exposes_secret() {
        let config = OAuthConfig::new("id", Some("super-secret".to_owned()));
        let state = test_state(Some(config)).await;

        let result = get_oauth_config_inner(&state).await;

        let dto = result.expect("command succeeds");
        let json = serde_json::to_string(&dto).expect("serializes");
        assert!(!json.contains("super-secret"));
    }
}
