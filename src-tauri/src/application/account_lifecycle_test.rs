use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{
    application::{
        AccessToken, AccountIdentity, AccountLifecycleError, AccountLifecycleService,
        AccountStorePort, AccountStorePortError, IdentityLookupError, IdentityLookupPort,
        OAuthGrant, RefreshToken, RefreshTokenStore, RefreshTokenStoreError, TokenExchangeError,
        TokenExchangePort, TokenResponse,
    },
    domain::{
        AccountId, AccountLabel, AccountProfile, AuthStatus, ConnectedAccount, GooglePermissionId,
    },
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
    fail_store: bool,
}

impl AccountStorePort for MockAccountStore {
    async fn find_by_permission_id(
        &self,
        perm_id: &GooglePermissionId,
    ) -> Result<Option<ConnectedAccount>, AccountStorePortError> {
        let guard = self.accounts.lock().unwrap();
        Ok(guard.get(perm_id.as_str()).cloned())
    }

    async fn find_by_id(
        &self,
        account_id: AccountId,
    ) -> Result<Option<ConnectedAccount>, AccountStorePortError> {
        if self.fail_store {
            return Err(AccountStorePortError::Storage("db error".into()));
        }
        let guard = self.accounts.lock().unwrap();
        Ok(guard.values().find(|acc| acc.id() == account_id).cloned())
    }

    async fn connect(
        &self,
        account: &ConnectedAccount,
    ) -> Result<ConnectedAccount, AccountStorePortError> {
        if self.fail_store {
            return Err(AccountStorePortError::Storage("db error".into()));
        }
        let mut guard = self.accounts.lock().unwrap();
        let perm = account.google_permission_id().as_str().to_owned();
        guard.insert(perm, (*account).clone());
        Ok((*account).clone())
    }

    async fn update_auth_status(
        &self,
        account_id: AccountId,
        status: AuthStatus,
    ) -> Result<(), AccountStorePortError> {
        if self.fail_store {
            return Err(AccountStorePortError::Storage("db error".into()));
        }
        let mut guard = self.accounts.lock().unwrap();
        if let Some(acc) = guard.values_mut().find(|acc| acc.id() == account_id) {
            acc.set_auth_status(status);
        }
        Ok(())
    }

    async fn update_label(
        &self,
        account_id: AccountId,
        label: Option<&AccountLabel>,
    ) -> Result<(), AccountStorePortError> {
        if self.fail_store {
            return Err(AccountStorePortError::Storage("db error".into()));
        }
        let mut guard = self.accounts.lock().unwrap();
        if let Some(acc) = guard.values_mut().find(|acc| acc.id() == account_id) {
            acc.set_label(label.cloned());
        }
        Ok(())
    }

    async fn mark_last_authenticated(
        &self,
        account_id: AccountId,
    ) -> Result<(), AccountStorePortError> {
        if self.fail_store {
            return Err(AccountStorePortError::Storage("db error".into()));
        }
        let mut guard = self.accounts.lock().unwrap();
        if let Some(acc) = guard.values_mut().find(|acc| acc.id() == account_id) {
            acc.set_auth_status(AuthStatus::Connected);
        }
        Ok(())
    }

    async fn remove(&self, id: AccountId) -> Result<(), AccountStorePortError> {
        if self.fail_store {
            return Err(AccountStorePortError::Storage("db error".into()));
        }
        let mut guard = self.accounts.lock().unwrap();
        guard.retain(|_, acc| acc.id() != id);
        Ok(())
    }

    async fn hard_delete(&self, id: AccountId) -> Result<(), AccountStorePortError> {
        self.remove(id).await
    }

    async fn list_all(&self) -> Result<Vec<ConnectedAccount>, AccountStorePortError> {
        if self.fail_store {
            return Err(AccountStorePortError::Storage("db error".into()));
        }
        let guard = self.accounts.lock().unwrap();
        let mut accounts: Vec<ConnectedAccount> = guard.values().cloned().collect();
        accounts.sort_by(|a, b| a.email().cmp(b.email()));
        Ok(accounts)
    }
}

#[derive(Clone, Default)]
struct MockKeyring {
    tokens: Arc<Mutex<HashMap<u128, String>>>,
    fail_delete: bool,
    fail_save: bool,
}

impl RefreshTokenStore for MockKeyring {
    fn save(&self, id: AccountId, token: RefreshToken) -> Result<(), RefreshTokenStoreError> {
        if self.fail_save {
            return Err(RefreshTokenStoreError::Unavailable);
        }
        let mut guard = self.tokens.lock().unwrap();
        guard.insert(id.value(), token.expose_secret().to_owned());
        Ok(())
    }

    fn load(&self, id: AccountId) -> Result<Option<RefreshToken>, RefreshTokenStoreError> {
        let guard = self.tokens.lock().unwrap();
        Ok(guard.get(&id.value()).map(|t| RefreshToken::new(t.clone())))
    }

    fn delete(&self, id: AccountId) -> Result<(), RefreshTokenStoreError> {
        if self.fail_delete {
            return Err(RefreshTokenStoreError::Unavailable);
        }
        let mut guard = self.tokens.lock().unwrap();
        guard.remove(&id.value());
        Ok(())
    }
}

fn build_service(
    token_client: MockTokenClient,
    identity_client: MockIdentityClient,
    account_store: MockAccountStore,
    keyring: MockKeyring,
) -> AccountLifecycleService<MockTokenClient, MockIdentityClient, MockAccountStore, MockKeyring> {
    AccountLifecycleService::new(token_client, identity_client, account_store, keyring)
}

#[tokio::test]
async fn list_accounts_returns_all_registered_accounts() {
    let store = MockAccountStore::default();
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
    store.connect(&account_a).await.unwrap();
    store.connect(&account_b).await.unwrap();

    let service = build_service(
        MockTokenClient {
            response: Ok(("token".into(), None)),
        },
        MockIdentityClient {
            response: Ok(("perm".into(), "email".into(), "name".into())),
        },
        store,
        MockKeyring::default(),
    );

    let accounts = service.list_accounts().await.unwrap();
    assert_eq!(accounts.len(), 2);
    assert_eq!(accounts[0].email(), "a@gmail.com");
    assert_eq!(accounts[1].email(), "b@gmail.com");
}

#[tokio::test]
async fn update_account_label_sets_and_clears_label() {
    let store = MockAccountStore::default();
    let account = ConnectedAccount::new(
        AccountId::new(1),
        GooglePermissionId::new("perm-1"),
        AccountProfile::new("user@gmail.com", "User"),
    );
    store.connect(&account).await.unwrap();

    let service = build_service(
        MockTokenClient {
            response: Ok(("token".into(), None)),
        },
        MockIdentityClient {
            response: Ok(("perm".into(), "email".into(), "name".into())),
        },
        store.clone(),
        MockKeyring::default(),
    );

    let updated = service
        .update_account_label(AccountId::new(1), Some("Personal Archive".into()))
        .await
        .unwrap();
    assert_eq!(
        updated.label().map(AccountLabel::as_str),
        Some("Personal Archive")
    );

    let cleared = service
        .update_account_label(AccountId::new(1), Some("".into()))
        .await
        .unwrap();
    assert_eq!(cleared.label(), None);
}

#[tokio::test]
async fn update_account_label_rejects_nonexistent_account() {
    let service = build_service(
        MockTokenClient {
            response: Ok(("token".into(), None)),
        },
        MockIdentityClient {
            response: Ok(("perm".into(), "email".into(), "name".into())),
        },
        MockAccountStore::default(),
        MockKeyring::default(),
    );

    let err = service
        .update_account_label(AccountId::new(999), Some("Label".into()))
        .await
        .unwrap_err();
    assert!(matches!(err, AccountLifecycleError::AccountNotFound));
}

#[tokio::test]
async fn disconnect_account_purges_keychain_and_updates_status() {
    let store = MockAccountStore::default();
    let keyring = MockKeyring::default();
    let account = ConnectedAccount::new(
        AccountId::new(1),
        GooglePermissionId::new("perm-1"),
        AccountProfile::new("user@gmail.com", "User"),
    );
    store.connect(&account).await.unwrap();
    keyring
        .save(AccountId::new(1), RefreshToken::new("secret-token".into()))
        .unwrap();

    let service = build_service(
        MockTokenClient {
            response: Ok(("token".into(), None)),
        },
        MockIdentityClient {
            response: Ok(("perm".into(), "email".into(), "name".into())),
        },
        store.clone(),
        keyring.clone(),
    );

    service.disconnect_account(AccountId::new(1)).await.unwrap();

    assert!(keyring.load(AccountId::new(1)).unwrap().is_none());
    let reloaded = store.find_by_id(AccountId::new(1)).await.unwrap().unwrap();
    assert_eq!(reloaded.auth_status(), AuthStatus::Disconnected);
}

#[tokio::test]
async fn disconnect_account_propagates_keychain_failure() {
    let store = MockAccountStore::default();
    let account = ConnectedAccount::new(
        AccountId::new(1),
        GooglePermissionId::new("perm-1"),
        AccountProfile::new("user@gmail.com", "User"),
    );
    store.connect(&account).await.unwrap();

    let keyring = MockKeyring {
        tokens: Arc::new(Mutex::new(HashMap::new())),
        fail_delete: true,
        fail_save: false,
    };

    let service = build_service(
        MockTokenClient {
            response: Ok(("token".into(), None)),
        },
        MockIdentityClient {
            response: Ok(("perm".into(), "email".into(), "name".into())),
        },
        store,
        keyring,
    );

    let err = service
        .disconnect_account(AccountId::new(1))
        .await
        .unwrap_err();
    assert!(matches!(err, AccountLifecycleError::Keychain(_)));
}

#[tokio::test]
async fn reauthenticate_account_updates_token_and_reconnects() {
    let store = MockAccountStore::default();
    let keyring = MockKeyring::default();
    let mut account = ConnectedAccount::new(
        AccountId::new(1),
        GooglePermissionId::new("perm-1"),
        AccountProfile::new("user@gmail.com", "User"),
    );
    account.set_auth_status(AuthStatus::ReauthRequired);
    store.connect(&account).await.unwrap();

    let service = build_service(
        MockTokenClient {
            response: Ok(("new-access".into(), Some("new-refresh".into()))),
        },
        MockIdentityClient {
            response: Ok((
                "perm-1".into(),
                "user@gmail.com".into(),
                "User Updated".into(),
            )),
        },
        store.clone(),
        keyring.clone(),
    );

    let updated = service
        .reauthenticate_account(AccountId::new(1), grant())
        .await
        .unwrap();

    assert_eq!(updated.display_name(), "User Updated");
    assert_eq!(
        keyring
            .load(AccountId::new(1))
            .unwrap()
            .unwrap()
            .expose_secret(),
        "new-refresh"
    );
}

#[tokio::test]
async fn reauthenticate_account_rejects_identity_mismatch() {
    let store = MockAccountStore::default();
    let keyring = MockKeyring::default();
    let account = ConnectedAccount::new(
        AccountId::new(1),
        GooglePermissionId::new("perm-expected"),
        AccountProfile::new("user@gmail.com", "User"),
    );
    store.connect(&account).await.unwrap();
    keyring
        .save(
            AccountId::new(1),
            RefreshToken::new("original-token".into()),
        )
        .unwrap();

    let service = build_service(
        MockTokenClient {
            response: Ok(("access".into(), Some("intruder-token".into()))),
        },
        MockIdentityClient {
            response: Ok((
                "perm-different".into(),
                "other@gmail.com".into(),
                "Other".into(),
            )),
        },
        store,
        keyring.clone(),
    );

    let err = service
        .reauthenticate_account(AccountId::new(1), grant())
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        AccountLifecycleError::IdentityMismatch { .. }
    ));
    assert_eq!(
        keyring
            .load(AccountId::new(1))
            .unwrap()
            .unwrap()
            .expose_secret(),
        "original-token"
    );
}

#[tokio::test]
async fn remove_account_purges_keychain_and_removes_from_store() {
    let store = MockAccountStore::default();
    let keyring = MockKeyring::default();
    let account = ConnectedAccount::new(
        AccountId::new(1),
        GooglePermissionId::new("perm-1"),
        AccountProfile::new("user@gmail.com", "User"),
    );
    store.connect(&account).await.unwrap();
    keyring
        .save(AccountId::new(1), RefreshToken::new("token".into()))
        .unwrap();

    let service = build_service(
        MockTokenClient {
            response: Ok(("token".into(), None)),
        },
        MockIdentityClient {
            response: Ok(("perm".into(), "email".into(), "name".into())),
        },
        store.clone(),
        keyring.clone(),
    );

    service.remove_account(AccountId::new(1)).await.unwrap();

    assert!(keyring.load(AccountId::new(1)).unwrap().is_none());
    assert!(store.find_by_id(AccountId::new(1)).await.unwrap().is_none());
}

#[tokio::test]
async fn delete_local_account_data_purges_keychain_and_store() {
    let store = MockAccountStore::default();
    let keyring = MockKeyring::default();
    let account = ConnectedAccount::new(
        AccountId::new(1),
        GooglePermissionId::new("perm-1"),
        AccountProfile::new("user@gmail.com", "User"),
    );
    store.connect(&account).await.unwrap();
    keyring
        .save(AccountId::new(1), RefreshToken::new("token".into()))
        .unwrap();

    let service = build_service(
        MockTokenClient {
            response: Ok(("token".into(), None)),
        },
        MockIdentityClient {
            response: Ok(("perm".into(), "email".into(), "name".into())),
        },
        store.clone(),
        keyring.clone(),
    );

    service
        .delete_local_account_data(AccountId::new(1))
        .await
        .unwrap();

    assert!(keyring.load(AccountId::new(1)).unwrap().is_none());
    assert!(store.find_by_id(AccountId::new(1)).await.unwrap().is_none());
}
