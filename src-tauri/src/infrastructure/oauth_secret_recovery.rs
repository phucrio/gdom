use tokio::sync::RwLock;

use crate::{application::RefreshTokenStore, state::OAuthConfig};

use super::account_store::{AccountStoreError, SqliteAccountStore};

pub(crate) enum OAuthSecretRecoveryError {
    Database(AccountStoreError),
    Keychain,
}

// Call while holding the authentication mutex so configure/reset cannot change the pair.
pub(crate) async fn recover_custom_oauth_secret(
    oauth_config: &RwLock<Option<OAuthConfig>>,
    account_store: &SqliteAccountStore,
    credential_store: &dyn RefreshTokenStore,
) -> Result<(), OAuthSecretRecoveryError> {
    let client_id = {
        let configuration = oauth_config.read().await;
        let Some(config) = configuration.as_ref() else {
            return Ok(());
        };
        if config.has_client_secret() {
            return Ok(());
        }
        config.client_id.clone()
    };
    let stored_client_id = account_store
        .get_setting("oauth.client_id")
        .await
        .map_err(OAuthSecretRecoveryError::Database)?;
    if stored_client_id.as_deref().map(str::trim) == Some(client_id.as_str()) {
        let secret = credential_store
            .load_oauth_secret()
            .map_err(|_| OAuthSecretRecoveryError::Keychain)?;
        if let Some(config) = oauth_config.write().await.as_mut() {
            config.client_secret = secret;
        }
    }
    Ok(())
}
