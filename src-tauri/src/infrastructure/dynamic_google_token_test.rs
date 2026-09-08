use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use super::*;
use crate::application::RefreshTokenStoreError;
use crate::domain::AccountId;
use crate::infrastructure::google_token_test::serve_once;

struct RecoveringCredentialStore {
    locked: AtomicBool,
}

impl RefreshTokenStore for RecoveringCredentialStore {
    fn save(&self, _: AccountId, _: RefreshToken) -> Result<(), RefreshTokenStoreError> {
        unreachable!("the refresh adapter must not write refresh tokens")
    }

    fn load(&self, _: AccountId) -> Result<Option<RefreshToken>, RefreshTokenStoreError> {
        unreachable!("the caller supplies the refresh token")
    }

    fn delete(&self, _: AccountId) -> Result<(), RefreshTokenStoreError> {
        unreachable!("the refresh adapter must not delete credentials")
    }

    fn load_oauth_secret(&self) -> Result<Option<String>, RefreshTokenStoreError> {
        if self.locked.load(Ordering::SeqCst) {
            Err(RefreshTokenStoreError::Unavailable)
        } else {
            Ok(Some("synthetic-custom-secret".into()))
        }
    }
}

#[tokio::test]
async fn refresh_recovers_custom_secret_after_unlock_without_sign_in() {
    let accounts = Arc::new(SqliteAccountStore::open_in_memory().await.unwrap());
    accounts
        .save_oauth_client_id("custom-client")
        .await
        .unwrap();
    let credentials = Arc::new(RecoveringCredentialStore {
        locked: AtomicBool::new(true),
    });
    let config = Arc::new(RwLock::new(Some(OAuthConfig::new("custom-client", None))));
    let mut client = DynamicGoogleTokenClient::new(config.clone()).with_secret_recovery(
        accounts,
        credentials.clone(),
        Arc::new(Mutex::new(())),
    );
    client.token_endpoint = Some("http://127.0.0.1:0".into());
    let refresh_token = RefreshToken::new("synthetic-refresh-token".into());
    assert!(matches!(
        client.refresh_token(&refresh_token).await,
        Err(TokenRefreshError::Unavailable)
    ));
    let (endpoint, request) = serve_once(
        "200 OK",
        r#"{"access_token":"synthetic-access-token","expires_in":3600,"token_type":"Bearer"}"#,
    );
    client.token_endpoint = Some(endpoint);

    credentials.locked.store(false, Ordering::SeqCst);
    let (token, lifetime) =
        tokio::time::timeout(Duration::from_secs(3), client.refresh_token(&refresh_token))
            .await
            .unwrap()
            .unwrap();

    assert_eq!(token.expose_secret(), "synthetic-access-token");
    assert_eq!(lifetime, Duration::from_secs(3600));
    let request = request
        .recv_timeout(Duration::from_secs(3))
        .unwrap()
        .unwrap();
    assert!(request.contains("client_id=custom-client"));
    assert!(request.contains("client_secret=synthetic-custom-secret"));
    assert!(request.contains("refresh_token=synthetic-refresh-token"));
    assert!(config.read().await.as_ref().unwrap().has_client_secret());
}

#[tokio::test]
async fn refresh_does_not_load_a_secret_for_a_different_client() {
    let accounts = Arc::new(SqliteAccountStore::open_in_memory().await.unwrap());
    accounts
        .save_oauth_client_id("stored-client")
        .await
        .unwrap();
    let credentials = Arc::new(RecoveringCredentialStore {
        locked: AtomicBool::new(true),
    });
    let config = Arc::new(RwLock::new(Some(OAuthConfig::new(
        "environment-client",
        None,
    ))));
    let mut client = DynamicGoogleTokenClient::new(config.clone()).with_secret_recovery(
        accounts,
        credentials,
        Arc::new(Mutex::new(())),
    );
    client.token_endpoint = Some("http://127.0.0.1:0".into());

    let result = client
        .refresh_token(&RefreshToken::new("synthetic-refresh-token".into()))
        .await;

    assert!(matches!(result, Err(TokenRefreshError::InvalidRequest)));
    assert!(!config.read().await.as_ref().unwrap().has_client_secret());
}

#[tokio::test]
async fn refresh_retries_current_client_after_configuration_lock_is_released() {
    let accounts = Arc::new(SqliteAccountStore::open_in_memory().await.unwrap());
    accounts.save_oauth_client_id("old-client").await.unwrap();
    let credentials = Arc::new(RecoveringCredentialStore {
        locked: AtomicBool::new(false),
    });
    let config = Arc::new(RwLock::new(Some(OAuthConfig::new("old-client", None))));
    let authentication_lock = Arc::new(Mutex::new(()));
    let mut client = DynamicGoogleTokenClient::new(config.clone()).with_secret_recovery(
        accounts.clone(),
        credentials,
        authentication_lock.clone(),
    );
    client.token_endpoint = Some("http://127.0.0.1:0".into());
    let refresh_token = RefreshToken::new("synthetic-refresh-token".into());
    let authentication = authentication_lock.lock().await;
    let busy_result =
        tokio::time::timeout(Duration::from_secs(1), client.refresh_token(&refresh_token))
            .await
            .unwrap();
    assert!(matches!(busy_result, Err(TokenRefreshError::Unavailable)));
    accounts.save_oauth_client_id("new-client").await.unwrap();
    *config.write().await = Some(OAuthConfig::new("new-client", None));
    let (endpoint, request) = serve_once(
        "200 OK",
        r#"{"access_token":"synthetic-access-token","expires_in":3600,"token_type":"Bearer"}"#,
    );
    client.token_endpoint = Some(endpoint);

    drop(authentication);
    let result = tokio::time::timeout(Duration::from_secs(3), client.refresh_token(&refresh_token))
        .await
        .unwrap();

    assert!(result.is_ok());
    let request = request
        .recv_timeout(Duration::from_secs(3))
        .unwrap()
        .unwrap();
    assert!(request.contains("client_id=new-client"));
    assert!(!request.contains("client_id=old-client"));
}

#[tokio::test]
async fn refresh_with_cached_secret_does_not_wait_for_authentication_or_keychain() {
    let accounts = Arc::new(SqliteAccountStore::open_in_memory().await.unwrap());
    let credentials = Arc::new(RecoveringCredentialStore {
        locked: AtomicBool::new(true),
    });
    let config = Arc::new(RwLock::new(Some(OAuthConfig::new(
        "configured-client",
        Some("cached-secret".into()),
    ))));
    let authentication_lock = Arc::new(Mutex::new(()));
    let mut client = DynamicGoogleTokenClient::new(config).with_secret_recovery(
        accounts,
        credentials,
        authentication_lock.clone(),
    );
    client.token_endpoint = Some("http://127.0.0.1:0".into());
    let _authentication = authentication_lock.lock().await;
    let (endpoint, request) = serve_once(
        "200 OK",
        r#"{"access_token":"synthetic-access-token","expires_in":3600,"token_type":"Bearer"}"#,
    );
    client.token_endpoint = Some(endpoint);

    let result = tokio::time::timeout(
        Duration::from_secs(3),
        client.refresh_token(&RefreshToken::new("synthetic-refresh-token".into())),
    )
    .await
    .unwrap();

    assert!(result.is_ok());
    assert!(
        request
            .recv_timeout(Duration::from_secs(3))
            .unwrap()
            .unwrap()
            .contains("client_secret=cached-secret")
    );
}
