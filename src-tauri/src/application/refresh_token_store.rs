use std::{error::Error, fmt, sync::Arc};

use crate::domain::AccountId;

#[derive(Clone)]
pub struct RefreshToken(String);

impl RefreshToken {
    pub const fn new(value: String) -> Self {
        Self(value)
    }

    pub fn expose_secret(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for RefreshToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RefreshToken([REDACTED])")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RefreshTokenStoreError {
    Unavailable,
}

impl fmt::Display for RefreshTokenStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable => formatter.write_str("credential store is unavailable"),
        }
    }
}

impl Error for RefreshTokenStoreError {}

pub trait RefreshTokenStore: Send + Sync {
    fn save(
        &self,
        account_id: AccountId,
        token: RefreshToken,
    ) -> Result<(), RefreshTokenStoreError>;

    fn load(&self, account_id: AccountId) -> Result<Option<RefreshToken>, RefreshTokenStoreError>;

    fn delete(&self, account_id: AccountId) -> Result<(), RefreshTokenStoreError>;

    fn save_oauth_secret(&self, secret: &str) -> Result<(), RefreshTokenStoreError> {
        let _ = secret;
        Ok(())
    }

    fn load_oauth_secret(&self) -> Result<Option<String>, RefreshTokenStoreError> {
        Ok(None)
    }

    fn delete_oauth_secret(&self) -> Result<(), RefreshTokenStoreError> {
        Ok(())
    }
}

impl<T: RefreshTokenStore> RefreshTokenStore for Arc<T> {
    fn save(
        &self,
        account_id: AccountId,
        token: RefreshToken,
    ) -> Result<(), RefreshTokenStoreError> {
        (**self).save(account_id, token)
    }

    fn load(&self, account_id: AccountId) -> Result<Option<RefreshToken>, RefreshTokenStoreError> {
        (**self).load(account_id)
    }

    fn delete(&self, account_id: AccountId) -> Result<(), RefreshTokenStoreError> {
        (**self).delete(account_id)
    }

    fn save_oauth_secret(&self, secret: &str) -> Result<(), RefreshTokenStoreError> {
        (**self).save_oauth_secret(secret)
    }

    fn load_oauth_secret(&self) -> Result<Option<String>, RefreshTokenStoreError> {
        (**self).load_oauth_secret()
    }

    fn delete_oauth_secret(&self) -> Result<(), RefreshTokenStoreError> {
        (**self).delete_oauth_secret()
    }
}

#[cfg(test)]
mod tests {
    use super::{RefreshToken, RefreshTokenStoreError};

    #[test]
    fn token_and_store_errors_redact_secrets() {
        // Given
        let secret = "do-not-print-this-refresh-token";

        // When
        let token_debug = format!("{:?}", RefreshToken::new(secret.to_owned()));
        let error_debug = format!("{:?}", RefreshTokenStoreError::Unavailable);
        let error_display = RefreshTokenStoreError::Unavailable.to_string();

        // Then
        assert!(!token_debug.contains(secret));
        assert!(!error_debug.contains(secret));
        assert!(!error_display.contains(secret));
    }
}
