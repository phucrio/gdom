pub mod application;
mod commands;
pub mod domain;
pub mod infrastructure;
mod runtime;
pub mod state;

use std::sync::Arc;

use tauri::Manager;

use infrastructure::account_store::SqliteAccountStore;
use state::{AppState, OAuthConfig};

#[cfg(target_os = "windows")]
use infrastructure::secrets::WindowsCredentialStore;

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

            let oauth_config = OAuthConfig::from_env();

            #[allow(unreachable_code)]
            let state = AppState::new(
                Arc::new(account_store),
                credential_store,
                oauth_config,
            );

            app.manage(state);

            Ok(())
        });

    if let Err(error) = builder.run(tauri::generate_context!()) {
        eprintln!("failed to run GDOM: {error}");
    }
}
