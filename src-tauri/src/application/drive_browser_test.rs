use super::*;
use crate::{
    application::{
        RefreshFuture, RefreshToken, RefreshTokenStore, RefreshTokenStoreError, TokenRefreshPort,
    },
    domain::{AccountProfile, GooglePermissionId},
    infrastructure::account_store::SqliteAccountStore,
};
use std::{
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    time::Duration,
};

struct TestCredentials;
impl RefreshTokenStore for TestCredentials {
    fn save(&self, _: AccountId, _: RefreshToken) -> Result<(), RefreshTokenStoreError> {
        Ok(())
    }
    fn load(&self, account_id: AccountId) -> Result<Option<RefreshToken>, RefreshTokenStoreError> {
        Ok(Some(RefreshToken::new(format!(
            "account-{}",
            account_id.value()
        ))))
    }
    fn delete(&self, _: AccountId) -> Result<(), RefreshTokenStoreError> {
        Ok(())
    }
}
struct TestRefresh;
impl TokenRefreshPort for TestRefresh {
    fn refresh_token(&self, refresh_token: &RefreshToken) -> RefreshFuture<'_> {
        let token = AccessToken::new(refresh_token.expose_secret().to_owned());
        Box::pin(async move { Ok((token, Duration::from_secs(3600))) })
    }
}
#[derive(Default)]
struct RecordingBrowser {
    total_calls: AtomicUsize,
    account_one_calls: AtomicUsize,
    account_two_calls: AtomicUsize,
    other_token_calls: AtomicUsize,
    unauthorized_next_list: AtomicBool,
}
impl RecordingBrowser {
    fn record(&self, token: &AccessToken) {
        self.total_calls.fetch_add(1, Ordering::SeqCst);
        let calls = match token.expose_secret() {
            "account-1" => &self.account_one_calls,
            "account-2" => &self.account_two_calls,
            _ => &self.other_token_calls,
        };
        calls.fetch_add(1, Ordering::SeqCst);
    }
    fn reject_next_list_as_unauthorized(&self) {
        self.unauthorized_next_list.store(true, Ordering::SeqCst);
    }
}

impl DriveBrowserPort for RecordingBrowser {
    fn storage_quota<'a>(
        &'a self,
        token: &'a AccessToken,
    ) -> BrowserFuture<'a, crate::application::StorageQuota> {
        self.record(token);
        Box::pin(async {
            Ok(crate::application::StorageQuota {
                usage_bytes: 120,
                limit_bytes: Some(5000),
            })
        })
    }
    fn list_files<'a>(
        &'a self,
        token: &'a AccessToken,
        _: &'a BrowseFolderRequest,
    ) -> BrowserFuture<'a, DriveChildPage> {
        self.record(token);
        let unauthorized = self.unauthorized_next_list.swap(false, Ordering::SeqCst);
        Box::pin(async move {
            if unauthorized {
                return Err(DriveFolderLookupError::Unauthorized);
            }
            Ok(DriveChildPage {
                files: vec![DriveChild {
                    id: "file".into(),
                    name: "File".into(),
                    mime_type: "text/plain".into(),
                    parents: vec![],
                    owners: vec![crate::application::DriveFolderOwner {
                        permission_id: GooglePermissionId::new("owner-1"),
                        email_address: None,
                        avatar_url: None,
                    }],
                    drive_id: None,
                    quota_bytes_used: None,
                    trashed: false,
                    shortcut_target_id: None,
                    shortcut_target_mime_type: None,
                    shortcut_target_resource_key: None,
                    resource_key: None,
                    modified_time: None,
                    web_view_link: None,
                }],
                next_page_token: None,
            })
        })
    }
    fn rename_file<'a>(
        &'a self,
        token: &'a AccessToken,
        _: &'a RenameFileRequest,
    ) -> BrowserFuture<'a, ()> {
        self.record(token);
        Box::pin(async { Ok(()) })
    }
    fn trash_file<'a>(&'a self, token: &'a AccessToken, _: &'a str) -> BrowserFuture<'a, ()> {
        self.record(token);
        Box::pin(async { Ok(()) })
    }
}
async fn setup() -> (
    DriveBrowserService<SqliteAccountStore>,
    Arc<SqliteAccountStore>,
    Arc<RecordingBrowser>,
    Arc<AccountTokenProvider<SqliteAccountStore>>,
) {
    let store = Arc::new(SqliteAccountStore::open_in_memory().await.unwrap());
    for number in [1, 2] {
        store
            .connect(&ConnectedAccount::new(
                AccountId::new(number),
                GooglePermissionId::new(format!("owner-{number}")),
                AccountProfile::new(format!("user{number}@gmail.com"), "User", None),
            ))
            .await
            .unwrap();
    }
    let tokens = Arc::new(AccountTokenProvider::new(
        Arc::new(TestRefresh),
        Arc::new(TestCredentials),
        store.clone(),
    ));
    let drive = Arc::new(RecordingBrowser::default());
    (
        DriveBrowserService::new(store.clone(), tokens.clone(), drive.clone()),
        store,
        drive,
        tokens,
    )
}
#[tokio::test]
async fn browser_routes_every_operation_to_selected_account() {
    let (service, _, drive, _) = setup().await;
    for number in [1, 2] {
        let account_id = AccountId::new(number);
        let page = service
            .list_files(account_id, BrowseFolderRequest::default())
            .await
            .unwrap();
        assert_eq!(page.items[0].is_owner, number == 1);
        service
            .rename_file(
                account_id,
                RenameFileRequest {
                    file_id: "file".into(),
                    new_name: "Renamed".into(),
                },
            )
            .await
            .unwrap();
        service.trash_file(account_id, "file").await.unwrap();
        assert_eq!(
            service.storage_quota(account_id).await.unwrap().usage_bytes,
            120
        );
    }
    assert_eq!(drive.account_one_calls.load(Ordering::SeqCst), 4);
    assert_eq!(drive.account_two_calls.load(Ordering::SeqCst), 4);
    assert_eq!(drive.other_token_calls.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn unauthorized_listing_marks_account_for_reauthentication_and_invalidates_cached_token() {
    let (service, store, drive, tokens) = setup().await;
    let account_id = AccountId::new(1);
    let cached_token = tokens.get_access_token(account_id).await.unwrap();
    assert_eq!(cached_token.expose_secret(), "account-1");
    drive.reject_next_list_as_unauthorized();

    let error = service
        .list_files(account_id, BrowseFolderRequest::default())
        .await
        .err()
        .expect("an unauthorized Drive response must fail listing");
    assert!(matches!(
        error,
        DriveBrowserError::Drive(DriveFolderLookupError::Unauthorized)
    ));

    let account = store.find_by_id(account_id).await.unwrap().unwrap();
    assert_eq!(account.auth_status(), AuthStatus::ReauthRequired);
    assert!(matches!(
        tokens.get_access_token(account_id).await,
        Err(TokenProviderError::ReauthRequired)
    ));
    assert_eq!(drive.account_one_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn browser_rejects_missing_and_disconnected_accounts_even_with_cached_token() {
    let (service, store, drive, _) = setup().await;
    service
        .list_files(AccountId::new(1), BrowseFolderRequest::default())
        .await
        .unwrap();
    store
        .update_auth_status(AccountId::new(1), AuthStatus::Disconnected)
        .await
        .unwrap();
    for account_id in [AccountId::new(1), AccountId::new(99)] {
        assert!(
            service
                .list_files(account_id, BrowseFolderRequest::default())
                .await
                .is_err()
        );
        assert!(
            service
                .rename_file(
                    account_id,
                    RenameFileRequest {
                        file_id: "file".into(),
                        new_name: "Renamed".into()
                    }
                )
                .await
                .is_err()
        );
        assert!(service.trash_file(account_id, "file").await.is_err());
        assert!(service.storage_quota(account_id).await.is_err());
    }
    assert_eq!(drive.total_calls.load(Ordering::SeqCst), 1);
}
