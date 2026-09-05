use std::{
    collections::HashMap,
    error::Error,
    fmt,
    pin::Pin,
    sync::Arc,
    time::{Duration, Instant},
};

use tokio::sync::{Mutex, RwLock};

use crate::{
    application::{
        AccessToken, AccountStorePort, AccountStorePortError, RefreshToken, RefreshTokenStore,
        RefreshTokenStoreError,
    },
    domain::{AccountId, AuthStatus},
};

const PRE_EXPIRY_BUFFER: Duration = Duration::from_secs(300); // 5 minutes

#[derive(Clone)]
struct CachedToken {
    access_token: AccessToken,
    expires_at: Instant,
}

impl CachedToken {
    fn is_valid(&self) -> bool {
        Instant::now() + PRE_EXPIRY_BUFFER < self.expires_at
    }
}

pub type RefreshFuture<'a> = Pin<
    Box<
        dyn std::future::Future<Output = Result<(AccessToken, Duration), TokenRefreshError>>
            + Send
            + 'a,
    >,
>;

pub trait TokenRefreshPort: Send + Sync {
    fn refresh_token(&self, refresh_token: &RefreshToken) -> RefreshFuture<'_>;
}

#[derive(Debug)]
pub enum TokenRefreshError {
    InvalidGrant,
    InvalidClient,
    RateLimited,
    Unavailable,
    Transport,
    InvalidResponse,
    UnexpectedStatus(u16),
}

impl fmt::Display for TokenRefreshError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidGrant => write!(f, "refresh token is invalid or revoked"),
            Self::InvalidClient => write!(f, "invalid OAuth client credentials"),
            Self::RateLimited => write!(f, "rate limit exceeded during token refresh"),
            Self::Unavailable => write!(f, "token service is unavailable"),
            Self::Transport => write!(f, "network failure during token refresh"),
            Self::InvalidResponse => write!(f, "invalid response from token service"),
            Self::UnexpectedStatus(status) => write!(f, "unexpected status: {status}"),
        }
    }
}

impl Error for TokenRefreshError {}

#[derive(Debug)]
pub enum TokenProviderError {
    AccountNotFound,
    AccountDisconnected,
    AccountRemoved,
    ReauthRequired,
    MissingRefreshToken,
    Keychain(RefreshTokenStoreError),
    Database(AccountStorePortError),
    Refresh(TokenRefreshError),
}

impl fmt::Display for TokenProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AccountNotFound => write!(f, "account not found"),
            Self::AccountDisconnected => {
                write!(f, "account is disconnected and requires re-authentication")
            }
            Self::AccountRemoved => write!(f, "account has been removed"),
            Self::ReauthRequired => write!(f, "account requires re-authentication"),
            Self::MissingRefreshToken => write!(f, "no refresh token found for account"),
            Self::Keychain(err) => write!(f, "keychain error: {err}"),
            Self::Database(err) => write!(f, "database error: {err}"),
            Self::Refresh(err) => write!(f, "refresh token failed: {err}"),
        }
    }
}

impl Error for TokenProviderError {}

pub struct AccountTokenProvider<AccountPersistence> {
    tokens: RwLock<HashMap<AccountId, CachedToken>>,
    locks: RwLock<HashMap<AccountId, Arc<Mutex<()>>>>,
    refresh_port: Arc<dyn TokenRefreshPort>,
    credential_store: Arc<dyn RefreshTokenStore + Send + Sync>,
    account_store: Arc<AccountPersistence>,
}

impl<AccountPersistence> AccountTokenProvider<AccountPersistence>
where
    AccountPersistence: AccountStorePort + Send + Sync,
{
    pub fn new(
        refresh_port: Arc<dyn TokenRefreshPort>,
        credential_store: Arc<dyn RefreshTokenStore + Send + Sync>,
        account_store: Arc<AccountPersistence>,
    ) -> Self {
        Self {
            tokens: RwLock::new(HashMap::new()),
            locks: RwLock::new(HashMap::new()),
            refresh_port,
            credential_store,
            account_store,
        }
    }

    pub async fn get_access_token(
        &self,
        account_id: AccountId,
    ) -> Result<AccessToken, TokenProviderError> {
        // Fast path: check token cache with read lock
        {
            let guard = self.tokens.read().await;
            if let Some(cached) = guard.get(&account_id)
                && cached.is_valid()
            {
                return Ok(cached.access_token.clone());
            }
        }

        // Slow path: acquire per-account mutex
        let account_lock = self.get_account_lock(account_id).await;
        let _guard = account_lock.lock().await;

        // Double-check cache inside per-account mutex
        {
            let guard = self.tokens.read().await;
            if let Some(cached) = guard.get(&account_id)
                && cached.is_valid()
            {
                return Ok(cached.access_token.clone());
            }
        }

        // Perform token refresh
        self.refresh_access_token_locked(account_id).await
    }

    pub async fn refresh_access_token(
        &self,
        account_id: AccountId,
    ) -> Result<AccessToken, TokenProviderError> {
        let account_lock = self.get_account_lock(account_id).await;
        let _guard = account_lock.lock().await;
        self.refresh_access_token_locked(account_id).await
    }

    async fn refresh_access_token_locked(
        &self,
        account_id: AccountId,
    ) -> Result<AccessToken, TokenProviderError> {
        let account = self
            .account_store
            .find_by_id(account_id)
            .await
            .map_err(TokenProviderError::Database)?
            .ok_or(TokenProviderError::AccountNotFound)?;

        if account.auth_status() == AuthStatus::Disconnected {
            return Err(TokenProviderError::AccountDisconnected);
        }

        if account.auth_status() == AuthStatus::RemovalPending || account.removed_at().is_some() {
            return Err(TokenProviderError::AccountRemoved);
        }

        if account.auth_status() == AuthStatus::ReauthRequired {
            return Err(TokenProviderError::ReauthRequired);
        }

        let previous_status = account.auth_status();

        let _ = self
            .account_store
            .update_auth_status(account_id, AuthStatus::TokenRefreshing)
            .await;

        let refresh_token = self
            .credential_store
            .load(account_id)
            .map_err(TokenProviderError::Keychain)?
            .ok_or(TokenProviderError::MissingRefreshToken)?;

        let refresh_res = self.refresh_port.refresh_token(&refresh_token).await;

        match refresh_res {
            Ok((new_access_token, expires_in)) => {
                let expires_at = Instant::now() + expires_in;
                let cached = CachedToken {
                    access_token: new_access_token.clone(),
                    expires_at,
                };

                let is_still_active = match self.account_store.find_by_id(account_id).await {
                    Ok(Some(acc)) => {
                        acc.is_active() && acc.auth_status() != AuthStatus::Disconnected
                    }
                    _ => false,
                };

                if is_still_active {
                    let mut tokens = self.tokens.write().await;
                    tokens.insert(account_id, cached);
                    let _ = self.account_store.mark_last_authenticated(account_id).await;
                }

                Ok(new_access_token)
            }
            Err(TokenRefreshError::InvalidGrant) => {
                let _ = self
                    .account_store
                    .update_auth_status(account_id, AuthStatus::ReauthRequired)
                    .await;
                Err(TokenProviderError::Refresh(TokenRefreshError::InvalidGrant))
            }
            Err(err) => {
                let _ = self
                    .account_store
                    .update_auth_status(account_id, previous_status)
                    .await;
                Err(TokenProviderError::Refresh(err))
            }
        }
    }

    pub async fn mark_reauth_required(
        &self,
        account_id: AccountId,
    ) -> Result<(), TokenProviderError> {
        self.invalidate_cache(account_id).await;
        self.account_store
            .update_auth_status(account_id, AuthStatus::ReauthRequired)
            .await
            .map_err(TokenProviderError::Database)?;
        Ok(())
    }

    pub async fn invalidate_cache(&self, account_id: AccountId) {
        let mut tokens = self.tokens.write().await;
        tokens.remove(&account_id);
    }

    #[cfg(test)]
    pub async fn insert_cached_token_for_test(&self, account_id: AccountId, token: AccessToken) {
        let mut tokens = self.tokens.write().await;
        tokens.insert(
            account_id,
            CachedToken {
                access_token: token,
                expires_at: Instant::now() + Duration::from_secs(3600),
            },
        );
    }

    pub async fn remove_account_lifecycle(&self, account_id: AccountId) {
        self.invalidate_cache(account_id).await;
        let mut locks = self.locks.write().await;
        locks.remove(&account_id);
    }

    async fn get_account_lock(&self, account_id: AccountId) -> Arc<Mutex<()>> {
        // Fast read lock
        {
            let guard = self.locks.read().await;
            if let Some(lock) = guard.get(&account_id) {
                return Arc::clone(lock);
            }
        }

        // Upgrade to write lock
        let mut guard = self.locks.write().await;
        guard
            .entry(account_id)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{AccountProfile, ConnectedAccount, GooglePermissionId};
    use crate::infrastructure::account_store::SqliteAccountStore;
    use std::sync::Mutex as StdMutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct MockRefreshPort {
        call_count: AtomicUsize,
        should_fail_invalid_grant: bool,
        should_fail_transport: bool,
    }

    impl TokenRefreshPort for MockRefreshPort {
        fn refresh_token(&self, _refresh_token: &RefreshToken) -> RefreshFuture<'_> {
            Box::pin(async move {
                self.call_count.fetch_add(1, Ordering::SeqCst);
                if self.should_fail_invalid_grant {
                    return Err(TokenRefreshError::InvalidGrant);
                }
                if self.should_fail_transport {
                    return Err(TokenRefreshError::Transport);
                }
                Ok((
                    AccessToken::new("refreshed-token".to_string()),
                    Duration::from_secs(3600),
                ))
            })
        }
    }

    struct MockCredStore {
        tokens: StdMutex<HashMap<AccountId, RefreshToken>>,
    }

    impl MockCredStore {
        fn new() -> Self {
            Self {
                tokens: StdMutex::new(HashMap::new()),
            }
        }
    }

    impl RefreshTokenStore for MockCredStore {
        fn save(&self, id: AccountId, token: RefreshToken) -> Result<(), RefreshTokenStoreError> {
            let mut guard = self.tokens.lock().unwrap();
            guard.insert(id, token);
            Ok(())
        }

        fn load(&self, id: AccountId) -> Result<Option<RefreshToken>, RefreshTokenStoreError> {
            let guard = self.tokens.lock().unwrap();
            Ok(guard.get(&id).cloned())
        }

        fn delete(&self, id: AccountId) -> Result<(), RefreshTokenStoreError> {
            let mut guard = self.tokens.lock().unwrap();
            guard.remove(&id);
            Ok(())
        }
    }

    #[tokio::test]
    async fn token_provider_deduplicates_concurrent_refreshes_for_single_account() {
        let store = Arc::new(SqliteAccountStore::open_in_memory().await.unwrap());
        let account = ConnectedAccount::new(
            AccountId::new(1),
            GooglePermissionId::new("perm-1"),
            AccountProfile::new("a@gmail.com", "A"),
        );
        store.connect(&account).await.unwrap();

        let cred_store = Arc::new(MockCredStore::new());
        cred_store
            .save(
                AccountId::new(1),
                RefreshToken::new("valid-refresh".to_string()),
            )
            .unwrap();

        let refresh_port = Arc::new(MockRefreshPort {
            call_count: AtomicUsize::new(0),
            should_fail_invalid_grant: false,
            should_fail_transport: false,
        });

        let provider = Arc::new(AccountTokenProvider::new(
            Arc::clone(&refresh_port) as Arc<dyn TokenRefreshPort>,
            Arc::clone(&cred_store) as Arc<dyn RefreshTokenStore + Send + Sync>,
            store,
        ));

        let mut handles = Vec::new();
        for _ in 0..10 {
            let p = Arc::clone(&provider);
            handles.push(tokio::spawn(async move {
                p.get_access_token(AccountId::new(1)).await.unwrap()
            }));
        }

        for h in handles {
            let token = h.await.unwrap();
            assert_eq!(token.expose_secret(), "refreshed-token");
        }

        assert_eq!(refresh_port.call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn token_provider_allows_concurrent_refreshes_for_different_accounts() {
        let store = Arc::new(SqliteAccountStore::open_in_memory().await.unwrap());
        let acc1 = ConnectedAccount::new(
            AccountId::new(1),
            GooglePermissionId::new("perm-1"),
            AccountProfile::new("a@gmail.com", "A"),
        );
        let acc2 = ConnectedAccount::new(
            AccountId::new(2),
            GooglePermissionId::new("perm-2"),
            AccountProfile::new("b@gmail.com", "B"),
        );
        store.connect(&acc1).await.unwrap();
        store.connect(&acc2).await.unwrap();

        let cred_store = Arc::new(MockCredStore::new());
        cred_store
            .save(
                AccountId::new(1),
                RefreshToken::new("valid-refresh-1".to_string()),
            )
            .unwrap();
        cred_store
            .save(
                AccountId::new(2),
                RefreshToken::new("valid-refresh-2".to_string()),
            )
            .unwrap();

        let refresh_port = Arc::new(MockRefreshPort {
            call_count: AtomicUsize::new(0),
            should_fail_invalid_grant: false,
            should_fail_transport: false,
        });

        let provider = Arc::new(AccountTokenProvider::new(
            Arc::clone(&refresh_port) as Arc<dyn TokenRefreshPort>,
            Arc::clone(&cred_store) as Arc<dyn RefreshTokenStore + Send + Sync>,
            store,
        ));

        let t1 = provider.get_access_token(AccountId::new(1)).await.unwrap();
        let t2 = provider.get_access_token(AccountId::new(2)).await.unwrap();

        assert_eq!(t1.expose_secret(), "refreshed-token");
        assert_eq!(t2.expose_secret(), "refreshed-token");
        assert_eq!(refresh_port.call_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn token_provider_rejects_disconnected_account() {
        let store = Arc::new(SqliteAccountStore::open_in_memory().await.unwrap());
        let mut account = ConnectedAccount::new(
            AccountId::new(1),
            GooglePermissionId::new("perm-1"),
            AccountProfile::new("a@gmail.com", "A"),
        );
        account.set_auth_status(AuthStatus::Disconnected);
        store.connect(&account).await.unwrap();
        store
            .update_auth_status(AccountId::new(1), AuthStatus::Disconnected)
            .await
            .unwrap();

        let cred_store = Arc::new(MockCredStore::new());
        let refresh_port = Arc::new(MockRefreshPort {
            call_count: AtomicUsize::new(0),
            should_fail_invalid_grant: false,
            should_fail_transport: false,
        });

        let provider = AccountTokenProvider::new(refresh_port, cred_store, store);

        let err = provider
            .get_access_token(AccountId::new(1))
            .await
            .unwrap_err();
        assert!(matches!(err, TokenProviderError::AccountDisconnected));
    }

    #[tokio::test]
    async fn token_provider_rejects_reauth_required_account() {
        let store = Arc::new(SqliteAccountStore::open_in_memory().await.unwrap());
        let account = ConnectedAccount::new(
            AccountId::new(1),
            GooglePermissionId::new("perm-1"),
            AccountProfile::new("a@gmail.com", "A"),
        );
        store.connect(&account).await.unwrap();
        store
            .update_auth_status(AccountId::new(1), AuthStatus::ReauthRequired)
            .await
            .unwrap();

        let cred_store = Arc::new(MockCredStore::new());
        let refresh_port = Arc::new(MockRefreshPort {
            call_count: AtomicUsize::new(0),
            should_fail_invalid_grant: false,
            should_fail_transport: false,
        });

        let provider = AccountTokenProvider::new(refresh_port, cred_store, store);

        let err = provider
            .get_access_token(AccountId::new(1))
            .await
            .unwrap_err();
        assert!(matches!(err, TokenProviderError::ReauthRequired));
    }

    #[tokio::test]
    async fn token_provider_restores_previous_status_on_transient_error() {
        let store = Arc::new(SqliteAccountStore::open_in_memory().await.unwrap());
        let account = ConnectedAccount::new(
            AccountId::new(1),
            GooglePermissionId::new("perm-1"),
            AccountProfile::new("a@gmail.com", "A"),
        );
        store.connect(&account).await.unwrap();

        let cred_store = Arc::new(MockCredStore::new());
        cred_store
            .save(
                AccountId::new(1),
                RefreshToken::new("valid-refresh".to_string()),
            )
            .unwrap();

        let refresh_port = Arc::new(MockRefreshPort {
            call_count: AtomicUsize::new(0),
            should_fail_invalid_grant: false,
            should_fail_transport: true,
        });

        let provider = AccountTokenProvider::new(refresh_port, cred_store, store.clone());

        let err = provider
            .get_access_token(AccountId::new(1))
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            TokenProviderError::Refresh(TokenRefreshError::Transport)
        ));

        let reloaded = store.find_by_id(AccountId::new(1)).await.unwrap().unwrap();
        assert_eq!(reloaded.auth_status(), AuthStatus::Connected);
    }

    #[tokio::test]
    async fn token_provider_transitions_to_reauth_required_on_invalid_grant() {
        let store = Arc::new(SqliteAccountStore::open_in_memory().await.unwrap());
        let account = ConnectedAccount::new(
            AccountId::new(1),
            GooglePermissionId::new("perm-1"),
            AccountProfile::new("a@gmail.com", "A"),
        );
        store.connect(&account).await.unwrap();

        let cred_store = Arc::new(MockCredStore::new());
        cred_store
            .save(
                AccountId::new(1),
                RefreshToken::new("valid-refresh".to_string()),
            )
            .unwrap();

        let refresh_port = Arc::new(MockRefreshPort {
            call_count: AtomicUsize::new(0),
            should_fail_invalid_grant: true,
            should_fail_transport: false,
        });

        let provider = AccountTokenProvider::new(refresh_port, cred_store, store.clone());

        let err = provider
            .get_access_token(AccountId::new(1))
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            TokenProviderError::Refresh(TokenRefreshError::InvalidGrant)
        ));

        let reloaded = store.find_by_id(AccountId::new(1)).await.unwrap().unwrap();
        assert_eq!(reloaded.auth_status(), AuthStatus::ReauthRequired);
    }
}
