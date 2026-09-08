use std::sync::Arc;

use tokio::sync::{Mutex, RwLock};

use crate::{
    application::{
        OAuthGrant, RefreshFuture, RefreshToken, RefreshTokenStore, TokenExchangeError,
        TokenExchangePort, TokenRefreshError, TokenRefreshPort, TokenResponse,
    },
    state::OAuthConfig,
};

use super::{
    account_store::SqliteAccountStore,
    google_token::{GoogleTokenClient, GoogleTokenError},
    oauth_secret_recovery::recover_custom_oauth_secret,
};

struct OAuthSecretRecovery {
    account_store: Arc<SqliteAccountStore>,
    credential_store: Arc<dyn RefreshTokenStore>,
    authentication_lock: Arc<Mutex<()>>,
}

pub struct DynamicGoogleTokenClient {
    oauth_config: Arc<RwLock<Option<OAuthConfig>>>,
    secret_recovery: Option<OAuthSecretRecovery>,
    #[cfg(test)]
    token_endpoint: Option<String>,
}

impl DynamicGoogleTokenClient {
    pub fn new(oauth_config: Arc<RwLock<Option<OAuthConfig>>>) -> Self {
        Self {
            oauth_config,
            secret_recovery: None,
            #[cfg(test)]
            token_endpoint: None,
        }
    }

    pub fn with_secret_recovery(
        mut self,
        account_store: Arc<SqliteAccountStore>,
        credential_store: Arc<dyn RefreshTokenStore>,
        authentication_lock: Arc<Mutex<()>>,
    ) -> Self {
        self.secret_recovery = Some(OAuthSecretRecovery {
            account_store,
            credential_store,
            authentication_lock,
        });
        self
    }

    async fn configuration_for_refresh(&self) -> Result<OAuthConfig, TokenRefreshError> {
        let config = self
            .oauth_config
            .read()
            .await
            .clone()
            .ok_or(TokenRefreshError::InvalidClient)?;
        if config.has_client_secret() {
            return Ok(config);
        }
        let Some(recovery) = &self.secret_recovery else {
            return Ok(config);
        };
        // Refresh may hold a per-account token lock that reauthentication needs later.
        let _authentication = recovery
            .authentication_lock
            .try_lock()
            .map_err(|_| TokenRefreshError::Unavailable)?;
        recover_custom_oauth_secret(
            &self.oauth_config,
            &recovery.account_store,
            recovery.credential_store.as_ref(),
        )
        .await
        .map_err(|_| TokenRefreshError::Unavailable)?;
        self.oauth_config
            .read()
            .await
            .clone()
            .ok_or(TokenRefreshError::InvalidClient)
    }

    fn client_for_config(
        &self,
        config: OAuthConfig,
    ) -> Result<GoogleTokenClient, GoogleTokenError> {
        #[cfg(test)]
        if let Some(endpoint) = &self.token_endpoint {
            return GoogleTokenClient::for_test(
                endpoint.clone(),
                config.client_id,
                config.client_secret,
            );
        }
        GoogleTokenClient::new(config.client_id, config.client_secret)
    }
}

impl TokenExchangePort for DynamicGoogleTokenClient {
    async fn exchange_code(&self, grant: OAuthGrant) -> Result<TokenResponse, TokenExchangeError> {
        let config = {
            let guard = self.oauth_config.read().await;
            guard.clone()
        };
        let config = config.ok_or(TokenExchangeError::InvalidClient)?;
        if !config.has_client_secret() {
            return Err(TokenExchangeError::InvalidRequest);
        }
        let client = self
            .client_for_config(config)
            .map_err(|_| TokenExchangeError::Transport)?;
        TokenExchangePort::exchange_code(&client, grant).await
    }
}

impl TokenRefreshPort for DynamicGoogleTokenClient {
    fn refresh_token(&self, refresh_token: &RefreshToken) -> RefreshFuture<'_> {
        let refresh_token = refresh_token.clone();
        Box::pin(async move {
            let config = self.configuration_for_refresh().await?;
            if !config.has_client_secret() {
                return Err(TokenRefreshError::InvalidRequest);
            }
            let client = self
                .client_for_config(config)
                .map_err(|_| TokenRefreshError::Transport)?;
            let response = client
                .refresh_token(&refresh_token)
                .await
                .map_err(TokenRefreshError::from)?;
            Ok((response.access_token, response.expires_in))
        })
    }
}

#[cfg(test)]
#[path = "dynamic_google_token_test.rs"]
mod tests;
