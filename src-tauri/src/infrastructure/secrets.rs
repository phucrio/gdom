use crate::domain::AccountId;

const SERVICE: &str = "gdom.google.oauth.refresh-token";

const OAUTH_CLIENT_SERVICE: &str = "gdom.google.oauth.client-secret";

const OAUTH_CLIENT_ACCOUNT: &str = "client-secret";

fn credential_username(account_id: AccountId) -> String {
    format!("account-{}", account_id.value())
}

#[derive(Default)]
pub struct NativeCredentialStore {
    #[cfg(test)]
    mock_store: Option<std::sync::Arc<keyring_core::CredentialStore>>,
}

impl NativeCredentialStore {
    pub const fn new() -> Self {
        Self {
            #[cfg(test)]
            mock_store: None,
        }
    }

    fn store(
        &self,
    ) -> Result<std::sync::Arc<keyring_core::CredentialStore>, RefreshTokenStoreError> {
        #[cfg(test)]
        if let Some(store) = &self.mock_store {
            return Ok(std::sync::Arc::clone(store));
        }

        // Reconnect on each operation so an unavailable desktop keychain can recover.
        #[cfg(target_os = "windows")]
        let store = windows_native_keyring_store::Store::new();
        #[cfg(target_os = "macos")]
        let store = apple_native_keyring_store::keychain::Store::new();
        #[cfg(target_os = "linux")]
        let store = dbus_secret_service_keyring_store::Store::new();
        match store {
            Ok(store) => Ok(store),
            Err(_) => Err(RefreshTokenStoreError::Unavailable),
        }
    }

    /// Create a `NativeCredentialStore` backed by an in-memory mock.
    /// Available only in test builds.
    #[cfg(test)]
    pub fn new_mock() -> Self {
        let store: std::sync::Arc<keyring_core::CredentialStore> =
            keyring_core::mock::Store::new().expect("mock credential store initializes");
        Self {
            mock_store: Some(store),
        }
    }

    fn entry(&self, account_id: AccountId) -> Result<keyring_core::Entry, RefreshTokenStoreError> {
        self.store()?
            .build(SERVICE, &credential_username(account_id), None)
            .map_err(|_| RefreshTokenStoreError::Unavailable)
    }
}

use crate::application::{RefreshToken, RefreshTokenStore, RefreshTokenStoreError};

impl RefreshTokenStore for NativeCredentialStore {
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
        self.store()?
            .build(OAUTH_CLIENT_SERVICE, OAUTH_CLIENT_ACCOUNT, None)
            .map_err(|_| RefreshTokenStoreError::Unavailable)?
            .set_password(secret)
            .map_err(|_| RefreshTokenStoreError::Unavailable)
    }

    fn load_oauth_secret(&self) -> Result<Option<String>, RefreshTokenStoreError> {
        match self
            .store()?
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
            .store()?
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

    use super::{NativeCredentialStore, credential_username};

    fn store() -> NativeCredentialStore {
        let store: Arc<keyring_core::CredentialStore> =
            keyring_core::mock::Store::new().expect("mock credential store initializes");
        NativeCredentialStore {
            mock_store: Some(store),
        }
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

    #[test]
    fn unavailable_keychain_can_retry_without_losing_credential() {
        let store = store();
        let account = AccountId::new(7);
        store
            .save(account, RefreshToken::new("retained-secret".to_owned()))
            .expect("save credential");
        let entry = store.entry(account).expect("entry exists");
        let credential: &keyring_core::mock::Cred =
            entry.as_any().downcast_ref().expect("mock credential");
        credential.set_error(keyring_core::Error::NoDefaultStore);

        assert!(matches!(
            store.load(account),
            Err(crate::application::RefreshTokenStoreError::Unavailable)
        ));
        assert_eq!(
            store
                .load(account)
                .expect("retry succeeds")
                .expect("credential retained")
                .expose_secret(),
            "retained-secret"
        );
    }

    #[test]
    #[ignore = "requires an unlocked native desktop keychain and GDOM_NATIVE_KEYCHAIN_TESTS=1"]
    fn native_keychain_round_trip() {
        assert_eq!(
            std::env::var("GDOM_NATIVE_KEYCHAIN_TESTS").as_deref(),
            Ok("1")
        );
        let unique_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let service = format!("gdom.native-smoke.{}.{unique_id}", std::process::id());
        let native = NativeCredentialStore::new();
        let entry = native
            .store()
            .expect("native store available")
            .build(&service, "synthetic-account", None)
            .expect("synthetic entry");
        let result = (|| -> Result<String, crate::application::RefreshTokenStoreError> {
            entry
                .set_password("synthetic-credential-only")
                .map_err(|_| crate::application::RefreshTokenStoreError::Unavailable)?;
            let reopened = native
                .store()?
                .build(&service, "synthetic-account", None)
                .map_err(|_| crate::application::RefreshTokenStoreError::Unavailable)?;
            reopened
                .get_password()
                .map_err(|_| crate::application::RefreshTokenStoreError::Unavailable)
        })();
        let cleanup = entry.delete_credential();
        assert!(cleanup.is_ok(), "synthetic credential cleanup failed");
        assert_eq!(
            result.expect("credential round trip"),
            "synthetic-credential-only"
        );
        assert!(matches!(
            entry.get_password(),
            Err(keyring_core::Error::NoEntry)
        ));
    }
}
