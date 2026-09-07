use super::{
    AccessToken, AccountStorePort, AccountStorePortError, AccountTokenProvider, DriveChild,
    DriveChildPage, DriveFolderLookupError, TokenProviderError,
};
use crate::domain::{AccountId, AuthStatus, ConnectedAccount};
use std::{future::Future, pin::Pin, sync::Arc};

#[derive(Default)]
pub struct BrowseFolderRequest {
    pub folder_id: Option<String>,
    pub folder_resource_key: Option<String>,
    pub page_token: Option<String>,
    pub page_size: Option<u32>,
    pub order_by: Option<String>,
}
pub type BrowserFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, DriveFolderLookupError>> + Send + 'a>>;
pub trait DriveBrowserPort: Send + Sync {
    fn storage_quota<'a>(
        &'a self,
        token: &'a AccessToken,
    ) -> BrowserFuture<'a, super::StorageQuota>;
    fn list_files<'a>(
        &'a self,
        token: &'a AccessToken,
        request: &'a BrowseFolderRequest,
    ) -> BrowserFuture<'a, DriveChildPage>;
    fn rename_file<'a>(
        &'a self,
        token: &'a AccessToken,
        file: &'a RenameFileRequest,
    ) -> BrowserFuture<'a, ()>;
    fn trash_file<'a>(&'a self, token: &'a AccessToken, file_id: &'a str) -> BrowserFuture<'a, ()>;
}
pub struct RenameFileRequest {
    pub file_id: String,
    pub new_name: String,
}
pub struct BrowserItem {
    pub file: DriveChild,
    pub is_owner: bool,
}
pub struct BrowserPage {
    pub items: Vec<BrowserItem>,
    pub next_page_token: Option<String>,
}
#[derive(Debug)]
pub enum DriveBrowserError {
    AccountNotFound,
    Database(AccountStorePortError),
    Authorization(TokenProviderError),
    Drive(DriveFolderLookupError),
    InvalidInput(&'static str),
}
pub struct DriveBrowserService<AccountPersistence> {
    account_store: Arc<AccountPersistence>,
    token_provider: Arc<AccountTokenProvider<AccountPersistence>>,
    drive: Arc<dyn DriveBrowserPort>,
}
impl<AccountPersistence: AccountStorePort + Send + Sync + 'static>
    DriveBrowserService<AccountPersistence>
{
    pub fn new(
        account_store: Arc<AccountPersistence>,
        token_provider: Arc<AccountTokenProvider<AccountPersistence>>,
        drive: Arc<dyn DriveBrowserPort>,
    ) -> Self {
        Self {
            account_store,
            token_provider,
            drive,
        }
    }
    pub async fn load_connected_account(
        &self,
        account_id: AccountId,
    ) -> Result<ConnectedAccount, DriveBrowserError> {
        let account = self
            .account_store
            .find_by_id(account_id)
            .await
            .map_err(DriveBrowserError::Database)?
            .ok_or(DriveBrowserError::AccountNotFound)?;
        // Check persistence before the token cache, which can still contain a revoked session.
        if !account.is_active() {
            return Err(DriveBrowserError::Authorization(
                TokenProviderError::AccountRemoved,
            ));
        }
        match account.auth_status() {
            AuthStatus::Connected | AuthStatus::TokenRefreshing => {}
            AuthStatus::RemovalPending => {
                return Err(DriveBrowserError::Authorization(
                    TokenProviderError::AccountRemoved,
                ));
            }
            AuthStatus::Disconnected => {
                return Err(DriveBrowserError::Authorization(
                    TokenProviderError::AccountDisconnected,
                ));
            }
            AuthStatus::ReauthRequired => {
                return Err(DriveBrowserError::Authorization(
                    TokenProviderError::ReauthRequired,
                ));
            }
        }
        Ok(account)
    }
    async fn authorize_account(
        &self,
        account_id: AccountId,
    ) -> Result<(ConnectedAccount, AccessToken), DriveBrowserError> {
        let account = self.load_connected_account(account_id).await?;
        let token = self
            .token_provider
            .get_access_token(account_id)
            .await
            .map_err(DriveBrowserError::Authorization)?;
        Ok((account, token))
    }
    pub async fn list_files(
        &self,
        account_id: AccountId,
        request: BrowseFolderRequest,
    ) -> Result<BrowserPage, DriveBrowserError> {
        let (account, token) = self.authorize_account(account_id).await?;
        let page = self
            .drive
            .list_files(&token, &request)
            .await
            .map_err(DriveBrowserError::Drive)?;
        Ok(BrowserPage {
            items: page
                .files
                .into_iter()
                .map(|file| {
                    let is_owner = file
                        .owners
                        .iter()
                        .any(|owner| &owner.permission_id == account.google_permission_id());
                    BrowserItem { file, is_owner }
                })
                .collect(),
            next_page_token: page.next_page_token,
        })
    }
    pub async fn storage_quota(
        &self,
        account_id: AccountId,
    ) -> Result<super::StorageQuota, DriveBrowserError> {
        let (_, token) = self.authorize_account(account_id).await?;
        self.drive
            .storage_quota(&token)
            .await
            .map_err(DriveBrowserError::Drive)
    }
    pub async fn rename_file(
        &self,
        account_id: AccountId,
        request: RenameFileRequest,
    ) -> Result<(), DriveBrowserError> {
        if request.file_id.trim().is_empty() || request.new_name.trim().is_empty() {
            return Err(DriveBrowserError::InvalidInput(
                "file ID and name must not be empty",
            ));
        }
        let (_, token) = self.authorize_account(account_id).await?;
        self.drive
            .rename_file(&token, &request)
            .await
            .map_err(DriveBrowserError::Drive)
    }
    pub async fn trash_file(
        &self,
        account_id: AccountId,
        file_id: &str,
    ) -> Result<(), DriveBrowserError> {
        if file_id.trim().is_empty() {
            return Err(DriveBrowserError::InvalidInput("file ID must not be empty"));
        }
        let (_, token) = self.authorize_account(account_id).await?;
        self.drive
            .trash_file(&token, file_id)
            .await
            .map_err(DriveBrowserError::Drive)
    }
}

#[cfg(test)]
#[path = "drive_browser_test.rs"]
mod tests;
