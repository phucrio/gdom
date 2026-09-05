#[cfg(any(target_os = "windows", test))]
use crate::domain::AccountId;

#[cfg(any(target_os = "windows", test))]
const SERVICE: &str = "gdom.google.oauth.refresh-token";

#[cfg(any(target_os = "windows", test))]
const OAUTH_CLIENT_SERVICE: &str = "gdom.google.oauth.client-secret";

#[cfg(any(target_os = "windows", test))]
const OAUTH_CLIENT_ACCOUNT: &str = "client-secret";

#[cfg(any(target_os = "windows", test))]
fn credential_username(account_id: AccountId) -> String {
    format!("account-{}", account_id.value())
}

#[cfg(any(target_os = "windows", test))]
pub struct WindowsCredentialStore {
    store: std::sync::Arc<keyring_core::CredentialStore>,
}

#[cfg(any(target_os = "windows", test))]
impl WindowsCredentialStore {
    #[cfg(target_os = "windows")]
    pub fn new() -> Result<Self, RefreshTokenStoreError> {
        let store = windows_native_keyring_store::Store::new()
            .map_err(|_| RefreshTokenStoreError::Unavailable)?;
        Ok(Self { store })
    }

    /// Create a `WindowsCredentialStore` backed by an in-memory mock.
    /// Available only in test builds.
    #[cfg(test)]
    pub fn new_mock() -> Self {
        let store: std::sync::Arc<keyring_core::CredentialStore> =
            keyring_core::mock::Store::new().expect("mock credential store initializes");
        Self { store }
    }

    fn entry(&self, account_id: AccountId) -> Result<keyring_core::Entry, RefreshTokenStoreError> {
        self.store
            .build(SERVICE, &credential_username(account_id), None)
            .map_err(|_| RefreshTokenStoreError::Unavailable)
    }
}

#[cfg(any(target_os = "windows", test))]
use crate::application::{RefreshToken, RefreshTokenStore, RefreshTokenStoreError};

#[cfg(any(target_os = "windows", test))]
impl RefreshTokenStore for WindowsCredentialStore {
    fn save(
        &self,
        account_id: AccountId,
        token: RefreshToken,
    ) -> Result<(), RefreshTokenStoreError> {
        self.entry(account_id)?
            .set_password(token.expose_secret())
            .map_err(|_| RefreshTokenStoreError::Unavailable)
    }

    fn load(&self, account_id: AccountId) -> Result<Option<RefreshToken>, RefreshTokenStoreError> {
        match self.entry(account_id)?.get_password() {
            Ok(token) => Ok(Some(RefreshToken::new(token))),
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(_) => Err(RefreshTokenStoreError::Unavailable),
        }
    }

    fn delete(&self, account_id: AccountId) -> Result<(), RefreshTokenStoreError> {
        match self.entry(account_id)?.delete_credential() {
            Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
            Err(_) => Err(RefreshTokenStoreError::Unavailable),
        }
    }

    fn save_oauth_secret(&self, secret: &str) -> Result<(), RefreshTokenStoreError> {
        self.store
            .build(OAUTH_CLIENT_SERVICE, OAUTH_CLIENT_ACCOUNT, None)
            .map_err(|_| RefreshTokenStoreError::Unavailable)?
            .set_password(secret)
            .map_err(|_| RefreshTokenStoreError::Unavailable)
    }

    fn load_oauth_secret(&self) -> Result<Option<String>, RefreshTokenStoreError> {
        match self
            .store
            .build(OAUTH_CLIENT_SERVICE, OAUTH_CLIENT_ACCOUNT, None)
            .map_err(|_| RefreshTokenStoreError::Unavailable)?
            .get_password()
        {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(_) => Err(RefreshTokenStoreError::Unavailable),
        }
    }

    fn delete_oauth_secret(&self) -> Result<(), RefreshTokenStoreError> {
        match self
            .store
            .build(OAUTH_CLIENT_SERVICE, OAUTH_CLIENT_ACCOUNT, None)
            .map_err(|_| RefreshTokenStoreError::Unavailable)?
            .delete_credential()
        {
            Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
            Err(_) => Err(RefreshTokenStoreError::Unavailable),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::application::{RefreshToken, RefreshTokenStore};
    use crate::domain::AccountId;

    use super::{WindowsCredentialStore, credential_username};

    fn store() -> WindowsCredentialStore {
        let store: Arc<keyring_core::CredentialStore> =
            keyring_core::mock::Store::new().expect("mock credential store initializes");
        WindowsCredentialStore { store }
    }

    #[test]
    fn credential_username_uses_stable_account_identity() {
        // Given
        let account_id = AccountId::new(42);

        // When
        let username = credential_username(account_id);

        // Then
        assert_eq!(username, "account-42");
    }

    #[test]
    fn store_keeps_multiple_accounts_isolated() {
        // Given
        let store = store();
        for (id, secret) in [(1, "secret-a"), (2, "secret-b"), (3, "secret-c")] {
            store
                .save(AccountId::new(id), RefreshToken::new(secret.to_owned()))
                .expect("token saves");
        }

        // When
        let token = store
            .load(AccountId::new(2))
            .expect("token loads")
            .expect("account has a token");

        // Then
        assert_eq!(token.expose_secret(), "secret-b");
    }

    #[test]
    fn save_replaces_only_the_selected_account() {
        // Given
        let store = store();
        store
            .save(AccountId::new(1), RefreshToken::new("old-a".to_owned()))
            .expect("first token saves");
        store
            .save(AccountId::new(2), RefreshToken::new("secret-b".to_owned()))
            .expect("second token saves");

        // When
        store
            .save(AccountId::new(1), RefreshToken::new("new-a".to_owned()))
            .expect("replacement token saves");

        // Then
        assert_eq!(
            store
                .load(AccountId::new(1))
                .expect("first token loads")
                .expect("first account has a token")
                .expose_secret(),
            "new-a"
        );
        assert_eq!(
            store
                .load(AccountId::new(2))
                .expect("second token loads")
                .expect("second account has a token")
                .expose_secret(),
            "secret-b"
        );
    }

    #[test]
    fn delete_removes_only_the_selected_account() {
        // Given
        let store = store();
        store
            .save(AccountId::new(1), RefreshToken::new("secret-a".to_owned()))
            .expect("first token saves");
        store
            .save(AccountId::new(2), RefreshToken::new("secret-b".to_owned()))
            .expect("second token saves");

        // When
        store.delete(AccountId::new(1)).expect("token deletes");

        // Then
        assert!(
            store
                .load(AccountId::new(1))
                .expect("deleted token lookup succeeds")
                .is_none()
        );
        assert_eq!(
            store
                .load(AccountId::new(2))
                .expect("remaining token loads")
                .expect("second account has a token")
                .expose_secret(),
            "secret-b"
        );
    }

    #[test]
    fn oauth_secret_lifecycle_persists_and_deletes() {
        let store = store();
        assert_eq!(store.load_oauth_secret().expect("load succeeds"), None);

        store
            .save_oauth_secret("my-client-secret")
            .expect("save succeeds");
        assert_eq!(
            store.load_oauth_secret().expect("load succeeds"),
            Some("my-client-secret".to_owned())
        );

        store.delete_oauth_secret().expect("delete succeeds");
        assert_eq!(store.load_oauth_secret().expect("load succeeds"), None);
    }
}
