use super::*;
use crate::{
    application::{
        RefreshFuture, RefreshToken, RefreshTokenStore, RefreshTokenStoreError, TokenRefreshPort,
    },
    domain::{AccountProfile, GooglePermissionId},
    infrastructure::account_store::SqliteAccountStore,
};
use std::{sync::Mutex, time::Duration};

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
struct RecordingBrowser(Mutex<Vec<String>>);
impl RecordingBrowser {
    fn record(&self, token: &AccessToken) {
        self.0
            .lock()
            .unwrap()
            .push(token.expose_secret().to_owned());
    }
}
impl DriveBrowserPort for RecordingBrowser {
    fn list_files<'a>(
        &'a self,
        token: &'a AccessToken,
        _: &'a BrowseFolderRequest,
    ) -> BrowserFuture<'a, DriveChildPage> {
        self.record(token);
        Box::pin(async {
            Ok(DriveChildPage {
                files: vec![DriveChild {
                    id: "file".into(),
                    name: "File".into(),
                    mime_type: "text/plain".into(),
                    parents: vec![],
                    owners: vec![crate::application::DriveFolderOwner {
                        permission_id: GooglePermissionId::new("owner-1"),
                        email_address: None,
                    }],
                    drive_id: None,
                    quota_bytes_used: None,
                    trashed: false,
                    shortcut_target_id: None,
                    shortcut_target_mime_type: None,
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
        DriveBrowserService::new(store.clone(), tokens, drive.clone()),
        store,
        drive,
    )
}
#[tokio::test]
async fn browser_routes_every_operation_to_selected_account() {
    let (service, _, drive) = setup().await;
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
    }
    assert_eq!(
        *drive.0.lock().unwrap(),
        [
            "account-1",
            "account-1",
            "account-1",
            "account-2",
            "account-2",
            "account-2"
        ]
    );
}
#[tokio::test]
async fn browser_rejects_missing_and_disconnected_accounts_even_with_cached_token() {
    let (service, store, drive) = setup().await;
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
    }
    assert_eq!(drive.0.lock().unwrap().len(), 1);
}
