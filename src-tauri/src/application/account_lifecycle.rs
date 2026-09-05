use std::{error::Error, fmt, sync::Arc};

use crate::{
    application::{
        AccountStorePort, AccountStorePortError, IdentityLookupError, IdentityLookupPort,
        OAuthGrant, RefreshTokenStore, RefreshTokenStoreError, TokenExchangeError,
        TokenExchangePort,
    },
    domain::{
        AccountError, AccountId, AccountLabel, AuthStatus, ConnectedAccount, GooglePermissionId,
    },
};

pub trait AccountLifecycleUseCase: Send + Sync {
    fn list_accounts(
        &self,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<Vec<ConnectedAccount>, AccountLifecycleError>>
                + Send
                + '_,
        >,
    >;

    fn update_account_label(
        &self,
        account_id: AccountId,
        label: Option<String>,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<ConnectedAccount, AccountLifecycleError>>
                + Send
                + '_,
        >,
    >;

    fn disconnect_account(
        &self,
        account_id: AccountId,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<(), AccountLifecycleError>> + Send + '_>,
    >;

    fn reauthenticate_account(
        &self,
        account_id: AccountId,
        oauth_grant: OAuthGrant,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<ConnectedAccount, AccountLifecycleError>>
                + Send
                + '_,
        >,
    >;

    fn remove_account(
        &self,
        account_id: AccountId,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<(), AccountLifecycleError>> + Send + '_>,
    >;

    fn delete_local_account_data(
        &self,
        account_id: AccountId,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<(), AccountLifecycleError>> + Send + '_>,
    >;
}

#[derive(Debug)]
pub enum AccountLifecycleError {
    AccountNotFound,
    IdentityMismatch {
        expected: GooglePermissionId,
        actual: GooglePermissionId,
    },
    ActiveJobsPreventRemoval,
    MissingRefreshToken,
    TokenExchange(TokenExchangeError),
    IdentityLookup(IdentityLookupError),
    Account(AccountError),
    Database(AccountStorePortError),
    Keychain(RefreshTokenStoreError),
}

impl fmt::Display for AccountLifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AccountNotFound => write!(f, "account not found"),
            Self::IdentityMismatch { expected, actual } => write!(
                f,
                "reauthentication account identity mismatch: expected permission ID '{}', got '{}'",
                expected.as_str(),
                actual.as_str()
            ),
            Self::ActiveJobsPreventRemoval => {
                write!(
                    f,
                    "cannot remove account referenced by active migration jobs"
                )
            }
            Self::MissingRefreshToken => write!(
                f,
                "Google did not provide a refresh token and no existing token exists"
            ),
            Self::TokenExchange(err) => write!(f, "token exchange failed: {err}"),
            Self::IdentityLookup(err) => write!(f, "identity lookup failed: {err}"),
            Self::Account(err) => write!(f, "invalid account: {err}"),
            Self::Database(err) => write!(f, "database error: {err}"),
            Self::Keychain(err) => write!(f, "keychain error: {err}"),
        }
    }
}

impl Error for AccountLifecycleError {}

pub struct AccountLifecycleService<
    TokenExchange,
    IdentityLookup,
    AccountPersistence,
    CredentialPersistence,
> {
    token_exchange: TokenExchange,
    identity_lookup: IdentityLookup,
    account_store: AccountPersistence,
    credential_store: CredentialPersistence,
    job_store: Option<Arc<dyn crate::application::job_store::JobStorePort>>,
}

impl<TokenExchange, IdentityLookup, AccountPersistence, CredentialPersistence>
    AccountLifecycleService<
        TokenExchange,
        IdentityLookup,
        AccountPersistence,
        CredentialPersistence,
    >
where
    TokenExchange: TokenExchangePort + Send + Sync,
    IdentityLookup: IdentityLookupPort + Send + Sync,
    AccountPersistence: AccountStorePort + Send + Sync,
    CredentialPersistence: RefreshTokenStore + Send + Sync,
{
    pub fn new(
        token_exchange: TokenExchange,
        identity_lookup: IdentityLookup,
        account_store: AccountPersistence,
        credential_store: CredentialPersistence,
    ) -> Self {
        Self {
            token_exchange,
            identity_lookup,
            account_store,
            credential_store,
            job_store: None,
        }
    }

    pub fn with_job_store(
        mut self,
        job_store: Arc<dyn crate::application::job_store::JobStorePort>,
    ) -> Self {
        self.job_store = Some(job_store);
        self
    }

    pub async fn list_accounts(&self) -> Result<Vec<ConnectedAccount>, AccountLifecycleError> {
        self.account_store
            .list_all()
            .await
            .map_err(AccountLifecycleError::Database)
    }

    pub async fn update_account_label(
        &self,
        account_id: AccountId,
        label: Option<String>,
    ) -> Result<ConnectedAccount, AccountLifecycleError> {
        let _account = self
            .account_store
            .find_by_id(account_id)
            .await
            .map_err(AccountLifecycleError::Database)?
            .ok_or(AccountLifecycleError::AccountNotFound)?;

        let parsed_label = match label {
            Some(ref s) if !s.trim().is_empty() => {
                Some(AccountLabel::new(s).map_err(AccountLifecycleError::Account)?)
            }
            _ => None,
        };

        self.account_store
            .update_label(account_id, parsed_label.as_ref())
            .await
            .map_err(AccountLifecycleError::Database)?;

        self.account_store
            .find_by_id(account_id)
            .await
            .map_err(AccountLifecycleError::Database)?
            .ok_or(AccountLifecycleError::AccountNotFound)
    }

    pub async fn disconnect_account(
        &self,
        account_id: AccountId,
    ) -> Result<(), AccountLifecycleError> {
        let _account = self
            .account_store
            .find_by_id(account_id)
            .await
            .map_err(AccountLifecycleError::Database)?
            .ok_or(AccountLifecycleError::AccountNotFound)?;

        self.credential_store
            .delete(account_id)
            .map_err(AccountLifecycleError::Keychain)?;

        self.account_store
            .update_auth_status(account_id, AuthStatus::Disconnected)
            .await
            .map_err(AccountLifecycleError::Database)?;

        Ok(())
    }

    pub async fn reauthenticate_account(
        &self,
        account_id: AccountId,
        oauth_grant: OAuthGrant,
    ) -> Result<ConnectedAccount, AccountLifecycleError> {
        let existing_account = self
            .account_store
            .find_by_id(account_id)
            .await
            .map_err(AccountLifecycleError::Database)?
            .ok_or(AccountLifecycleError::AccountNotFound)?;

        let token_response = self
            .token_exchange
            .exchange_code(oauth_grant)
            .await
            .map_err(AccountLifecycleError::TokenExchange)?;

        let identity = self
            .identity_lookup
            .account_identity(&token_response.access_token)
            .await
            .map_err(AccountLifecycleError::IdentityLookup)?;

        if identity.permission_id() != existing_account.google_permission_id() {
            return Err(AccountLifecycleError::IdentityMismatch {
                expected: existing_account.google_permission_id().clone(),
                actual: identity.permission_id().clone(),
            });
        }

        if let Some(refresh_token) = token_response.refresh_token {
            self.credential_store
                .save(account_id, refresh_token)
                .map_err(AccountLifecycleError::Keychain)?;
        } else {
            let has_stored = self
                .credential_store
                .load(account_id)
                .map_err(AccountLifecycleError::Keychain)?
                .is_some();
            if !has_stored {
                return Err(AccountLifecycleError::MissingRefreshToken);
            }
        }

        let updated_candidate = ConnectedAccount::new_personal(
            account_id,
            identity.permission_id().clone(),
            identity.email(),
            identity.display_name(),
        )
        .map_err(AccountLifecycleError::Account)?;

        let persisted = self
            .account_store
            .connect(&updated_candidate)
            .await
            .map_err(AccountLifecycleError::Database)?;

        Ok(persisted)
    }

    pub async fn remove_account(&self, account_id: AccountId) -> Result<(), AccountLifecycleError> {
        let _account = self
            .account_store
            .find_by_id(account_id)
            .await
            .map_err(AccountLifecycleError::Database)?
            .ok_or(AccountLifecycleError::AccountNotFound)?;

        let job_store = self
            .job_store
            .as_ref()
            .ok_or(AccountLifecycleError::ActiveJobsPreventRemoval)?;
        let has_active = job_store
            .has_active_jobs_for_account(account_id)
            .await
            .map_err(|e| {
                AccountLifecycleError::Database(AccountStorePortError::Storage(e.to_string()))
            })?;
        if has_active {
            return Err(AccountLifecycleError::ActiveJobsPreventRemoval);
        }

        self.credential_store
            .delete(account_id)
            .map_err(AccountLifecycleError::Keychain)?;

        self.account_store
            .remove(account_id)
            .await
            .map_err(AccountLifecycleError::Database)?;

        Ok(())
    }

    pub async fn delete_local_account_data(
        &self,
        account_id: AccountId,
    ) -> Result<(), AccountLifecycleError> {
        let job_store = self
            .job_store
            .as_ref()
            .ok_or(AccountLifecycleError::ActiveJobsPreventRemoval)?;
        let has_jobs = job_store
            .has_jobs_for_account(account_id)
            .await
            .map_err(|e| {
                AccountLifecycleError::Database(AccountStorePortError::Storage(e.to_string()))
            })?;
        if has_jobs {
            return Err(AccountLifecycleError::ActiveJobsPreventRemoval);
        }

        self.credential_store
            .delete(account_id)
            .map_err(AccountLifecycleError::Keychain)?;

        self.account_store
            .hard_delete(account_id)
            .await
            .map_err(AccountLifecycleError::Database)?;

        Ok(())
    }
}

impl<TokenExchange, IdentityLookup, AccountPersistence, CredentialPersistence>
    AccountLifecycleUseCase
    for AccountLifecycleService<
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
    fn list_accounts(
        &self,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<Vec<ConnectedAccount>, AccountLifecycleError>>
                + Send
                + '_,
        >,
    > {
        Box::pin(self.list_accounts())
    }

    fn update_account_label(
        &self,
        account_id: AccountId,
        label: Option<String>,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<ConnectedAccount, AccountLifecycleError>>
                + Send
                + '_,
        >,
    > {
        Box::pin(self.update_account_label(account_id, label))
    }

    fn disconnect_account(
        &self,
        account_id: AccountId,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<(), AccountLifecycleError>> + Send + '_>,
    > {
        Box::pin(self.disconnect_account(account_id))
    }

    fn reauthenticate_account(
        &self,
        account_id: AccountId,
        oauth_grant: OAuthGrant,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<ConnectedAccount, AccountLifecycleError>>
                + Send
                + '_,
        >,
    > {
        Box::pin(self.reauthenticate_account(account_id, oauth_grant))
    }

    fn remove_account(
        &self,
        account_id: AccountId,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<(), AccountLifecycleError>> + Send + '_>,
    > {
        Box::pin(self.remove_account(account_id))
    }

    fn delete_local_account_data(
        &self,
        account_id: AccountId,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<(), AccountLifecycleError>> + Send + '_>,
    > {
        Box::pin(self.delete_local_account_data(account_id))
    }
}

impl<T: AccountLifecycleUseCase + ?Sized> AccountLifecycleUseCase for Arc<T> {
    fn list_accounts(
        &self,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<Vec<ConnectedAccount>, AccountLifecycleError>>
                + Send
                + '_,
        >,
    > {
        (**self).list_accounts()
    }

    fn update_account_label(
        &self,
        account_id: AccountId,
        label: Option<String>,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<ConnectedAccount, AccountLifecycleError>>
                + Send
                + '_,
        >,
    > {
        (**self).update_account_label(account_id, label)
    }

    fn disconnect_account(
        &self,
        account_id: AccountId,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<(), AccountLifecycleError>> + Send + '_>,
    > {
        (**self).disconnect_account(account_id)
    }

    fn reauthenticate_account(
        &self,
        account_id: AccountId,
        oauth_grant: OAuthGrant,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<ConnectedAccount, AccountLifecycleError>>
                + Send
                + '_,
        >,
    > {
        (**self).reauthenticate_account(account_id, oauth_grant)
    }

    fn remove_account(
        &self,
        account_id: AccountId,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<(), AccountLifecycleError>> + Send + '_>,
    > {
        (**self).remove_account(account_id)
    }

    fn delete_local_account_data(
        &self,
        account_id: AccountId,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<(), AccountLifecycleError>> + Send + '_>,
    > {
        (**self).delete_local_account_data(account_id)
    }
}
