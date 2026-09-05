pub mod application;
mod commands;
pub mod domain;
pub mod infrastructure;
mod runtime;
pub mod state;

use std::sync::Arc;

use tauri::Manager;

use application::{
    ConnectAccountService, ConnectAccountUseCase, OAuthGrant, RefreshTokenStore, TokenExchangePort,
    TokenResponse,
};
use infrastructure::{
    account_store::SqliteAccountStore, google_drive::GoogleDriveClient,
    google_token::GoogleTokenClient,
};
use state::{AppState, OAuthConfig};

#[cfg(target_os = "windows")]
use infrastructure::secrets::WindowsCredentialStore;

struct DynamicTokenExchange {
    oauth_config: Arc<tokio::sync::RwLock<Option<OAuthConfig>>>,
}

impl TokenExchangePort for DynamicTokenExchange {
    async fn exchange_code(
        &self,
        grant: OAuthGrant,
    ) -> Result<TokenResponse, application::TokenExchangeError> {
        let config = {
            let guard = self.oauth_config.read().await;
            guard.clone()
        };
        let config = config.ok_or(application::TokenExchangeError::InvalidClient)?;
        let client = GoogleTokenClient::new(config.client_id, config.client_secret)
            .map_err(|_| application::TokenExchangeError::Transport)?;
        TokenExchangePort::exchange_code(&client, grant).await
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::account::list_accounts,
            commands::account::configure_oauth,
            commands::account::get_oauth_config,
            commands::account::connect_account,
        ])
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir().map_err(|e| {
                format!("failed to resolve app data directory: {e}")
            })?;

            std::fs::create_dir_all(&app_data_dir).map_err(|e| {
                format!("failed to create app data directory: {e}")
            })?;

            let db_path = app_data_dir.join("gdom.db");

            let account_store = tauri::async_runtime::block_on(
                SqliteAccountStore::open(&db_path),
            )
            .map_err(|e| format!("failed to open account database: {e}"))?;

            #[cfg(target_os = "windows")]
            let credential_store = Arc::new(
                WindowsCredentialStore::new()
                    .map_err(|e| format!("failed to initialize credential store: {e}"))?,
            );

            #[cfg(not(target_os = "windows"))]
            return Err("GDOM requires Windows — credential store adapters for other platforms are not yet available".into());

            let db_client_id = tauri::async_runtime::block_on(
                account_store.get_setting("oauth.client_id"),
            )
            .map_err(|e| format!("failed to read oauth.client_id: {e}"))?;

            let keychain_client_secret = credential_store
                .load_oauth_secret()
                .map_err(|e| format!("failed to read oauth secret from credential store: {e}"))?;

            let oauth_config = match (db_client_id, keychain_client_secret) {
                (Some(id), secret) if !id.trim().is_empty() => {
                    Some(OAuthConfig::new(id.trim(), secret))
                }
                _ => OAuthConfig::from_env(),
            };

            let account_store = Arc::new(account_store);
            let shared_oauth_config = Arc::new(tokio::sync::RwLock::new(oauth_config));

            let token_exchange = DynamicTokenExchange {
                oauth_config: Arc::clone(&shared_oauth_config),
            };

            let drive_client = GoogleDriveClient::new()
                .map_err(|e| format!("failed to initialize Google Drive client: {e}"))?;

            let service = ConnectAccountService::new(
                token_exchange,
                drive_client,
                Arc::clone(&account_store),
                Arc::clone(&credential_store),
            );

            let connect_account_use_case: Arc<dyn ConnectAccountUseCase> = Arc::new(service);

            #[allow(unreachable_code)]
            let state = AppState::new(
                account_store,
                credential_store,
                shared_oauth_config,
                connect_account_use_case,
            );

            app.manage(state);

            Ok(())
        });

    if let Err(error) = builder.run(tauri::generate_context!()) {
        eprintln!("failed to run GDOM: {error}");
    }
}
