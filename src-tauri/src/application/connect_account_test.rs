use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use crate::{
    application::{
        AccessToken, AccountIdentity, AccountStorePort, AccountStorePortError, ConnectAccountError,
        ConnectAccountService, IdentityLookupError, IdentityLookupPort, OAuthGrant, RefreshToken,
        RefreshTokenStore, RefreshTokenStoreError, TokenExchangeError, TokenExchangePort,
        TokenResponse,
    },
    domain::{AccountId, AccountProfile, ConnectedAccount, GooglePermissionId},
};

fn grant() -> OAuthGrant {
    OAuthGrant::new(
        "auth-code".to_owned(),
        "pkce-verifier".to_owned(),
        "http://127.0.0.1:8080".to_owned(),
    )
}

#[derive(Clone)]
struct MockTokenClient {
    response: Result<(String, Option<String>), TokenExchangeError>,
}

impl TokenExchangePort for MockTokenClient {
    async fn exchange_code(&self, _grant: OAuthGrant) -> Result<TokenResponse, TokenExchangeError> {
        self.response
            .clone()
            .map(|(access, refresh)| TokenResponse {
                access_token: AccessToken::new(access),
                refresh_token: refresh.map(RefreshToken::new),
            })
    }
}

#[derive(Clone)]
struct MockIdentityClient {
    response: Result<(String, String, String), IdentityLookupError>,
}

impl IdentityLookupPort for MockIdentityClient {
    async fn account_identity(
        &self,
        _token: &AccessToken,
    ) -> Result<AccountIdentity, IdentityLookupError> {
        self.response.clone().map(|(perm, email, name)| {
            AccountIdentity::new(GooglePermissionId::new(perm), email, name)
        })
    }
}

#[derive(Clone, Default)]
struct MockAccountStore {
    accounts: Arc<Mutex<HashMap<String, ConnectedAccount>>>,
    fail_remove: bool,
    fail_connect_after: Option<usize>,
    connect_count: Arc<AtomicUsize>,
}

impl AccountStorePort for MockAccountStore {
    async fn find_by_permission_id(
        &self,
        perm_id: &GooglePermissionId,
    ) -> Result<Option<ConnectedAccount>, AccountStorePortError> {
        let guard = self.accounts.lock().unwrap();
        Ok(guard.get(perm_id.as_str()).cloned())
    }

    async fn connect(
        &self,
        account: &ConnectedAccount,
    ) -> Result<ConnectedAccount, AccountStorePortError> {
        let count = self.connect_count.fetch_add(1, Ordering::SeqCst);
        if let Some(threshold) = self.fail_connect_after
            && count >= threshold
        {
            return Err(AccountStorePortError::Storage(
                "database failure during connect".to_owned(),
            ));
        }
        let mut guard = self.accounts.lock().unwrap();
        let perm = account.google_permission_id().as_str().to_owned();
        let final_account = if let Some(existing) = guard.get(&perm) {
            ConnectedAccount::new(
                existing.id(),
                GooglePermissionId::new(perm.clone()),
                AccountProfile::new(account.email(), account.display_name()),
            )
        } else {
            (*account).clone()
        };
        guard.insert(perm, final_account.clone());
        Ok(final_account)
    }

    async fn remove(&self, id: AccountId) -> Result<(), AccountStorePortError> {
        if self.fail_remove {
            return Err(AccountStorePortError::Storage(
                "database failure during remove".to_owned(),
            ));
        }
        let mut guard = self.accounts.lock().unwrap();
        guard.retain(|_, acc| acc.id() != id);
        Ok(())
    }

    async fn list_all(&self) -> Result<Vec<ConnectedAccount>, AccountStorePortError> {
        let guard = self.accounts.lock().unwrap();
        let mut accounts: Vec<ConnectedAccount> = guard.values().cloned().collect();
        accounts.sort_by(|a, b| a.email().cmp(b.email()));
        Ok(accounts)
    }
}

#[derive(Clone, Default)]
struct MockKeyring {
    tokens: Arc<Mutex<HashMap<u128, String>>>,
    fail_save: bool,
    fail_on_token: Option<String>,
}

impl RefreshTokenStore for MockKeyring {
    fn save(
        &self,
        account_id: AccountId,
        token: RefreshToken,
    ) -> Result<(), RefreshTokenStoreError> {
        if self.fail_save {
            return Err(RefreshTokenStoreError::Unavailable);
        }
        if let Some(ref fail_token) = self.fail_on_token
            && token.expose_secret() == fail_token
        {
            return Err(RefreshTokenStoreError::Unavailable);
        }
        let mut guard = self.tokens.lock().unwrap();
        guard.insert(account_id.value(), token.expose_secret().to_owned());
        Ok(())
    }

    fn load(&self, account_id: AccountId) -> Result<Option<RefreshToken>, RefreshTokenStoreError> {
        let guard = self.tokens.lock().unwrap();
        Ok(guard
            .get(&account_id.value())
            .cloned()
            .map(RefreshToken::new))
    }

    fn delete(&self, account_id: AccountId) -> Result<(), RefreshTokenStoreError> {
        let mut guard = self.tokens.lock().unwrap();
        guard.remove(&account_id.value());
        Ok(())
    }
}

#[tokio::test]
async fn connect_account_persists_new_account_and_keychain() {
    // Given
    let token_client = MockTokenClient {
        response: Ok(("access-1".to_owned(), Some("refresh-1".to_owned()))),
    };
    let identity_client = MockIdentityClient {
        response: Ok((
            "perm-1".to_owned(),
            "user@gmail.com".to_owned(),
            "User".to_owned(),
        )),
    };
    let account_store = MockAccountStore::default();
    let keyring = MockKeyring::default();

    let service = ConnectAccountService::new(
        token_client,
        identity_client,
        account_store.clone(),
        keyring.clone(),
    );

    // When
    let account = service
        .connect_account(grant(), AccountId::new(100))
        .await
        .expect("connection succeeds");

    // Then
    assert_eq!(account.id(), AccountId::new(100));
    assert_eq!(account.email(), "user@gmail.com");
    assert_eq!(account.display_name(), "User");
    assert_eq!(account.google_permission_id().as_str(), "perm-1");

    // Verify keychain
    let token = keyring
        .load(AccountId::new(100))
        .unwrap()
        .expect("token exists");
    assert_eq!(token.expose_secret(), "refresh-1");
}

#[tokio::test]
async fn connect_account_reconnect_preserves_id_and_updates_token() {
    // Given
    let account_store = MockAccountStore::default();
    let keyring = MockKeyring::default();

    // Seed existing account
    account_store
        .connect(&ConnectedAccount::new(
            AccountId::new(1),
            GooglePermissionId::new("perm-existing"),
            AccountProfile::new("old@gmail.com", "Old Name"),
        ))
        .await
        .unwrap();
    keyring
        .save(
            AccountId::new(1),
            RefreshToken::new("old-refresh".to_owned()),
        )
        .unwrap();

    let token_client = MockTokenClient {
        response: Ok(("access-2".to_owned(), Some("new-refresh".to_owned()))),
    };
    let identity_client = MockIdentityClient {
        response: Ok((
            "perm-existing".to_owned(),
            "new@gmail.com".to_owned(),
            "New Name".to_owned(),
        )),
    };

    let service = ConnectAccountService::new(
        token_client,
        identity_client,
        account_store.clone(),
        keyring.clone(),
    );

    // When
    let reconnected = service
        .connect_account(grant(), AccountId::new(999)) // ID 999 should be ignored
        .await
        .expect("reconnect succeeds");

    // Then
    assert_eq!(reconnected.id(), AccountId::new(1));
    assert_eq!(reconnected.email(), "new@gmail.com");
    assert_eq!(reconnected.display_name(), "New Name");

    let token = keyring
        .load(AccountId::new(1))
        .unwrap()
        .expect("token exists");
    assert_eq!(token.expose_secret(), "new-refresh");
}

#[tokio::test]
async fn connect_account_rolls_back_sqlite_when_keychain_fails() {
    // Given
    let token_client = MockTokenClient {
        response: Ok(("access-1".to_owned(), Some("refresh-1".to_owned()))),
    };
    let identity_client = MockIdentityClient {
        response: Ok((
            "perm-1".to_owned(),
            "user@gmail.com".to_owned(),
            "User".to_owned(),
        )),
    };
    let account_store = MockAccountStore::default();
    let keyring = MockKeyring {
        fail_save: true,
        ..Default::default()
    };

    let service = ConnectAccountService::new(
        token_client,
        identity_client,
        account_store.clone(),
        keyring.clone(),
    );

    // When
    let error = service
        .connect_account(grant(), AccountId::new(100))
        .await
        .expect_err("fails due to keychain");

    // Then
    assert!(matches!(error, ConnectAccountError::Keychain(_)));
    // Verify SQLite record was rolled back / removed
    let stored = account_store
        .find_by_permission_id(&GooglePermissionId::new("perm-1"))
        .await
        .unwrap();
    assert_eq!(stored, None);
}

#[tokio::test]
async fn connect_account_rejects_new_account_missing_refresh_token() {
    // Given
    let token_client = MockTokenClient {
        response: Ok(("access-1".to_owned(), None)), // No refresh token
    };
    let identity_client = MockIdentityClient {
        response: Ok((
            "perm-new".to_owned(),
            "user@gmail.com".to_owned(),
            "User".to_owned(),
        )),
    };
    let account_store = MockAccountStore::default();
    let keyring = MockKeyring::default();

    let service = ConnectAccountService::new(
        token_client,
        identity_client,
        account_store.clone(),
        keyring.clone(),
    );

    // When
    let error = service
        .connect_account(grant(), AccountId::new(100))
        .await
        .expect_err("fails because new account has no refresh token");

    // Then
    assert!(matches!(error, ConnectAccountError::MissingRefreshToken));
    let stored = account_store
        .find_by_permission_id(&GooglePermissionId::new("perm-new"))
        .await
        .unwrap();
    assert_eq!(stored, None);
}

#[tokio::test]
async fn reconnect_restores_previous_profile_when_keychain_save_fails() {
    // Given
    let account_store = MockAccountStore::default();

    // Seed existing account
    account_store
        .connect(&ConnectedAccount::new(
            AccountId::new(1),
            GooglePermissionId::new("perm-existing"),
            AccountProfile::new("old@gmail.com", "Old Name"),
        ))
        .await
        .unwrap();

    let keyring = MockKeyring {
        tokens: Arc::new(Mutex::new(HashMap::from([(
            1_u128,
            "old-refresh".to_owned(),
        )]))),
        fail_save: true,
        ..Default::default()
    };

    let token_client = MockTokenClient {
        response: Ok(("access-2".to_owned(), Some("new-refresh".to_owned()))),
    };
    let identity_client = MockIdentityClient {
        response: Ok((
            "perm-existing".to_owned(),
            "new@gmail.com".to_owned(),
            "New Name".to_owned(),
        )),
    };

    let service = ConnectAccountService::new(
        token_client,
        identity_client,
        account_store.clone(),
        keyring.clone(),
    );

    // When
    let error = service
        .connect_account(grant(), AccountId::new(999))
        .await
        .expect_err("fails due to keychain save");

    // Then
    assert!(matches!(error, ConnectAccountError::Keychain(_)));

    // Verify previous profile was restored
    let restored = account_store
        .find_by_permission_id(&GooglePermissionId::new("perm-existing"))
        .await
        .unwrap()
        .expect("account still exists");
    assert_eq!(restored.email(), "old@gmail.com");
    assert_eq!(restored.display_name(), "Old Name");

    // Verify old refresh token is still in the keyring
    let token = keyring
        .load(AccountId::new(1))
        .unwrap()
        .expect("old token still exists");
    assert_eq!(token.expose_secret(), "old-refresh");
}

#[tokio::test]
async fn reconnect_restores_previous_profile_when_refresh_token_missing_and_keyring_empty() {
    // Given
    let account_store = MockAccountStore::default();

    // Seed existing account
    account_store
        .connect(&ConnectedAccount::new(
            AccountId::new(1),
            GooglePermissionId::new("perm-existing"),
            AccountProfile::new("old@gmail.com", "Old Name"),
        ))
        .await
        .unwrap();

    // Keyring has NO token for account 1
    let keyring = MockKeyring::default();

    let token_client = MockTokenClient {
        response: Ok(("access-2".to_owned(), None)), // No refresh token
    };
    let identity_client = MockIdentityClient {
        response: Ok((
            "perm-existing".to_owned(),
            "new@gmail.com".to_owned(),
            "New Name".to_owned(),
        )),
    };

    let service = ConnectAccountService::new(
        token_client,
        identity_client,
        account_store.clone(),
        keyring.clone(),
    );

    // When
    let error = service
        .connect_account(grant(), AccountId::new(999))
        .await
        .expect_err("fails because no refresh token anywhere");

    // Then
    assert!(matches!(error, ConnectAccountError::MissingRefreshToken));

    // Verify previous profile was restored
    let restored = account_store
        .find_by_permission_id(&GooglePermissionId::new("perm-existing"))
        .await
        .unwrap()
        .expect("account still exists");
    assert_eq!(restored.email(), "old@gmail.com");
    assert_eq!(restored.display_name(), "Old Name");
}

#[tokio::test]
async fn connect_account_propagates_rollback_failure_on_new_account() {
    // Given: Keyring save will fail, and account store removal will also fail during rollback
    let token_client = MockTokenClient {
        response: Ok(("access-1".to_owned(), Some("refresh-1".to_owned()))),
    };
    let identity_client = MockIdentityClient {
        response: Ok((
            "perm-fail-rollback".to_owned(),
            "user@gmail.com".to_owned(),
            "User".to_owned(),
        )),
    };
    let account_store = MockAccountStore {
        fail_remove: true,
        ..Default::default()
    };
    let keyring = MockKeyring {
        fail_save: true,
        ..Default::default()
    };

    let service = ConnectAccountService::new(
        token_client,
        identity_client,
        account_store.clone(),
        keyring.clone(),
    );

    // When
    let error = service
        .connect_account(grant(), AccountId::new(100))
        .await
        .expect_err("fails and surfaces rollback failure");

    // Then
    match error {
        ConnectAccountError::RollbackFailed {
            primary_error,
            rollback_error,
        } => {
            assert!(matches!(*primary_error, ConnectAccountError::Keychain(_)));
            assert!(matches!(*rollback_error, AccountStorePortError::Storage(_)));
        }
        other => panic!("expected RollbackFailed, got {other:?}"),
    }
}

#[tokio::test]
async fn reconnect_propagates_rollback_failure_when_profile_restoration_fails() {
    // Given: Existing account, keyring save fails, and subsequent profile restoration fails
    let account_store = MockAccountStore::default();
    account_store
        .connect(&ConnectedAccount::new(
            AccountId::new(1),
            GooglePermissionId::new("perm-existing"),
            AccountProfile::new("old@gmail.com", "Old Name"),
        ))
        .await
        .unwrap();

    let keyring = MockKeyring {
        tokens: Arc::new(Mutex::new(HashMap::from([(
            1_u128,
            "old-refresh".to_owned(),
        )]))),
        fail_save: true,
        ..Default::default()
    };

    let token_client = MockTokenClient {
        response: Ok(("access-2".to_owned(), Some("new-refresh".to_owned()))),
    };
    let identity_client = MockIdentityClient {
        response: Ok((
            "perm-existing".to_owned(),
            "new@gmail.com".to_owned(),
            "New Name".to_owned(),
        )),
    };

    // Make connect fail on the second call during connect_account:
    // Call 0: seeding (done above)
    // Call 1: candidate account update in connect_account (succeeds)
    // Call 2: rollback restore in rollback_account (fails)
    let failing_account_store = MockAccountStore {
        accounts: account_store.accounts.clone(),
        fail_connect_after: Some(2),
        connect_count: Arc::new(AtomicUsize::new(1)), // seeding was 0, so next call is 1
        ..Default::default()
    };

    let service = ConnectAccountService::new(
        token_client,
        identity_client,
        failing_account_store,
        keyring,
    );

    // When
    let error = service
        .connect_account(grant(), AccountId::new(999))
        .await
        .expect_err("fails and surfaces rollback failure");

    // Then
    match error {
        ConnectAccountError::RollbackFailed {
            primary_error,
            rollback_error,
        } => {
            assert!(matches!(*primary_error, ConnectAccountError::Keychain(_)));
            assert!(matches!(*rollback_error, AccountStorePortError::Storage(_)));
        }
        other => panic!("expected RollbackFailed, got {other:?}"),
    }
}

#[tokio::test]
async fn concurrent_connects_for_same_identity_do_not_delete_successful_account() {
    // Given: Two connect requests for the same new permission ID.
    // Request 1 succeeds with refresh-1.
    // Request 2 has refresh-2 but fails on keychain save (triggering rollback).
    let account_store = MockAccountStore::default();
    let keyring = MockKeyring {
        fail_on_token: Some("refresh-2".to_owned()),
        ..Default::default()
    };

    let service = Arc::new(ConnectAccountService::new(
        MockTokenClient {
            response: Ok(("access-1".to_owned(), Some("refresh-1".to_owned()))),
        },
        MockIdentityClient {
            response: Ok((
                "perm-concurrent".to_owned(),
                "shared@gmail.com".to_owned(),
                "Shared".to_owned(),
            )),
        },
        account_store.clone(),
        keyring.clone(),
    ));

    let failing_service = Arc::new(ConnectAccountService::new(
        MockTokenClient {
            response: Ok(("access-2".to_owned(), Some("refresh-2".to_owned()))),
        },
        MockIdentityClient {
            response: Ok((
                "perm-concurrent".to_owned(),
                "shared@gmail.com".to_owned(),
                "Shared".to_owned(),
            )),
        },
        account_store.clone(),
        keyring.clone(),
    ));

    // When: Run both concurrently
    let h1 = {
        let s = service.clone();
        tokio::spawn(async move { s.connect_account(grant(), AccountId::new(100)).await })
    };

    let h2 = {
        let s = failing_service.clone();
        tokio::spawn(async move { s.connect_account(grant(), AccountId::new(101)).await })
    };

    let (res1, res2) = tokio::join!(h1, h2);
    let res1 = res1.unwrap();
    let res2 = res2.unwrap();

    // One succeeds, one fails due to Keychain
    assert!(res1.is_ok());
    assert!(matches!(
        res2.unwrap_err(),
        ConnectAccountError::Keychain(_)
    ));

    // Then: The successfully connected account MUST still exist in the store!
    let stored = account_store
        .find_by_permission_id(&GooglePermissionId::new("perm-concurrent"))
        .await
        .unwrap();
    assert!(
        stored.is_some(),
        "successful account must not be deleted by the failing concurrent request"
    );
    assert_eq!(stored.unwrap().email(), "shared@gmail.com");
}
