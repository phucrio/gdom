use tauri::{AppHandle, Emitter};

use crate::{
    application::{OAuthGrant, RefreshTokenStore},
    domain::AccountId,
    infrastructure::google_oauth::DesktopOAuthSession,
    state::{AppState, DESKTOP_CLIENT_SECRET_REQUIRED, OAuthConfig},
};

use super::dto::{
    AccountDto, AccountIdInput, ConfigureOAuthInput, DeleteAccountDataInput, OAuthConfigDto,
    UpdateAccountLabelInput,
};
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
    let _lock = state.connect_account_lock.try_lock().map_err(|_| {
        CommandError::OAuth(
            "Cannot change OAuth configuration while an account connection is in progress".into(),
        )
    })?;

    let client_id = input.client_id.trim();
    if client_id.is_empty() {
        return Err(CommandError::NotConfigured(
            "OAuth client ID must not be empty".into(),
        ));
    }

    let existing_accounts_count = state
        .account_store
        .account_count()
        .await
        .map_err(|e| CommandError::Database(e.to_string()))?;

    if existing_accounts_count > 0 {
        let guard = state.oauth_config.read().await;
        if let Some(existing_config) = guard.as_ref()
            && existing_config.client_id != client_id
        {
            return Err(CommandError::OAuth(
                "Cannot change OAuth client ID while connected accounts exist. Disconnect all accounts first.".into(),
            ));
        }
    }

    let client_secret = input
        .client_secret
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| CommandError::NotConfigured(DESKTOP_CLIENT_SECRET_REQUIRED.into()))?;

    state
        .account_store
        .save_oauth_client_id(client_id)
        .await
        .map_err(|e| CommandError::Database(e.to_string()))?;

    state
        .credential_store
        .save_oauth_secret(&client_secret)
        .map_err(|e| CommandError::Keychain(e.to_string()))?;

    let mut guard = state.oauth_config.write().await;
    *guard = Some(OAuthConfig::new(client_id.to_owned(), Some(client_secret)));

    Ok(())
}

async fn get_oauth_config_inner(state: &AppState) -> Result<OAuthConfigDto, CommandError> {
    let stored = state
        .account_store
        .get_setting("oauth.client_id")
        .await
        .map_err(|e| CommandError::Database(e.to_string()))?;
    let using_custom_override = stored
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty());

    let authentication_lock = state.connect_account_lock.try_lock().ok();
    if authentication_lock.is_some() {
        recover_custom_oauth_secret(state).await?;
    }
    let guard = state.oauth_config.read().await;
    let client_id = match guard.as_ref() {
        Some(config) => Some(config.client_id.clone()),
        None => Some(OAuthConfig::embedded_client_id().to_owned()),
    };

    let can_sign_in = match guard.as_ref() {
        Some(config) => config.has_client_secret(),
        None => OAuthConfig::default_config().has_client_secret(),
    };

    Ok(OAuthConfigDto {
        is_configured: client_id
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty()),
        client_id,
        using_custom_override,
        can_sign_in,
    })
}

// Call while holding connect_account_lock so configure/reset cannot change the client pair.
pub(super) async fn recover_custom_oauth_secret(state: &AppState) -> Result<(), CommandError> {
    let mut configuration = state.oauth_config.write().await;
    let Some(config) = configuration.as_mut() else {
        return Ok(());
    };
    if config.has_client_secret() {
        return Ok(());
    }
    let stored_client_id = state
        .account_store
        .get_setting("oauth.client_id")
        .await
        .map_err(|error| CommandError::Database(error.to_string()))?;
    if stored_client_id.as_deref().map(str::trim) == Some(config.client_id.as_str()) {
        config.client_secret = state.credential_store.load_oauth_secret().map_err(|_| {
            CommandError::Keychain(
                "Unlock your system credential store, then retry sign-in.".into(),
            )
        })?;
    }
    Ok(())
}

pub(super) fn require_desktop_client_secret(config: &OAuthConfig) -> Result<(), CommandError> {
    if config.has_client_secret() {
        Ok(())
    } else {
        Err(CommandError::NotConfigured(
            DESKTOP_CLIENT_SECRET_REQUIRED.into(),
        ))
    }
}

async fn reset_oauth_config_inner(state: &AppState) -> Result<OAuthConfigDto, CommandError> {
    let _lock = state.connect_account_lock.try_lock().map_err(|_| {
        CommandError::OAuth(
            "Cannot change OAuth configuration while an account connection is in progress".into(),
        )
    })?;

    let next = OAuthConfig::from_env().unwrap_or_else(OAuthConfig::default_config);
    let existing_accounts_count = state
        .account_store
        .account_count()
        .await
        .map_err(|e| CommandError::Database(e.to_string()))?;
    if existing_accounts_count > 0 {
        let guard = state.oauth_config.read().await;
        if let Some(existing) = guard.as_ref()
            && existing.client_id != next.client_id
        {
            return Err(CommandError::OAuth(
                "Cannot change OAuth client ID while connected accounts exist. Disconnect all accounts first.".into(),
            ));
        }
    }

    state
        .account_store
        .clear_oauth_client_id()
        .await
        .map_err(|e| CommandError::Database(e.to_string()))?;
    state
        .credential_store
        .delete_oauth_secret()
        .map_err(|e| CommandError::Keychain(e.to_string()))?;

    {
        let mut guard = state.oauth_config.write().await;
        *guard = Some(next);
    }

    get_oauth_config_inner(state).await
}

pub(crate) fn parse_account_id(id_str: &str) -> Result<AccountId, CommandError> {
    let parsed = id_str.parse::<u128>().map_err(|_| {
        CommandError::AccountNotFound(format!("invalid account id format: {id_str}"))
    })?;
    Ok(AccountId::new(parsed))
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
pub async fn reset_oauth_config(
    state: tauri::State<'_, AppState>,
) -> Result<OAuthConfigDto, CommandError> {
    reset_oauth_config_inner(&state).await
}

async fn disconnect_account_inner(
    state: &AppState,
    account_id: AccountId,
) -> Result<(), CommandError> {
    state.token_provider.invalidate_cache(account_id).await;
    state
        .account_lifecycle_use_case
        .disconnect_account(account_id)
        .await?;
    Ok(())
}

async fn update_account_label_inner(
    state: &AppState,
    account_id: AccountId,
    label: Option<String>,
) -> Result<AccountDto, CommandError> {
    let updated = state
        .account_lifecycle_use_case
        .update_account_label(account_id, label)
        .await?;
    Ok(AccountDto::from(updated))
}

async fn remove_account_inner(state: &AppState, account_id: AccountId) -> Result<(), CommandError> {
    state
        .token_provider
        .remove_account_lifecycle(account_id)
        .await;
    state
        .account_lifecycle_use_case
        .remove_account(account_id)
        .await?;
    Ok(())
}

async fn delete_local_account_data_inner(
    state: &AppState,
    account_id: AccountId,
    confirmation: bool,
) -> Result<(), CommandError> {
    if !confirmation {
        return Err(CommandError::ConfirmationRequired(
            "confirmation flag must be true to hard-delete local account data".into(),
        ));
    }

    state
        .token_provider
        .remove_account_lifecycle(account_id)
        .await;
    state
        .account_lifecycle_use_case
        .delete_local_account_data(account_id)
        .await?;
    Ok(())
}

#[tauri::command]
pub async fn disconnect_account(
    input: AccountIdInput,
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), CommandError> {
    let account_id = parse_account_id(&input.account_id)?;
    disconnect_account_inner(&state, account_id).await?;
    let _ = app.emit("account-registry-changed", ());
    Ok(())
}

#[tauri::command]
pub async fn update_account_label(
    input: UpdateAccountLabelInput,
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<AccountDto, CommandError> {
    let account_id = parse_account_id(&input.account_id)?;
    let account = update_account_label_inner(&state, account_id, input.label).await?;
    let _ = app.emit("account-registry-changed", ());
    Ok(account)
}

#[tauri::command]
pub async fn reauthenticate_account(
    input: AccountIdInput,
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<AccountDto, CommandError> {
    let account_id = parse_account_id(&input.account_id)?;

    let _lock = state
        .connect_account_lock
        .try_lock()
        .map_err(|_| CommandError::OAuth("Another account authentication is in progress".into()))?;

    recover_custom_oauth_secret(&state).await?;
    let config = {
        let guard = state.oauth_config.read().await;
        guard.clone().unwrap_or_else(OAuthConfig::default_config)
    };
    require_desktop_client_secret(&config)?;

    let session = DesktopOAuthSession::start(&config.client_id)
        .await
        .map_err(|e| CommandError::OAuth(e.to_string()))?;

    let authorization_url = session.authorization_url().to_owned();
    super::account_connection::launch_authorization_browser(&authorization_url).await?;

    let grant = session
        .receive_callback()
        .await
        .map_err(|e| CommandError::OAuth(e.to_string()))?;

    let grant = OAuthGrant::new(
        grant.authorization_code().to_owned(),
        grant.pkce_verifier().to_owned(),
        grant.redirect_uri().to_owned(),
    );

    let account = state
        .account_lifecycle_use_case
        .reauthenticate_account(account_id, grant)
        .await?;
    state.token_provider.invalidate_cache(account_id).await;

    let _ = app.emit("account-registry-changed", ());
    Ok(AccountDto::from(account))
}

#[tauri::command]
pub async fn remove_account(
    input: AccountIdInput,
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), CommandError> {
    let account_id = parse_account_id(&input.account_id)?;
    remove_account_inner(&state, account_id).await?;
    let _ = app.emit("account-registry-changed", ());
    Ok(())
}

#[tauri::command]
pub async fn delete_local_account_data(
    input: DeleteAccountDataInput,
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), CommandError> {
    let account_id = parse_account_id(&input.account_id)?;
    delete_local_account_data_inner(&state, account_id, input.confirmation).await?;
    let _ = app.emit("account-registry-changed", ());
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::{
        application::RefreshTokenStore,
        commands::dto::ConfigureOAuthInput,
        commands::error::CommandError,
        domain::{AccountId, AccountProfile, ConnectedAccount, GooglePermissionId},
        infrastructure::{account_store::SqliteAccountStore, secrets::NativeCredentialStore},
        state::{AppState, OAuthConfig},
    };

    use super::{
        configure_oauth_inner, get_oauth_config_inner, list_accounts_inner,
        reset_oauth_config_inner,
    };

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

    async fn test_state(oauth: Option<OAuthConfig>) -> AppState {
        let store = SqliteAccountStore::open_in_memory()
            .await
            .expect("in-memory account store");
        let account_store = Arc::new(store);
        let cred_store = Arc::new(NativeCredentialStore::new_mock());
        let use_case: Arc<dyn crate::application::ConnectAccountUseCase> =
            Arc::new(DummyConnectAccountUseCase);
        let oauth_lock = Arc::new(tokio::sync::RwLock::new(oauth));

        let token_service = Arc::new(
            crate::infrastructure::google_token::DynamicGoogleTokenClient::new(Arc::clone(
                &oauth_lock,
            )),
        );
        let token_provider = Arc::new(crate::application::AccountTokenProvider::new(
            token_service.clone(),
            cred_store.clone(),
            account_store.clone(),
        ));

        let job_store = Arc::new(crate::infrastructure::SqliteJobStore::new(
            account_store.pool().clone(),
        ));
        let drive_client = crate::infrastructure::google_drive::GoogleDriveClient::new().unwrap();
        let lifecycle_service = crate::application::AccountLifecycleService::new(
            token_service,
            drive_client.clone(),
            account_store.clone(),
            cred_store.clone(),
        )
        .with_job_store(job_store.clone());
        let lifecycle_use_case: Arc<dyn crate::application::AccountLifecycleUseCase> =
            Arc::new(lifecycle_service);

        let drive_arc = Arc::new(drive_client);
        let job_service = Arc::new(crate::application::JobService::new(
            account_store.clone(),
            job_store.clone(),
            drive_arc.clone() as Arc<dyn crate::application::DrivePort>,
            token_provider.clone(),
        ));

        AppState::new(
            account_store,
            cred_store,
            oauth_lock,
            use_case,
            lifecycle_use_case,
            token_provider,
            job_store,
            drive_arc,
            job_service,
        )
    }

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

        let account_a = ConnectedAccount::new(
            AccountId::new(1),
            GooglePermissionId::new("perm-a"),
            AccountProfile::new("a@gmail.com", "Alice", None),
        );
        let account_b = ConnectedAccount::new(
            AccountId::new(2),
            GooglePermissionId::new("perm-b"),
            AccountProfile::new("b@gmail.com", "Bob", None),
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
        assert_eq!(accounts[0].email, "a@gmail.com");
        assert_eq!(accounts[1].email, "b@gmail.com");
    }

    #[tokio::test]
    async fn configure_oauth_stores_config() {
        let state = test_state(None).await;
        let input = ConfigureOAuthInput {
            client_id: "test-client-id".into(),
            client_secret: Some("test-secret".into()),
        };

        let result = configure_oauth_inner(input, &state).await;
        assert!(result.is_ok());

        let guard = state.oauth_config.read().await;
        let config = guard.as_ref().expect("config is stored");
        assert_eq!(config.client_id, "test-client-id");
        assert_eq!(config.client_secret.as_deref(), Some("test-secret"));

        let stored_id = state
            .account_store
            .get_setting("oauth.client_id")
            .await
            .expect("read setting")
            .expect("setting exists");
        assert_eq!(stored_id, "test-client-id");

        let stored_secret = state
            .credential_store
            .load_oauth_secret()
            .expect("keychain read")
            .expect("secret exists");
        assert_eq!(stored_secret, "test-secret");

        let dto = get_oauth_config_inner(&state).await.expect("dto");
        assert!(dto.using_custom_override);
        assert!(dto.can_sign_in);
    }

    #[tokio::test]
    async fn configure_oauth_rejects_missing_secret() {
        let state = test_state(None).await;
        let input = ConfigureOAuthInput {
            client_id: "desktop-id".into(),
            client_secret: None,
        };

        let result = configure_oauth_inner(input, &state).await;
        assert!(matches!(result, Err(CommandError::NotConfigured(_))));
    }

    #[tokio::test]
    async fn configure_oauth_rejects_empty_client_id() {
        let state = test_state(None).await;
        let input = ConfigureOAuthInput {
            client_id: "   ".into(),
            client_secret: None,
        };

        let result = configure_oauth_inner(input, &state).await;
        assert!(matches!(result, Err(CommandError::NotConfigured(_))));
    }

    #[tokio::test]
    async fn configure_oauth_replaces_existing_config() {
        let state = test_state(Some(OAuthConfig::new("old-id", Some("old-secret".into())))).await;
        let input = ConfigureOAuthInput {
            client_id: "new-id".into(),
            client_secret: Some("new-secret".into()),
        };

        let result = configure_oauth_inner(input, &state).await;
        assert!(result.is_ok());

        let guard = state.oauth_config.read().await;
        let config = guard.as_ref().expect("config is stored");
        assert_eq!(config.client_id, "new-id");
        assert_eq!(config.client_secret.as_deref(), Some("new-secret"));

        let stored_secret = state
            .credential_store
            .load_oauth_secret()
            .expect("keychain read")
            .expect("secret exists");
        assert_eq!(stored_secret, "new-secret");
    }

    #[tokio::test]
    async fn configure_oauth_rejects_client_id_change_when_accounts_exist() {
        let state = test_state(Some(OAuthConfig::new("current-id", None))).await;

        let account = ConnectedAccount::new(
            AccountId::new(1),
            GooglePermissionId::new("perm-1"),
            AccountProfile::new("user@gmail.com", "User", None),
        );
        state
            .account_store
            .connect(&account)
            .await
            .expect("seed account");

        let input = ConfigureOAuthInput {
            client_id: "different-id".into(),
            client_secret: Some("secret".into()),
        };

        let result = configure_oauth_inner(input, &state).await;
        assert!(matches!(result, Err(CommandError::OAuth(_))));
    }

    #[tokio::test]
    async fn configure_oauth_rejected_while_connection_in_progress() {
        let state = test_state(None).await;

        let _guard = state.connect_account_lock.lock().await;

        let input = ConfigureOAuthInput {
            client_id: "some-id".into(),
            client_secret: None,
        };

        let result = configure_oauth_inner(input, &state).await;
        assert!(matches!(result, Err(CommandError::OAuth(_))));
    }

    #[tokio::test]
    async fn get_oauth_config_returns_embedded_default_when_unset() {
        let state = test_state(None).await;
        let result = get_oauth_config_inner(&state).await.expect("succeeds");

        assert!(result.is_configured);
        assert_eq!(
            result.client_id.as_deref(),
            Some(OAuthConfig::embedded_client_id())
        );
        assert!(!result.using_custom_override);
        assert_eq!(
            result.can_sign_in,
            OAuthConfig::default_config().has_client_secret()
        );
    }

    #[tokio::test]
    async fn get_oauth_config_returns_configured_with_client_id() {
        let state = test_state(Some(OAuthConfig::new("my-client-id", None))).await;
        let result = get_oauth_config_inner(&state).await.expect("succeeds");

        assert!(result.is_configured);
        assert_eq!(result.client_id.as_deref(), Some("my-client-id"));
        assert!(!result.using_custom_override);
        assert!(!result.can_sign_in);
    }

    #[tokio::test]
    async fn get_oauth_config_recovers_custom_secret_after_keychain_becomes_available() {
        let state = test_state(Some(OAuthConfig::new("custom-client", None))).await;
        state
            .account_store
            .save_oauth_client_id("custom-client")
            .await
            .expect("stored override");
        assert!(
            !get_oauth_config_inner(&state)
                .await
                .expect("missing secret")
                .can_sign_in
        );
        state
            .credential_store
            .save_oauth_secret("recovered-secret")
            .expect("keychain unlocked");

        let configuration = get_oauth_config_inner(&state).await.expect("retry");

        assert!(configuration.can_sign_in);
        assert!(configuration.using_custom_override);
        let cached = state.oauth_config.read().await;
        let cached = cached.as_ref().expect("shared configuration");
        assert_eq!(cached.client_id, "custom-client");
        assert_eq!(cached.client_secret.as_deref(), Some("recovered-secret"));
    }

    #[tokio::test]
    async fn get_oauth_config_does_not_replace_configured_or_other_client_secrets() {
        for (client_id, existing_secret, stored_client) in [
            (
                "custom-client",
                Some("active-secret"),
                Some("custom-client"),
            ),
            ("environment-client", None, None),
            ("environment-client", None, Some("other-client")),
        ] {
            let state = test_state(Some(OAuthConfig::new(
                client_id,
                existing_secret.map(str::to_owned),
            )))
            .await;
            if let Some(stored_client) = stored_client {
                state
                    .account_store
                    .save_oauth_client_id(stored_client)
                    .await
                    .expect("stored override");
            }
            state
                .credential_store
                .save_oauth_secret("unrelated-keychain-secret")
                .expect("stored secret");

            get_oauth_config_inner(&state)
                .await
                .expect("get configuration");

            let cached = state.oauth_config.read().await;
            let cached = cached.as_ref().expect("shared configuration");
            assert_eq!(cached.client_id, client_id);
            assert_eq!(cached.client_secret.as_deref(), existing_secret);
        }
    }

    #[tokio::test]
    async fn reset_oauth_config_clears_override_and_restores_default() {
        let state = test_state(Some(OAuthConfig::new("custom-id", Some("secret".into())))).await;
        state
            .account_store
            .save_oauth_client_id("custom-id")
            .await
            .expect("store override");

        let result = reset_oauth_config_inner(&state).await.expect("reset");
        assert!(result.is_configured);
        assert!(!result.using_custom_override);
        assert_eq!(
            result.client_id.as_deref(),
            Some(OAuthConfig::embedded_client_id())
        );
        assert!(
            state
                .account_store
                .get_setting("oauth.client_id")
                .await
                .expect("read")
                .is_none()
        );
        assert!(
            state
                .credential_store
                .load_oauth_secret()
                .expect("keychain")
                .is_none()
        );
    }

    #[tokio::test]
    async fn get_oauth_config_never_exposes_secret() {
        let state = test_state(Some(OAuthConfig::new(
            "my-client-id",
            Some("super-secret".into()),
        )))
        .await;
        let result = get_oauth_config_inner(&state).await.expect("succeeds");

        assert_eq!(result.client_id.as_deref(), Some("my-client-id"));
        assert!(result.can_sign_in);
        let rendered = format!("{result:?}");
        assert!(!rendered.contains("super-secret"));
        let json = serde_json::to_string(&result).expect("serializes");
        assert!(!json.contains("super-secret"));
        assert!(!json.contains("clientSecret"));
    }

    #[tokio::test]
    async fn parse_account_id_validates_u128() {
        assert!(super::parse_account_id("12345").is_ok());
        assert!(matches!(
            super::parse_account_id("not-a-number"),
            Err(CommandError::AccountNotFound(_))
        ));
    }

    #[tokio::test]
    async fn disconnect_account_inner_clears_token_and_sets_disconnected_status() {
        let state = test_state(None).await;
        let account = ConnectedAccount::new(
            AccountId::new(1),
            GooglePermissionId::new("perm-1"),
            AccountProfile::new("a@gmail.com", "Alice", None),
        );
        state.account_store.connect(&account).await.unwrap();
        state
            .credential_store
            .save(
                AccountId::new(1),
                crate::application::RefreshToken::new("token".into()),
            )
            .unwrap();

        super::disconnect_account_inner(&state, AccountId::new(1))
            .await
            .unwrap();

        let updated = state
            .account_store
            .find_by_id(AccountId::new(1))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            updated.auth_status(),
            crate::domain::AuthStatus::Disconnected
        );
        assert!(
            state
                .credential_store
                .load(AccountId::new(1))
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn update_account_label_inner_updates_label_and_returns_dto() {
        let state = test_state(None).await;
        let account = ConnectedAccount::new(
            AccountId::new(1),
            GooglePermissionId::new("perm-1"),
            AccountProfile::new("a@gmail.com", "Alice", None),
        );
        state.account_store.connect(&account).await.unwrap();

        let dto = super::update_account_label_inner(
            &state,
            AccountId::new(1),
            Some("Personal Work".into()),
        )
        .await
        .unwrap();
        assert_eq!(dto.label.as_deref(), Some("Personal Work"));

        let cleared =
            super::update_account_label_inner(&state, AccountId::new(1), Some("  ".into()))
                .await
                .unwrap();
        assert_eq!(cleared.label, None);
    }

    #[tokio::test]
    async fn remove_account_inner_soft_deletes_account() {
        let state = test_state(None).await;
        let account = ConnectedAccount::new(
            AccountId::new(1),
            GooglePermissionId::new("perm-1"),
            AccountProfile::new("a@gmail.com", "Alice", None),
        );
        state.account_store.connect(&account).await.unwrap();
        state
            .credential_store
            .save(
                AccountId::new(1),
                crate::application::RefreshToken::new("token".into()),
            )
            .unwrap();

        super::remove_account_inner(&state, AccountId::new(1))
            .await
            .unwrap();

        assert!(
            state
                .account_store
                .find_by_id(AccountId::new(1))
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            state
                .credential_store
                .load(AccountId::new(1))
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn delete_local_account_data_inner_requires_confirmation() {
        let state = test_state(None).await;
        let err = super::delete_local_account_data_inner(&state, AccountId::new(1), false)
            .await
            .unwrap_err();
        assert!(matches!(err, CommandError::ConfirmationRequired(_)));
    }

    #[tokio::test]
    async fn delete_local_account_data_inner_hard_deletes_record() {
        let state = test_state(None).await;
        let account = ConnectedAccount::new(
            AccountId::new(1),
            GooglePermissionId::new("perm-1"),
            AccountProfile::new("a@gmail.com", "Alice", None),
        );
        state.account_store.connect(&account).await.unwrap();

        super::delete_local_account_data_inner(&state, AccountId::new(1), true)
            .await
            .unwrap();

        let any = state
            .account_store
            .find_any_by_permission_id(&GooglePermissionId::new("perm-1"))
            .await
            .unwrap();
        assert!(any.is_none());
    }
}
