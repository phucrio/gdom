use std::{error::Error, fmt};

use crate::{
    application::{AccessToken, RefreshToken, RefreshTokenStore, RefreshTokenStoreError},
    domain::{AccountId, AccountProfile, ConnectedAccount, GooglePermissionId},
    infrastructure::{
        account_store::{AccountStoreError, SqliteAccountStore},
        google_drive::{DriveAccountIdentity, GoogleDriveClient, GoogleDriveError},
        google_oauth::OAuthGrant,
        google_token::{GoogleTokenClient, GoogleTokenError, GoogleTokenResponse},
    },
};

pub struct ConnectAccountService<
    TokenExchange = GoogleTokenClient,
    IdentityLookup = GoogleDriveClient,
    AccountPersistence = SqliteAccountStore,
    CredentialPersistence = Box<dyn RefreshTokenStore + Send + Sync>,
> {
    token_exchange: TokenExchange,
    identity_lookup: IdentityLookup,
    account_persistence: AccountPersistence,
    credential_persistence: CredentialPersistence,
}

#[derive(Debug)]
pub enum ConnectAccountError {
    TokenExchange(GoogleTokenError),
    DriveIdentity(GoogleDriveError),
    Database(AccountStoreError),
    MissingRefreshToken,
    Keychain(RefreshTokenStoreError),
}

impl fmt::Display for ConnectAccountError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TokenExchange(error) => {
                write!(formatter, "failed to exchange OAuth code: {error}")
            }
            Self::DriveIdentity(error) => {
                write!(formatter, "failed to fetch Drive account identity: {error}")
            }
            Self::Database(error) => {
                write!(formatter, "database error while storing account: {error}")
            }
            Self::MissingRefreshToken => formatter.write_str(
                "Google did not provide a refresh token and no existing token was found",
            ),
            Self::Keychain(error) => write!(
                formatter,
                "keychain error while storing refresh token: {error}"
            ),
        }
    }
}

impl Error for ConnectAccountError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::TokenExchange(error) => Some(error),
            Self::DriveIdentity(error) => Some(error),
            Self::Database(error) => Some(error),
            Self::MissingRefreshToken => None,
            Self::Keychain(error) => Some(error),
        }
    }
}

pub trait TokenExchangePort {
    fn exchange_code(
        &self,
        grant: OAuthGrant,
    ) -> impl std::future::Future<Output = Result<GoogleTokenResponse, GoogleTokenError>> + Send;
}

impl TokenExchangePort for GoogleTokenClient {
    async fn exchange_code(
        &self,
        grant: OAuthGrant,
    ) -> Result<GoogleTokenResponse, GoogleTokenError> {
        self.exchange_code(grant).await
    }
}

pub trait DriveIdentityPort {
    fn account_identity(
        &self,
        token: &AccessToken,
    ) -> impl std::future::Future<Output = Result<DriveAccountIdentity, GoogleDriveError>> + Send;
}

impl DriveIdentityPort for GoogleDriveClient {
    async fn account_identity(
        &self,
        token: &AccessToken,
    ) -> Result<DriveAccountIdentity, GoogleDriveError> {
        self.account_identity(token).await
    }
}

pub trait AccountStorePort {
    fn find_by_permission_id(
        &self,
        permission_id: &GooglePermissionId,
    ) -> impl std::future::Future<Output = Result<Option<ConnectedAccount>, AccountStoreError>> + Send;

    fn connect(
        &self,
        account: &ConnectedAccount,
    ) -> impl std::future::Future<Output = Result<ConnectedAccount, AccountStoreError>> + Send;

    fn remove(
        &self,
        account_id: AccountId,
    ) -> impl std::future::Future<Output = Result<(), AccountStoreError>> + Send;
}

impl AccountStorePort for SqliteAccountStore {
    async fn find_by_permission_id(
        &self,
        permission_id: &GooglePermissionId,
    ) -> Result<Option<ConnectedAccount>, AccountStoreError> {
        self.find_by_permission_id(permission_id).await
    }

    async fn connect(
        &self,
        account: &ConnectedAccount,
    ) -> Result<ConnectedAccount, AccountStoreError> {
        self.connect(account).await
    }

    async fn remove(&self, account_id: AccountId) -> Result<(), AccountStoreError> {
        self.remove(account_id).await
    }
}

impl<TokenExchange, IdentityLookup, AccountPersistence, CredentialPersistence>
    ConnectAccountService<TokenExchange, IdentityLookup, AccountPersistence, CredentialPersistence>
where
    TokenExchange: TokenExchangePort + Send + Sync,
    IdentityLookup: DriveIdentityPort + Send + Sync,
    AccountPersistence: AccountStorePort + Send + Sync,
    CredentialPersistence: RefreshTokenStore + Send + Sync,
{
    pub fn new(
        token_exchange: TokenExchange,
        identity_lookup: IdentityLookup,
        account_persistence: AccountPersistence,
        credential_persistence: CredentialPersistence,
    ) -> Self {
        Self {
            token_exchange,
            identity_lookup,
            account_persistence,
            credential_persistence,
        }
    }

    pub async fn connect_account(
        &self,
        oauth_grant: OAuthGrant,
        fallback_account_id: AccountId,
    ) -> Result<ConnectedAccount, ConnectAccountError> {
        let token_response = self
            .token_exchange
            .exchange_code(oauth_grant)
            .await
            .map_err(ConnectAccountError::TokenExchange)?;

        let identity = self
            .identity_lookup
            .account_identity(&token_response.access_token)
            .await
            .map_err(ConnectAccountError::DriveIdentity)?;

        let prior_account = self
            .account_persistence
            .find_by_permission_id(identity.permission_id())
            .await
            .map_err(ConnectAccountError::Database)?;

        let target_account_id = prior_account
            .as_ref()
            .map(ConnectedAccount::id)
            .unwrap_or(fallback_account_id);

        let candidate_account = ConnectedAccount::new(
            target_account_id,
            GooglePermissionId::new(identity.permission_id().as_str()),
            AccountProfile::new(identity.email(), identity.display_name()),
        );

        let persisted_account = self
            .account_persistence
            .connect(&candidate_account)
            .await
            .map_err(ConnectAccountError::Database)?;

        if let Err(error) = self
            .persist_credential(persisted_account.id(), token_response.refresh_token)
            .await
        {
            self.rollback_account(persisted_account.id(), prior_account)
                .await;
            return Err(error);
        }

        Ok(persisted_account)
    }

    async fn persist_credential(
        &self,
        account_id: AccountId,
        refresh_token: Option<RefreshToken>,
    ) -> Result<(), ConnectAccountError> {
        if let Some(token) = refresh_token {
            self.credential_persistence
                .save(account_id, token)
                .map_err(ConnectAccountError::Keychain)?;
        } else {
            let has_stored_token = self
                .credential_persistence
                .load(account_id)
                .map_err(ConnectAccountError::Keychain)?
                .is_some();

            if !has_stored_token {
                return Err(ConnectAccountError::MissingRefreshToken);
            }
        }

        Ok(())
    }

    async fn rollback_account(
        &self,
        account_id: AccountId,
        prior_account: Option<ConnectedAccount>,
    ) {
        match prior_account {
            None => {
                let _ = self.account_persistence.remove(account_id).await;
            }
            Some(previous) => {
                let _ = self.account_persistence.connect(&previous).await;
            }
        }
    }
}
