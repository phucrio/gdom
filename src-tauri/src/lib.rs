pub mod application;
pub mod commands;
pub use gdom_migration as domain;
pub mod infrastructure;
pub mod runtime;
pub mod state;

#[cfg(test)]
pub mod test_support;

use std::sync::Arc;

use tauri::Manager;

use application::{
    AccountLifecycleUseCase, ConnectAccountService, ConnectAccountUseCase, RefreshTokenStore,
};
use infrastructure::{
    account_store::SqliteAccountStore, google_drive::GoogleDriveClient,
    google_token::DynamicGoogleTokenClient,
};
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
            commands::account::reset_oauth_config,
            commands::account::import_oauth_client,
            commands::account::connect_account,
            commands::account::disconnect_account,
            commands::account::update_account_label,
            commands::account::reauthenticate_account,
            commands::account::remove_account,
            commands::account::delete_local_account_data,
            commands::job::create_job,
            commands::job::update_draft_job_accounts,
            commands::job::get_job,
            commands::job::list_jobs,
            commands::job::delete_draft_job,
            commands::job::get_account_references,
            commands::job::validate_root,
            commands::job::add_root,
            commands::job::remove_root,
            commands::job::start_scan,
            commands::job::pause_scan,
            commands::job::list_job_items,
            commands::job::export_dry_run,
            commands::job::start_canary,
            commands::job::continue_migration,
            commands::job::pause_migration,
            commands::job::resume_migration,
            commands::job::cancel_migration,
            commands::job::retry_failed_items,
            commands::job::queue_job,
        ])
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir().map_err(|e| {
                format!("failed to resolve app data directory: {e}")
            })?;

            std::fs::create_dir_all(&app_data_dir).map_err(|e| {
                format!("failed to create app data directory: {e}")
            })?;

            let log_dir = app_data_dir.join(gdom_logs::LOG_DIR_NAME);
            let log_guard = gdom_logs::init_file_logging(&log_dir).map_err(|e| {
                format!("failed to initialize file logging: {e}")
            })?;
            tracing::info!(
                path = %gdom_logs::log_file_path(&log_dir).display(),
                "file logging initialized"
            );

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

            let oauth_config = Some(
                OAuthConfig::resolve(
                    db_client_id.as_deref(),
                    keychain_client_secret,
                    |key| std::env::var(key),
                )
                .0,
            );

            let account_store = Arc::new(account_store);
            let shared_oauth_config = Arc::new(tokio::sync::RwLock::new(oauth_config));

            let token_service = Arc::new(DynamicGoogleTokenClient::new(Arc::clone(
                &shared_oauth_config,
            )));

            let drive_client = GoogleDriveClient::new()
                .map_err(|e| format!("failed to initialize Google Drive client: {e}"))?;

            let connect_service = ConnectAccountService::new(
                token_service.clone(),
                drive_client.clone(),
                Arc::clone(&account_store),
                Arc::clone(&credential_store),
            );
            let connect_account_use_case: Arc<dyn ConnectAccountUseCase> =
                Arc::new(connect_service);

            let job_store = Arc::new(infrastructure::SqliteJobStore::new(
                account_store.pool().clone(),
            ));

            let lifecycle_service = application::AccountLifecycleService::new(
                token_service.clone(),
                drive_client.clone(),
                Arc::clone(&account_store),
                Arc::clone(&credential_store),
            )
            .with_job_store(Arc::clone(&job_store) as Arc<dyn application::job_store::JobStorePort>);

            let account_lifecycle_use_case: Arc<dyn AccountLifecycleUseCase> =
                Arc::new(lifecycle_service);

            let token_provider = Arc::new(application::AccountTokenProvider::new(
                token_service,
                Arc::clone(&credential_store) as Arc<dyn RefreshTokenStore + Send + Sync>,
                Arc::clone(&account_store),
            ));

            let job_service = Arc::new(
                application::JobService::new(
                    Arc::clone(&account_store),
                    Arc::clone(&job_store),
                    Arc::new(drive_client) as Arc<dyn application::DrivePort>,
                    Arc::clone(&token_provider),
                )
                .with_event_sink(Arc::new(runtime::TauriJobEventSink::new(
                    app.handle().clone(),
                ))),
            );

            tauri::async_runtime::block_on(job_service.reconcile_on_startup()).map_err(|e| {
                tracing::error!("failed to reconcile unfinished migration jobs on startup: {e}");
                format!("failed to reconcile unfinished migration jobs on startup: {e}")
            })?;

            #[allow(unreachable_code)]
            let state = AppState::new(
                account_store,
                credential_store,
                shared_oauth_config,
                connect_account_use_case,
                account_lifecycle_use_case,
                token_provider,
                job_store,
                job_service,
            );

            app.manage(log_guard);
            app.manage(state);

            Ok(())
        });

    if let Err(error) = builder.run(tauri::generate_context!()) {
        tracing::error!("failed to run GDOM: {error}");
        eprintln!("failed to run GDOM: {error}");
    }
}
