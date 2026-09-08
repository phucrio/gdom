#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use tokio::sync::RwLock;

    use crate::application::account_token_provider::AccountTokenProvider;
    use crate::application::connect_account::ConnectAccountUseCase;
    use crate::application::{AccessToken, AccountLifecycleUseCase, JobService};
    struct DummyConnectAccountUseCase;

    impl ConnectAccountUseCase for DummyConnectAccountUseCase {
        fn connect_account(
            &self,
            _grant: crate::application::connect_account::OAuthGrant,
            _fallback_id: AccountId,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<
                        Output = Result<
                            ConnectedAccount,
                            crate::application::connect_account::ConnectAccountError,
                        >,
                    > + Send
                    + '_,
            >,
        > {
            Box::pin(async { unimplemented!() })
        }
    }
    use crate::commands::drive::{
        list_drive_files_inner, rename_drive_item_inner, start_transfer_operation_inner,
        trash_drive_item_inner,
    };
    use crate::commands::drive_dto::{
        ListDriveFilesInput, RenameDriveItemInput, StartTransferOperationInput, TrashDriveItemInput,
    };
    use crate::domain::{AccountId, ConnectedAccount, GooglePermissionId};
    use crate::infrastructure::account_store::SqliteAccountStore;
    use crate::infrastructure::google_drive::GoogleDriveClient;
    use crate::infrastructure::google_token::DynamicGoogleTokenClient;
    use crate::infrastructure::job_store::SqliteJobStore;
    use crate::infrastructure::secrets::NativeCredentialStore;
    use crate::state::{AppState, OAuthConfig};
    use crate::test_support::spawn_http_handler;

    const SOURCE_TOKEN: &str = "source-test-token";
    const TARGET_TOKEN: &str = "target-test-token";
    const SOURCE_PERM: &str = "perm-src-123";
    const TARGET_PERM: &str = "perm-tgt-456";

    async fn build_state(drive_client: GoogleDriveClient) -> AppState {
        let account_store = Arc::new(SqliteAccountStore::open_in_memory().await.unwrap());
        let job_store = Arc::new(SqliteJobStore::new(account_store.pool().clone()));
        let cred_store = Arc::new(NativeCredentialStore::new_mock());
        let oauth_config = Arc::new(RwLock::new(Some(OAuthConfig::new("client-id", None))));
        let token_service = Arc::new(DynamicGoogleTokenClient::new(oauth_config.clone()));
        let token_provider = Arc::new(AccountTokenProvider::new(
            token_service.clone(),
            cred_store.clone(),
            account_store.clone(),
        ));
        let connect_account_use_case: Arc<dyn ConnectAccountUseCase> =
            Arc::new(DummyConnectAccountUseCase);
        let lifecycle_service = crate::application::AccountLifecycleService::new(
            token_service,
            drive_client.clone(),
            account_store.clone(),
            cred_store.clone(),
        )
        .with_job_store(job_store.clone());
        let account_lifecycle_use_case: Arc<dyn AccountLifecycleUseCase> =
            Arc::new(lifecycle_service);
        let drive_arc = Arc::new(drive_client.clone());
        let job_service = Arc::new(JobService::new(
            account_store.clone(),
            job_store.clone(),
            drive_arc.clone() as Arc<dyn crate::application::DrivePort>,
            token_provider.clone(),
        ));
        AppState::new(
            account_store,
            cred_store,
            oauth_config,
            connect_account_use_case,
            account_lifecycle_use_case,
            token_provider,
            job_store,
            drive_arc,
            job_service,
        )
    }

    async fn create_test_account(
        store: &SqliteAccountStore,
        id: u128,
        email: &str,
        name: &str,
        perm: &str,
    ) {
        let acc = ConnectedAccount::new_personal(
            AccountId::new(id),
            GooglePermissionId::new(perm),
            email,
            name,
            None,
        )
        .unwrap();
        store.connect(&acc).await.unwrap();
    }

    #[tokio::test]
    async fn list_drive_files_returns_browse_children_and_identifies_ownership() {
        let files_json = serde_json::json!({
            "files": [
                {
                    "id": "folder-1",
                    "name": "My Folder", "resourceKey": "folder-key",
                    "mimeType": "application/vnd.google-apps.folder",
                    "owners": [{"permissionId": SOURCE_PERM, "emailAddress": "src@gmail.com", "photoLink": "https://lh3.googleusercontent.com/test-owner"}],
                    "modifiedTime": "2026-09-01T10:00:00Z",
                    "webViewLink": "https://drive.google.com/folder-1"
                },
                {
                    "id": "file-2",
                    "name": "Shared Document.pdf",
                    "mimeType": "application/pdf",
                    "size": "2048",
                    "owners": [{"permissionId": "other-perm", "emailAddress": "other@gmail.com"}],
                    "modifiedTime": "2026-09-02T12:00:00Z"
                },
                {
                    "id": "folder-shortcut",
                    "name": "Shared folder shortcut", "resourceKey": "shortcut-key",
                    "mimeType": "application/vnd.google-apps.shortcut",
                    "shortcutDetails": {
                        "targetId": "shared-folder", "targetResourceKey": "target-key",
                        "targetMimeType": "application/vnd.google-apps.folder"
                    },
                    "owners": [{"permissionId": SOURCE_PERM}]
                },
                {
                    "id": "file-shortcut",
                    "name": "Document shortcut",
                    "mimeType": "application/vnd.google-apps.shortcut",
                    "shortcutDetails": {"targetId": "document", "targetMimeType": "application/pdf"}
                },
                {
                    "id": "unknown-shortcut",
                    "name": "Shortcut without target type",
                    "mimeType": "application/vnd.google-apps.shortcut",
                    "shortcutDetails": {"targetId": "unknown"}
                }
            ],
            "nextPageToken": "token-next-page"
        });

        let (base_url, _) = spawn_http_handler(move |request| {
            if crate::test_support::request_method(request) == Some("GET") {
                ("200 OK".into(), files_json.to_string())
            } else {
                ("404 Not Found".into(), "{}".into())
            }
        });

        let state = build_state(GoogleDriveClient::for_test(base_url).unwrap()).await;
        create_test_account(&state.account_store, 1, "src@gmail.com", "Src", SOURCE_PERM).await;
        state
            .token_provider
            .insert_cached_token_for_test(AccountId::new(1), AccessToken::new(SOURCE_TOKEN.into()))
            .await;

        let res = list_drive_files_inner(
            &state,
            ListDriveFilesInput {
                account_id: "1".into(),
                folder_id: None,
                folder_resource_key: None,
                page_token: None,
                page_size: Some(50),
                order_by: None,
            },
        )
        .await
        .expect("list drive files");

        assert_eq!(res.items.len(), 5);
        assert_eq!(res.next_page_token.as_deref(), Some("token-next-page"));

        let folder = &res.items[0];
        assert_eq!(folder.id, "folder-1");
        assert!(folder.is_folder);
        assert!(folder.is_owner);
        assert!(folder.can_transfer_ownership);
        assert_eq!(folder.size, None);

        let file = &res.items[1];
        assert_eq!(file.id, "file-2");
        assert!(!file.is_folder);
        assert!(!file.is_owner);
        assert!(!file.can_transfer_ownership);
        assert_eq!(file.size, Some(2048));

        let serialized = serde_json::to_value(&res).unwrap();
        assert_eq!(serialized["items"][0]["folderId"], "folder-1");
        assert_eq!(
            serialized["items"][0]["owners"][0]["avatarUrl"],
            "https://lh3.googleusercontent.com/test-owner"
        );
        assert!(serialized["items"][1]["owners"][0]["avatarUrl"].is_null());
        assert!(serialized["items"][1]["folderId"].is_null());
        assert_eq!(serialized["items"][2]["folderId"], "shared-folder");
        assert_eq!(serialized["items"][0]["folderResourceKey"], "folder-key");
        assert_eq!(serialized["items"][2]["folderResourceKey"], "target-key");
        assert_eq!(serialized["items"][2]["resourceKey"], "shortcut-key");
        assert!(serialized["items"][3]["folderId"].is_null());
        assert!(serialized["items"][4]["folderId"].is_null());
        let shortcut = &res.items[2];
        assert_eq!(shortcut.id, "folder-shortcut");
        assert!(!shortcut.is_folder);
        assert!(shortcut.is_owner);
        assert!(shortcut.can_transfer_ownership);
        assert_eq!(
            shortcut.shortcut_target_id.as_deref(),
            Some("shared-folder")
        );
    }

    #[tokio::test]
    async fn rename_and_trash_drive_item_execute_patch_requests() {
        let (base_url, captured) = spawn_http_handler(|request| {
            if crate::test_support::request_method(request) == Some("PATCH") {
                ("200 OK".into(), "{}".into())
            } else {
                ("404 Not Found".into(), "{}".into())
            }
        });

        let state = build_state(GoogleDriveClient::for_test(base_url).unwrap()).await;
        create_test_account(&state.account_store, 1, "src@gmail.com", "Src", SOURCE_PERM).await;
        state
            .token_provider
            .insert_cached_token_for_test(AccountId::new(1), AccessToken::new(SOURCE_TOKEN.into()))
            .await;

        rename_drive_item_inner(
            &state,
            RenameDriveItemInput {
                account_id: "1".into(),
                file_id: "folder-shortcut".into(),
                new_name: "Renamed File.txt".into(),
            },
        )
        .await
        .expect("rename");

        trash_drive_item_inner(
            &state,
            TrashDriveItemInput {
                account_id: "1".into(),
                file_id: "folder-shortcut".into(),
            },
        )
        .await
        .expect("trash");

        let requests = captured.lock().unwrap().clone();
        assert_eq!(requests.len(), 2);
        assert!(
            requests[0].contains("PATCH /drive/v3/files/folder-shortcut?supportsAllDrives=true")
        );
        assert!(requests[0].contains("Renamed File.txt"));
        assert!(
            requests[1].contains("PATCH /drive/v3/files/folder-shortcut?supportsAllDrives=true")
        );
        assert!(requests[1].contains("\"trashed\":true"));
    }

    #[tokio::test]
    async fn start_transfer_operation_creates_and_scans_job() {
        let (base_url, _) = spawn_http_handler(|request| {
            if crate::test_support::request_path(request).is_some_and(|p| p.contains("about")) {
                (
                    "200 OK".into(),
                    r#"{"storageQuota":{"limit":"1000000","usage":"500"}}"#.into(),
                )
            } else if crate::test_support::request_method(request) == Some("GET") {
                (
                    "200 OK".into(),
                    format!(
                        r#"{{"id":"root-folder","name":"My Folder","mimeType":"application/vnd.google-apps.folder","trashed":false,"owners":[{{"permissionId":"{SOURCE_PERM}","emailAddress":"src@gmail.com"}}]}}"#
                    ),
                )
            } else {
                ("404 Not Found".into(), "{}".into())
            }
        });

        let state = build_state(GoogleDriveClient::for_test(base_url).unwrap()).await;
        create_test_account(&state.account_store, 1, "src@gmail.com", "Src", SOURCE_PERM).await;
        create_test_account(&state.account_store, 2, "tgt@gmail.com", "Tgt", TARGET_PERM).await;
        state
            .token_provider
            .insert_cached_token_for_test(AccountId::new(1), AccessToken::new(SOURCE_TOKEN.into()))
            .await;
        state
            .token_provider
            .insert_cached_token_for_test(AccountId::new(2), AccessToken::new(TARGET_TOKEN.into()))
            .await;

        let job = start_transfer_operation_inner(
            &state,
            StartTransferOperationInput {
                source_account_id: "1".into(),
                target_account_id: "2".into(),
                root_file_ids: vec!["root-folder".into()],
                recursive: true,
            },
        )
        .await
        .expect("start transfer operation");

        assert_eq!(job.source_account_id, "1");
        assert_eq!(job.target_account_id, "2");
        assert_eq!(job.roots.len(), 1);
        assert_eq!(job.roots[0].root_file_id, "root-folder");
        assert!(
            job.status == "SCANNING"
                || job.status == "READY_FOR_REVIEW"
                || job.status == "RUNNING_CANARY"
                || job.status == "COMPLETED"
        );
    }
    #[tokio::test]
    async fn browse_resource_key_uses_selected_token_and_survives_paging() {
        let (base_url, captured) = spawn_http_handler(|_| {
            (
                "200 OK".into(),
                r#"{"files":[],"nextPageToken":"next"}"#.into(),
            )
        });
        let state = build_state(GoogleDriveClient::for_test(base_url).unwrap()).await;
        create_test_account(&state.account_store, 1, "src@gmail.com", "Src", SOURCE_PERM).await;
        create_test_account(&state.account_store, 2, "tgt@gmail.com", "Tgt", TARGET_PERM).await;
        state
            .token_provider
            .insert_cached_token_for_test(AccountId::new(2), AccessToken::new(TARGET_TOKEN.into()))
            .await;
        for (folder_id, key, page) in [
            (Some("shared-folder"), Some("target-key"), None),
            (Some("shared-folder"), Some("target-key"), Some("next")),
            (Some("shared-folder"), None, None),
            (Some("shared-folder"), Some(""), None),
            (None, Some("target-key"), None),
        ] {
            let result = list_drive_files_inner(
                &state,
                serde_json::from_value(serde_json::json!({
                    "accountId": "2", "folderId": folder_id, "folderResourceKey": key,
                    "pageToken": page, "pageSize": 25, "orderBy": "name"
                }))
                .unwrap(),
            )
            .await
            .unwrap();
            assert_eq!(result.next_page_token.as_deref(), Some("next"));
        }
        let requests = captured.lock().unwrap();
        assert_eq!(requests.len(), 5);
        for (index, request) in requests.iter().enumerate() {
            assert!(request.contains(&format!("Bearer {TARGET_TOKEN}")));
            assert!(!request.contains(SOURCE_TOKEN));
            let fields = crate::test_support::query_param(request, "fields").unwrap();
            assert!(fields.contains("resourceKey"));
            assert!(fields.contains("shortcutDetails"));
            assert_eq!(
                crate::test_support::query_param(request, "q").as_deref(),
                Some(if index < 4 {
                    "'shared-folder' in parents and trashed=false"
                } else {
                    "'root' in parents and trashed=false"
                })
            );
            let header = request.to_ascii_lowercase();
            assert_eq!(
                header.contains("x-goog-drive-resource-keys: shared-folder/target-key"),
                index < 2
            );
            assert_eq!(header.contains("x-goog-drive-resource-keys:"), index < 2);
            assert_eq!(
                crate::test_support::query_param(request, "pageSize").as_deref(),
                Some("25")
            );
        }
        assert_eq!(
            crate::test_support::query_param(&requests[1], "pageToken").as_deref(),
            Some("next")
        );
    }

    #[tokio::test]
    async fn open_drive_item_resolves_selected_connected_account_and_rejects_invalid_accounts() {
        let state =
            build_state(GoogleDriveClient::for_test("http://127.0.0.1:1".into()).unwrap()).await;
        create_test_account(&state.account_store, 1, "src@gmail.com", "Src", SOURCE_PERM).await;
        create_test_account(
            &state.account_store,
            2,
            "target+tag@gmail.com",
            "Target",
            TARGET_PERM,
        )
        .await;
        for (account_id, expected_email) in [("1", "src@gmail.com"), ("2", "target+tag@gmail.com")]
        {
            let url = crate::commands::drive::drive_item_url_inner(
                &state,
                serde_json::from_value(serde_json::json!({
                    "accountId": account_id, "fileId": "original-shortcut"
                }))
                .unwrap(),
            )
            .await
            .unwrap();
            assert_eq!(
                url.query_pairs().collect::<Vec<_>>(),
                vec![
                    ("id".into(), "original-shortcut".into()),
                    ("authuser".into(), expected_email.into())
                ]
            );
        }
        state
            .account_store
            .update_auth_status(AccountId::new(2), crate::domain::AuthStatus::Disconnected)
            .await
            .unwrap();
        for account_id in ["invalid", "99", "2"] {
            assert!(
                crate::commands::drive::drive_item_url_inner(
                    &state,
                    serde_json::from_value(serde_json::json!({
                        "accountId": account_id, "fileId": "original-shortcut"
                    }))
                    .unwrap()
                )
                .await
                .is_err()
            );
        }
        assert!(
            serde_json::from_value::<crate::commands::drive_dto::OpenDriveItemInput>(
                serde_json::json!({
                    "accountId": "1", "fileId": "original-shortcut", "email": "injected@gmail.com"
                })
            )
            .is_err()
        );
    }
}
