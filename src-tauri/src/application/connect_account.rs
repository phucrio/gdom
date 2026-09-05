use std::{error::Error, fmt, sync::Arc};

use crate::{
    application::{AccessToken, RefreshToken, RefreshTokenStore, RefreshTokenStoreError},
    domain::{
        AccountError, AccountId, AccountLabel, AuthStatus, ConnectedAccount, GooglePermissionId,
    },
};

pub struct OAuthGrant {
    authorization_code: String,
    pkce_verifier: String,
    redirect_uri: String,
}

impl OAuthGrant {
    pub fn new(authorization_code: String, pkce_verifier: String, redirect_uri: String) -> Self {
        Self {
            authorization_code,
            pkce_verifier,
            redirect_uri,
        }
    }

    pub fn authorization_code(&self) -> &str {
        &self.authorization_code
    }

    pub fn pkce_verifier(&self) -> &str {
        &self.pkce_verifier
    }

    pub fn redirect_uri(&self) -> &str {
        &self.redirect_uri
    }
}

impl fmt::Debug for OAuthGrant {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("OAuthGrant([REDACTED])")
    }
}

#[derive(Debug)]
pub struct TokenResponse {
    pub access_token: AccessToken,
    pub refresh_token: Option<RefreshToken>,
}

impl TokenResponse {
    pub const fn new(access_token: AccessToken, refresh_token: Option<RefreshToken>) -> Self {
        Self {
            access_token,
            refresh_token,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenExchangeError {
    InvalidGrant,
    InvalidClient,
    RateLimited,
    Unavailable,
    Transport,
    InvalidResponse,
    UnexpectedStatus(u16),
}

impl fmt::Display for TokenExchangeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidGrant => formatter.write_str("token exchange rejected grant or verifier"),
            Self::InvalidClient => {
                formatter.write_str("token exchange rejected client credentials")
            }
            Self::RateLimited => formatter.write_str("token exchange rate limit reached"),
            Self::Unavailable => formatter.write_str("token service is unavailable"),
            Self::Transport => formatter.write_str("token exchange network request failed"),
            Self::InvalidResponse => {
                formatter.write_str("token service returned an invalid response")
            }
            Self::UnexpectedStatus(status) => {
                write!(
                    formatter,
                    "token service returned unexpected status {status}"
                )
            }
        }
    }
}

impl Error for TokenExchangeError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountIdentity {
    permission_id: GooglePermissionId,
    email: String,
    display_name: String,
}

impl AccountIdentity {
    pub fn new(
        permission_id: GooglePermissionId,
        email: impl Into<String>,
        display_name: impl Into<String>,
    ) -> Self {
        Self {
            permission_id,
            email: email.into(),
            display_name: display_name.into(),
        }
    }

    pub const fn permission_id(&self) -> &GooglePermissionId {
        &self.permission_id
    }

    pub fn email(&self) -> &str {
        &self.email
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentityLookupError {
    Unauthorized,
    Forbidden,
    RateLimited,
    Unavailable,
    Transport,
    InvalidResponse,
    UnexpectedStatus(u16),
}

impl fmt::Display for IdentityLookupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unauthorized => formatter.write_str("identity service rejected access token"),
            Self::Forbidden => formatter.write_str("identity service denied access"),
            Self::RateLimited => formatter.write_str("identity service rate limit reached"),
            Self::Unavailable => formatter.write_str("identity service is unavailable"),
            Self::Transport => formatter.write_str("identity service request failed"),
            Self::InvalidResponse => {
                formatter.write_str("identity service returned an invalid response")
            }
            Self::UnexpectedStatus(status) => {
                write!(
                    formatter,
                    "identity service returned unexpected status {status}"
                )
            }
        }
    }
}

impl Error for IdentityLookupError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AccountStorePortError {
    Storage(String),
}

impl fmt::Display for AccountStorePortError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(message) => write!(formatter, "{message}"),
        }
    }
}

impl Error for AccountStorePortError {}

pub trait TokenExchangePort {
    fn exchange_code(
        &self,
        grant: OAuthGrant,
    ) -> impl std::future::Future<Output = Result<TokenResponse, TokenExchangeError>> + Send;
}

pub trait IdentityLookupPort {
    fn account_identity(
        &self,
        token: &AccessToken,
    ) -> impl std::future::Future<Output = Result<AccountIdentity, IdentityLookupError>> + Send;
}

pub trait AccountStorePort {
    fn find_by_permission_id(
        &self,
        permission_id: &GooglePermissionId,
    ) -> impl std::future::Future<Output = Result<Option<ConnectedAccount>, AccountStorePortError>> + Send;

    fn find_by_id(
        &self,
        account_id: AccountId,
    ) -> impl std::future::Future<Output = Result<Option<ConnectedAccount>, AccountStorePortError>> + Send;

    fn connect(
        &self,
        account: &ConnectedAccount,
    ) -> impl std::future::Future<Output = Result<ConnectedAccount, AccountStorePortError>> + Send;

    fn update_auth_status(
        &self,
        account_id: AccountId,
        status: AuthStatus,
    ) -> impl std::future::Future<Output = Result<(), AccountStorePortError>> + Send;

    fn update_label(
        &self,
        account_id: AccountId,
        label: Option<&AccountLabel>,
    ) -> impl std::future::Future<Output = Result<(), AccountStorePortError>> + Send;

    fn mark_last_authenticated(
        &self,
        account_id: AccountId,
    ) -> impl std::future::Future<Output = Result<(), AccountStorePortError>> + Send;

    fn remove(
        &self,
        account_id: AccountId,
    ) -> impl std::future::Future<Output = Result<(), AccountStorePortError>> + Send;

    fn hard_delete(
        &self,
        account_id: AccountId,
    ) -> impl std::future::Future<Output = Result<(), AccountStorePortError>> + Send;

    fn list_all(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<ConnectedAccount>, AccountStorePortError>> + Send;
}

pub trait ConnectAccountUseCase: Send + Sync {
    fn connect_account(
        &self,
        oauth_grant: OAuthGrant,
        fallback_account_id: AccountId,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<ConnectedAccount, ConnectAccountError>>
                + Send
                + '_,
        >,
    >;
}

impl<TokenExchange, IdentityLookup, AccountPersistence, CredentialPersistence> ConnectAccountUseCase
    for ConnectAccountService<
        TokenExchange,
        IdentityLookup,
        AccountPersistence,
        CredentialPersistence,
    >
where
    TokenExchange: TokenExchangePort + Send + Sync + 'static,
    IdentityLookup: IdentityLookupPort + Send + Sync + 'static,
    AccountPersistence: AccountStorePort + Send + Sync + 'static,
    CredentialPersistence: RefreshTokenStore + Send + Sync + 'static,
{
    fn connect_account(
        &self,
        oauth_grant: OAuthGrant,
        fallback_account_id: AccountId,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<ConnectedAccount, ConnectAccountError>>
                + Send
                + '_,
        >,
    > {
        Box::pin(self.connect_account(oauth_grant, fallback_account_id))
    }
}

impl<T: TokenExchangePort + Send + Sync + ?Sized> TokenExchangePort for Arc<T> {
    fn exchange_code(
        &self,
        grant: OAuthGrant,
    ) -> impl std::future::Future<Output = Result<TokenResponse, TokenExchangeError>> + Send {
        (**self).exchange_code(grant)
    }
}

impl<T: IdentityLookupPort + Send + Sync + ?Sized> IdentityLookupPort for Arc<T> {
    fn account_identity(
        &self,
        token: &AccessToken,
    ) -> impl std::future::Future<Output = Result<AccountIdentity, IdentityLookupError>> + Send
    {
        (**self).account_identity(token)
    }
}

impl<T: ConnectAccountUseCase + ?Sized> ConnectAccountUseCase for Arc<T> {
    fn connect_account(
        &self,
        oauth_grant: OAuthGrant,
        fallback_account_id: AccountId,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<ConnectedAccount, ConnectAccountError>>
                + Send
                + '_,
        >,
    > {
        (**self).connect_account(oauth_grant, fallback_account_id)
    }
}

impl<T: AccountStorePort + Send + Sync> AccountStorePort for Arc<T> {
    fn find_by_permission_id(
        &self,
        permission_id: &GooglePermissionId,
    ) -> impl std::future::Future<Output = Result<Option<ConnectedAccount>, AccountStorePortError>> + Send
    {
        (**self).find_by_permission_id(permission_id)
    }

    fn find_by_id(
        &self,
        account_id: AccountId,
    ) -> impl std::future::Future<Output = Result<Option<ConnectedAccount>, AccountStorePortError>> + Send
    {
        (**self).find_by_id(account_id)
    }

    fn connect(
        &self,
        account: &ConnectedAccount,
    ) -> impl std::future::Future<Output = Result<ConnectedAccount, AccountStorePortError>> + Send
    {
        (**self).connect(account)
    }

    fn update_auth_status(
        &self,
        account_id: AccountId,
        status: AuthStatus,
    ) -> impl std::future::Future<Output = Result<(), AccountStorePortError>> + Send {
        (**self).update_auth_status(account_id, status)
    }

    fn update_label(
        &self,
        account_id: AccountId,
        label: Option<&AccountLabel>,
    ) -> impl std::future::Future<Output = Result<(), AccountStorePortError>> + Send {
        (**self).update_label(account_id, label)
    }

    fn mark_last_authenticated(
        &self,
        account_id: AccountId,
    ) -> impl std::future::Future<Output = Result<(), AccountStorePortError>> + Send {
        (**self).mark_last_authenticated(account_id)
    }

    fn remove(
        &self,
        account_id: AccountId,
    ) -> impl std::future::Future<Output = Result<(), AccountStorePortError>> + Send {
        (**self).remove(account_id)
    }

    fn hard_delete(
        &self,
        account_id: AccountId,
    ) -> impl std::future::Future<Output = Result<(), AccountStorePortError>> + Send {
        (**self).hard_delete(account_id)
    }

    fn list_all(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<ConnectedAccount>, AccountStorePortError>> + Send
    {
        (**self).list_all()
    }
}

#[derive(Clone)]
pub struct ConnectAccountService<
    TokenExchange,
    IdentityLookup,
    AccountPersistence,
    CredentialPersistence,
> {
    token_exchange: TokenExchange,
    identity_lookup: IdentityLookup,
    account_persistence: AccountPersistence,
    credential_persistence: CredentialPersistence,
    lock: Arc<tokio::sync::Mutex<()>>,
}

#[derive(Debug)]
pub enum ConnectAccountError {
    TokenExchange(TokenExchangeError),
    IdentityLookup(IdentityLookupError),
    Account(AccountError),
    Database(AccountStorePortError),
    MissingRefreshToken,
    Keychain(RefreshTokenStoreError),
    RollbackFailed {
        primary_error: Box<ConnectAccountError>,
        rollback_error: Box<AccountStorePortError>,
    },
}

impl fmt::Display for ConnectAccountError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TokenExchange(error) => {
                write!(formatter, "failed to exchange OAuth code: {error}")
            }
            Self::IdentityLookup(error) => {
                write!(formatter, "failed to fetch account identity: {error}")
            }
            Self::Account(error) => write!(formatter, "account validation failed: {error}"),
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
            Self::RollbackFailed {
                primary_error,
                rollback_error,
            } => write!(
                formatter,
                "account connection failed ({primary_error}) and rollback also failed: {rollback_error}"
            ),
        }
    }
}

impl Error for ConnectAccountError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::TokenExchange(error) => Some(error),
            Self::IdentityLookup(error) => Some(error),
            Self::Account(error) => Some(error),
            Self::Database(error) => Some(error),
            Self::MissingRefreshToken => None,
            Self::Keychain(error) => Some(error),
            Self::RollbackFailed { primary_error, .. } => Some(primary_error),
        }
    }
}

impl<TokenExchange, IdentityLookup, AccountPersistence, CredentialPersistence>
    ConnectAccountService<TokenExchange, IdentityLookup, AccountPersistence, CredentialPersistence>
where
    TokenExchange: TokenExchangePort + Send + Sync,
    IdentityLookup: IdentityLookupPort + Send + Sync,
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
            lock: Arc::new(tokio::sync::Mutex::new(())),
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
            .map_err(ConnectAccountError::IdentityLookup)?;

        let _guard = self.lock.lock().await;

        let prior_account = self
            .account_persistence
            .find_by_permission_id(identity.permission_id())
            .await
            .map_err(ConnectAccountError::Database)?;

        let target_account_id = prior_account
            .as_ref()
            .map(ConnectedAccount::id)
            .unwrap_or(fallback_account_id);

        let candidate_account = ConnectedAccount::new_personal(
            target_account_id,
            identity.permission_id().clone(),
            identity.email(),
            identity.display_name(),
        )
        .map_err(ConnectAccountError::Account)?;

        let persisted_account = self
            .account_persistence
            .connect(&candidate_account)
            .await
            .map_err(ConnectAccountError::Database)?;

        if let Err(primary_error) = self
            .persist_credential(persisted_account.id(), token_response.refresh_token)
            .await
        {
            if let Err(rollback_error) = self
                .rollback_account(persisted_account.id(), prior_account)
                .await
            {
                return Err(ConnectAccountError::RollbackFailed {
                    primary_error: Box::new(primary_error),
                    rollback_error: Box::new(rollback_error),
                });
            }
            return Err(primary_error);
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
    ) -> Result<(), AccountStorePortError> {
        match prior_account {
            None => {
                let has_valid_token = self
                    .credential_persistence
                    .load(account_id)
                    .map(|token| token.is_some())
                    .unwrap_or(false);

                if has_valid_token {
                    Ok(())
                } else {
                    self.account_persistence.remove(account_id).await
                }
            }
            Some(previous) => self
                .account_persistence
                .connect(&previous)
                .await
                .map(|_| ()),
        }
    }
}
