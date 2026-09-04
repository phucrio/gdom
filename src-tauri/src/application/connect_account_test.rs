use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::{
    application::{
        AccessToken, RefreshToken, RefreshTokenStore, RefreshTokenStoreError,
        connect_account::{
            AccountStorePort, ConnectAccountError, ConnectAccountService, DriveIdentityPort,
            TokenExchangePort,
        },
    },
    domain::{AccountId, AccountProfile, ConnectedAccount, GooglePermissionId},
    infrastructure::{
        account_store::AccountStoreError,
        google_drive::{DriveAccountIdentity, GoogleDriveError},
        google_oauth::OAuthGrant,
        google_token::{GoogleTokenError, GoogleTokenResponse},
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
    response: Result<(String, Option<String>), GoogleTokenError>,
}

impl TokenExchangePort for MockTokenClient {
    async fn exchange_code(
        &self,
        _grant: OAuthGrant,
    ) -> Result<GoogleTokenResponse, GoogleTokenError> {
        self.response
            .clone()
            .map(|(access, refresh)| GoogleTokenResponse {
                access_token: AccessToken::new(access),
                expires_in: Duration::from_secs(3600),
                refresh_token: refresh.map(RefreshToken::new),
                token_type: "Bearer".to_owned(),
                scope: None,
            })
    }
}

#[derive(Clone)]
struct MockDriveClient {
    response: Result<(String, String, String), GoogleDriveError>,
}

impl DriveIdentityPort for MockDriveClient {
    async fn account_identity(
        &self,
        _token: &AccessToken,
    ) -> Result<DriveAccountIdentity, GoogleDriveError> {
        self.response.clone().map(|(perm, email, name)| {
            DriveAccountIdentity::new(GooglePermissionId::new(perm), email, name)
        })
    }
}

#[derive(Clone, Default)]
struct MockAccountStore {
    accounts: Arc<Mutex<HashMap<String, ConnectedAccount>>>,
}

impl AccountStorePort for MockAccountStore {
    async fn find_by_permission_id(
        &self,
        perm_id: &GooglePermissionId,
    ) -> Result<Option<ConnectedAccount>, AccountStoreError> {
        let guard = self.accounts.lock().unwrap();
        Ok(guard.get(perm_id.as_str()).cloned())
    }

    async fn connect(
        &self,
        account: &ConnectedAccount,
    ) -> Result<ConnectedAccount, AccountStoreError> {
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

    async fn remove(&self, id: AccountId) -> Result<(), AccountStoreError> {
        let mut guard = self.accounts.lock().unwrap();
        guard.retain(|_, acc| acc.id() != id);
        Ok(())
    }
}

#[derive(Clone, Default)]
struct MockKeyring {
    tokens: Arc<Mutex<HashMap<u128, String>>>,
    fail_save: bool,
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
    let drive_client = MockDriveClient {
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
        drive_client,
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
    let drive_client = MockDriveClient {
        response: Ok((
            "perm-existing".to_owned(),
            "new@gmail.com".to_owned(),
            "New Name".to_owned(),
        )),
    };

    let service = ConnectAccountService::new(
        token_client,
        drive_client,
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
    let drive_client = MockDriveClient {
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
        drive_client,
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
    let drive_client = MockDriveClient {
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
        drive_client,
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
    let keyring = MockKeyring {
        fail_save: true,
        ..Default::default()
    };

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
        .unwrap_or(()); // save succeeds during seed (fail_save only blocks ConnectAccountService)

    // Re-create keyring with old token pre-loaded but fail_save enabled
    let keyring = MockKeyring {
        tokens: Arc::new(Mutex::new(HashMap::from([(
            1_u128,
            "old-refresh".to_owned(),
        )]))),
        fail_save: true,
    };

    let token_client = MockTokenClient {
        response: Ok(("access-2".to_owned(), Some("new-refresh".to_owned()))),
    };
    let drive_client = MockDriveClient {
        response: Ok((
            "perm-existing".to_owned(),
            "new@gmail.com".to_owned(),
            "New Name".to_owned(),
        )),
    };

    let service = ConnectAccountService::new(
        token_client,
        drive_client,
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
    let drive_client = MockDriveClient {
        response: Ok((
            "perm-existing".to_owned(),
            "new@gmail.com".to_owned(),
            "New Name".to_owned(),
        )),
    };

    let service = ConnectAccountService::new(
        token_client,
        drive_client,
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
