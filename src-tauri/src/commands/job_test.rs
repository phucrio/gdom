#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use tokio::sync::RwLock;

    use crate::application::AccessToken;
    use crate::application::item_store::{ItemBatchCommit, ItemStorePort};
    use crate::application::{
        AccountLifecycleUseCase, AccountTokenProvider, ConnectAccountUseCase, JobService,
        JobStorePort,
    };
    use crate::commands::dto::{
        CreateJobInput, ExportDryRunInput, JobIdInput, ListJobItemsInput, ListJobsFilter,
        QueueJobInput, StartCanaryInput, UpdateDraftJobAccountsInput,
    };
    use crate::commands::error::CommandError;
    use crate::commands::job::{
        cancel_migration_inner, create_job_inner, delete_draft_job_inner, export_dry_run_inner,
        get_account_references_inner, get_job_inner, list_job_items_inner, list_jobs_inner,
        pause_migration_inner, pause_scan_inner, queue_job_inner, retry_failed_items_inner,
        start_canary_inner, start_scan_inner, update_draft_job_accounts_inner,
    };
    use crate::domain::job::{MigrationRoot, RootId, RootValidationStatus};
    use crate::domain::{AccountId, ConnectedAccount, GooglePermissionId};
    use crate::infrastructure::SqliteJobStore;
    use crate::infrastructure::account_store::SqliteAccountStore;
    use crate::infrastructure::google_drive::GoogleDriveClient;
    use crate::infrastructure::google_token::DynamicGoogleTokenClient;
    use crate::infrastructure::secrets::WindowsCredentialStore;
    use crate::state::{AppState, OAuthConfig};
    use crate::test_support::{
        SOURCE_PERM, SOURCE_TOKEN, TARGET_PERM, TARGET_TOKEN, folder_id_from_list_request,
        query_param, request_is_list, request_is_quota, shortcut_json, source_file, source_folder,
        spawn_http_handler,
    };

    struct DummyConnectAccountUseCase;
    impl ConnectAccountUseCase for DummyConnectAccountUseCase {
        fn connect_account(
            &self,
            _grant: crate::application::OAuthGrant,
            _fallback_account_id: AccountId,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<
                        Output = Result<ConnectedAccount, crate::application::ConnectAccountError>,
                    > + Send
                    + '_,
            >,
        > {
            Box::pin(async { unimplemented!() })
        }
    }

    async fn build_test_state() -> AppState {
        let account_store = Arc::new(SqliteAccountStore::open_in_memory().await.unwrap());
        let job_store = Arc::new(SqliteJobStore::new(account_store.pool().clone()));
        let cred_store = Arc::new(WindowsCredentialStore::new_mock());
        let oauth_config = Arc::new(RwLock::new(Some(OAuthConfig::new("test-client", None))));

        let token_service = Arc::new(DynamicGoogleTokenClient::new(oauth_config.clone()));
        let drive_client = GoogleDriveClient::new().unwrap();

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

        let job_service = Arc::new(JobService::new(
            account_store.clone(),
            job_store.clone(),
            Arc::new(drive_client.clone()) as Arc<dyn crate::application::DrivePort>,
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
            Arc::new(drive_client),
            job_service,
        )
    }

    async fn await_job_idle(state: &AppState, job_id: &str) {
        let parsed = job_id.parse().expect("job id");
        state.job_service.await_idle(parsed).await;
    }

    async fn create_dummy_account(
        store: &SqliteAccountStore,
        id: u128,
        email: &str,
        name: &str,
        perm: &str,
    ) -> ConnectedAccount {
        let acc = ConnectedAccount::new_personal(
            AccountId::new(id),
            GooglePermissionId::new(perm),
            email,
            name,
            None,
        )
        .unwrap();
        store.connect(&acc).await.unwrap()
    }

    #[tokio::test]
    async fn queue_actions_reject_invalid_job_ids_before_store_access() {
        use crate::commands::job_actions::{
            ReorderQueuedJobInput, remove_queued_job_inner, reorder_queued_job_inner,
        };
        let state = build_test_state().await;
        let reordered = reorder_queued_job_inner(
            &state,
            ReorderQueuedJobInput {
                job_id: "invalid".into(),
                position: 1,
            },
        )
        .await;
        let removed = remove_queued_job_inner(
            &state,
            JobIdInput {
                job_id: "invalid".into(),
            },
        )
        .await;
        assert!(matches!(reordered, Err(CommandError::JobNotFound(_))));
        assert!(matches!(removed, Err(CommandError::JobNotFound(_))));
    }

    #[tokio::test]
    async fn final_report_command_exports_cancelled_job_without_credentials() {
        use crate::commands::job_actions::export_final_report_inner;
        let state = build_test_state().await;
        create_dummy_account(
            &state.account_store,
            1,
            "source@gmail.com",
            "Source",
            "source",
        )
        .await;
        create_dummy_account(
            &state.account_store,
            2,
            "target@gmail.com",
            "Target",
            "target",
        )
        .await;
        let job = create_job_inner(
            &state,
            CreateJobInput {
                source_account_id: "1".into(),
                target_account_id: "2".into(),
            },
        )
        .await
        .expect("draft");
        let path = std::env::temp_dir().join(format!("gdom-final-command-{}.csv", job.id));
        let destination = path.to_string_lossy().into_owned();
        let draft_result = export_final_report_inner(
            &state,
            ExportDryRunInput {
                job_id: job.id.clone(),
                destination: destination.clone(),
            },
        )
        .await;
        assert!(matches!(
            draft_result,
            Err(CommandError::IllegalJobTransition(_))
        ));
        cancel_migration_inner(
            &state,
            JobIdInput {
                job_id: job.id.clone(),
            },
        )
        .await
        .expect("cancel draft");
        let report = export_final_report_inner(
            &state,
            ExportDryRunInput {
                job_id: job.id,
                destination,
            },
        )
        .await
        .expect("local report");
        let result = serde_json::to_value(report).expect("IPC response");
        assert_eq!(result["status"], "CANCELLED");
        assert_eq!(result["counts"]["total"], 0);
        assert_eq!(result["counts"]["verified"], 0);
        assert!(
            std::fs::read_to_string(&path)
                .expect("written CSV")
                .contains("CANCELLED")
        );
        std::fs::remove_file(path).expect("remove test output");
    }

    #[tokio::test]
    async fn progress_preserves_verified_items_and_terminal_history() {
        // Given persisted outcomes, including a transferred item awaiting verification.
        let state = build_test_state().await;
        create_dummy_account(
            &state.account_store,
            1,
            "source@gmail.com",
            "Source",
            "source",
        )
        .await;
        create_dummy_account(
            &state.account_store,
            2,
            "target@gmail.com",
            "Target",
            "target",
        )
        .await;
        let job = create_job_inner(
            &state,
            CreateJobInput {
                source_account_id: "1".into(),
                target_account_id: "2".into(),
            },
        )
        .await
        .unwrap();
        let job_id = job.id.parse().unwrap();
        use crate::domain::item::{ItemId, ItemState, MigrationItem};
        let items = [
            ItemState::Verified,
            ItemState::Verified,
            ItemState::Transferred,
            ItemState::RetryableFailed,
            ItemState::PermanentFailed,
            ItemState::SkippedIneligible,
            ItemState::SkippedShortcutTarget,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, item_state)| MigrationItem {
            id: ItemId::new(index as u128 + 1),
            job_id,
            file_id: index.to_string(),
            name: index.to_string(),
            mime_type: "text/plain".into(),
            depth: 0,
            original_parent_ids: vec![],
            original_owner_permission_id: None,
            quota_bytes_used: None,
            target_permission_id: None,
            state: item_state,
            canary_selected: false,
            created_at: "t".into(),
            updated_at: "t".into(),
        })
        .collect();
        state
            .job_store
            .commit_scan_batch(
                job_id,
                &ItemBatchCommit {
                    items,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        // When the command reports progress.
        let result = get_job_inner(&state, JobIdInput { job_id: job.id })
            .await
            .unwrap();
        // Then successes remain counted and the denominator includes every persisted item.
        let progress = result.progress.unwrap();
        assert_eq!((progress.completed, progress.total), (2, 7));
        assert_eq!((progress.failed, progress.skipped), (2, 2));

        sqlx::query("UPDATE migration_items SET state = CASE state WHEN 'TRANSFERRED' THEN 'VERIFIED' WHEN 'RETRYABLE_FAILED' THEN 'PERMANENT_FAILED' ELSE state END WHERE job_id = ?1")
            .bind(job_id.value().to_string()).execute(state.account_store.pool()).await.unwrap();
        let history = get_job_inner(
            &state,
            JobIdInput {
                job_id: job_id.value().to_string(),
            },
        )
        .await
        .unwrap();
        let history_progress = history.progress.unwrap();
        assert_eq!((history_progress.completed, history_progress.total), (3, 7));
        assert_eq!((history_progress.failed, history_progress.skipped), (2, 2));
    }

    #[tokio::test]
    async fn job_lifecycle_and_root_management_integration() {
        let state = build_test_state().await;

        // Seed two connected accounts
        create_dummy_account(
            &state.account_store,
            1,
            "source@gmail.com",
            "Source User",
            "perm-source-1",
        )
        .await;
        create_dummy_account(
            &state.account_store,
            2,
            "target@gmail.com",
            "Target User",
            "perm-target-2",
        )
        .await;
        create_dummy_account(
            &state.account_store,
            3,
            "target2@gmail.com",
            "Target Two",
            "perm-target-3",
        )
        .await;

        // 1. Invariant: Source and target cannot be same
        let same_acc_input = CreateJobInput {
            source_account_id: "1".to_string(),
            target_account_id: "1".to_string(),
        };
        let err = create_job_inner(&state, same_acc_input).await;
        assert!(matches!(err, Err(CommandError::SameSourceAndTarget(_))));

        // 2. Create job succeeds
        let create_input = CreateJobInput {
            source_account_id: "1".to_string(),
            target_account_id: "2".to_string(),
        };
        let job_dto = create_job_inner(&state, create_input)
            .await
            .expect("job created");
        assert_eq!(job_dto.source_account_id, "1");
        assert_eq!(job_dto.target_account_id, "2");
        assert_eq!(job_dto.status, "DRAFT");
        assert_eq!(job_dto.source_snapshot.email, "source@gmail.com");
        assert_eq!(job_dto.target_snapshot.email, "target@gmail.com");

        let job_id = job_dto.id.clone();

        // 3. Update draft job accounts
        let update_input = UpdateDraftJobAccountsInput {
            job_id: job_id.clone(),
            source_account_id: "1".to_string(),
            target_account_id: "3".to_string(),
        };
        let updated = update_draft_job_accounts_inner(&state, update_input)
            .await
            .expect("draft accounts updated");
        assert_eq!(updated.target_account_id, "3");
        assert_eq!(updated.target_snapshot.email, "target2@gmail.com");

        // 4. List jobs
        let list = list_jobs_inner(&state, None)
            .await
            .expect("list jobs succeeds");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, job_id);

        // 5. Get job
        let fetched = get_job_inner(
            &state,
            JobIdInput {
                job_id: job_id.clone(),
            },
        )
        .await
        .expect("get job succeeds");
        assert_eq!(fetched.id, job_id);

        // 6. Active job check prevents deletion of source account
        let delete_err = state
            .account_lifecycle_use_case
            .delete_local_account_data(AccountId::new(1))
            .await;
        assert!(matches!(
            delete_err,
            Err(crate::application::AccountLifecycleError::ActiveJobsPreventRemoval)
        ));

        sqlx::query("UPDATE migration_jobs SET status = 'COMPLETED_WITH_ERRORS' WHERE id = ?1")
            .bind(&job_id)
            .execute(state.account_store.pool())
            .await
            .unwrap();

        let delete_historical = state
            .account_lifecycle_use_case
            .delete_local_account_data(AccountId::new(1))
            .await;
        assert!(matches!(
            delete_historical,
            Err(crate::application::AccountLifecycleError::ActiveJobsPreventRemoval)
        ));
    }

    #[tokio::test]
    async fn list_filter_account_references_and_draft_delete() {
        let state = build_test_state().await;
        create_dummy_account(
            &state.account_store,
            1,
            "source@gmail.com",
            "Source User",
            "perm-source-1",
        )
        .await;
        create_dummy_account(
            &state.account_store,
            2,
            "target@gmail.com",
            "Target User",
            "perm-target-2",
        )
        .await;
        create_dummy_account(
            &state.account_store,
            3,
            "other@gmail.com",
            "Other User",
            "perm-other-3",
        )
        .await;

        let first = create_job_inner(
            &state,
            CreateJobInput {
                source_account_id: "1".to_string(),
                target_account_id: "2".to_string(),
            },
        )
        .await
        .expect("first job");
        let second = create_job_inner(
            &state,
            CreateJobInput {
                source_account_id: "3".to_string(),
                target_account_id: "2".to_string(),
            },
        )
        .await
        .expect("second job");

        let filtered = list_jobs_inner(
            &state,
            Some(ListJobsFilter {
                status: None,
                account_id: Some("1".to_string()),
            }),
        )
        .await
        .expect("filtered list");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, first.id);
        assert_eq!(filtered[0].source_snapshot.email, "source@gmail.com");
        assert_eq!(filtered[0].target_snapshot.email, "target@gmail.com");

        let references = get_account_references_inner(&state, AccountId::new(2))
            .await
            .expect("target references");
        assert_eq!(references.account_id, "2");
        assert_eq!(references.jobs.len(), 2);
        assert!(
            references
                .jobs
                .iter()
                .all(|job| job.role == "target" && job.status == "DRAFT")
        );

        delete_draft_job_inner(
            &state,
            JobIdInput {
                job_id: first.id.clone(),
            },
        )
        .await
        .expect("draft deleted");

        let after_delete = list_jobs_inner(&state, None)
            .await
            .expect("list after delete");
        assert_eq!(after_delete.len(), 1);
        assert_eq!(after_delete[0].id, second.id);

        sqlx::query("UPDATE migration_jobs SET status = 'SCANNING' WHERE id = ?1")
            .bind(&second.id)
            .execute(state.account_store.pool())
            .await
            .unwrap();

        let scanned_delete = delete_draft_job_inner(
            &state,
            JobIdInput {
                job_id: second.id.clone(),
            },
        )
        .await;
        assert!(matches!(
            scanned_delete,
            Err(CommandError::IllegalJobTransition(_))
        ));

        let missing_account = get_account_references_inner(&state, AccountId::new(99)).await;
        assert!(matches!(
            missing_account,
            Err(CommandError::AccountNotFound(_))
        ));
    }

    async fn build_state_with_drive(drive: GoogleDriveClient) -> AppState {
        let account_store = Arc::new(SqliteAccountStore::open_in_memory().await.unwrap());
        let job_store = Arc::new(SqliteJobStore::new(account_store.pool().clone()));
        let cred_store = Arc::new(WindowsCredentialStore::new_mock());
        let oauth_config = Arc::new(RwLock::new(Some(OAuthConfig::new("test-client", None))));
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
            drive.clone(),
            account_store.clone(),
            cred_store.clone(),
        )
        .with_job_store(job_store.clone());
        let account_lifecycle_use_case: Arc<dyn AccountLifecycleUseCase> =
            Arc::new(lifecycle_service);
        let job_service = Arc::new(JobService::new(
            account_store.clone(),
            job_store.clone(),
            Arc::new(drive.clone()) as Arc<dyn crate::application::DrivePort>,
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
            Arc::new(drive),
            job_service,
        )
    }

    #[tokio::test]
    async fn start_scan_preflight_list_and_export_roundtrip() {
        let (base_url, captured) = spawn_http_handler(|request| {
            if crate::test_support::request_path(request)
                .is_some_and(|path| path.split('?').next() == Some("/drive/v3/files/root-1"))
            {
                return ("200 OK".into(), source_folder("root-1", "Root One"));
            }
            if request_is_quota(request) {
                return (
                    "200 OK".into(),
                    r#"{"storageQuota":{"limit":"10000","usage":"100"}}"#.into(),
                );
            }
            if request_is_list(request) {
                if query_param(request, "pageToken").is_some() {
                    return (
                        "200 OK".into(),
                        format!(r#"{{"files":[{}]}}"#, source_file("paged", "Paged", 7)),
                    );
                }
                let folder = folder_id_from_list_request(request).unwrap_or_default();
                if folder == "root-1" {
                    return (
                        "200 OK".into(),
                        format!(
                            r#"{{"files":[{},{}],"nextPageToken":"next"}}"#,
                            source_folder("nested", "Nested"),
                            shortcut_json("short", "target-skip"),
                        ),
                    );
                }
                return ("200 OK".into(), r#"{"files":[]}"#.into());
            }
            ("404 Not Found".into(), "{}".into())
        });

        let state = build_state_with_drive(GoogleDriveClient::for_test(base_url).unwrap()).await;
        create_dummy_account(
            &state.account_store,
            1,
            "source@gmail.com",
            "Source User",
            SOURCE_PERM,
        )
        .await;
        create_dummy_account(
            &state.account_store,
            2,
            "target@gmail.com",
            "Target User",
            TARGET_PERM,
        )
        .await;
        state
            .token_provider
            .insert_cached_token_for_test(AccountId::new(1), AccessToken::new(SOURCE_TOKEN.into()))
            .await;
        state
            .token_provider
            .insert_cached_token_for_test(AccountId::new(2), AccessToken::new(TARGET_TOKEN.into()))
            .await;

        let job = create_job_inner(
            &state,
            CreateJobInput {
                source_account_id: "1".into(),
                target_account_id: "2".into(),
            },
        )
        .await
        .unwrap();
        let job_id: crate::domain::job::JobId = job.id.parse().unwrap();
        state
            .job_store
            .add_root(&MigrationRoot {
                id: RootId::new(88),
                job_id,
                root_file_id: "root-1".into(),
                root_name: "Root One".into(),
                validation_status: RootValidationStatus::Validated,
                created_at: "2026-09-05T00:00:00Z".into(),
            })
            .await
            .unwrap();

        let started = start_scan_inner(
            &state,
            JobIdInput {
                job_id: job.id.clone(),
            },
        )
        .await
        .expect("scan starts");
        assert_eq!(started.status, "SCANNING");
        await_job_idle(&state, &job.id).await;
        let scanned = get_job_inner(
            &state,
            JobIdInput {
                job_id: job.id.clone(),
            },
        )
        .await
        .expect("scan completes");
        assert_eq!(scanned.status, "READY_FOR_REVIEW");
        let scan = scanned.scan.expect("preflight summary attached");
        assert_eq!(scan.folders, 2);
        assert_eq!(scan.files, 1);
        assert!(scan.skipped >= 1);
        assert!(scan.shortcuts >= 1);
        assert_eq!(scan.eligible_items, scan.files + scan.folders);
        assert!(!scan.quota_warning);
        assert!(scan.target_remaining_bytes.is_some());

        let page = list_job_items_inner(
            &state,
            ListJobItemsInput {
                job_id: job.id.clone(),
                filter: Some("eligible".into()),
                page: Some(1),
            },
        )
        .await
        .unwrap();
        assert!(!page.items.is_empty());
        assert!(page.items.iter().all(|item| item.state == "ELIGIBLE"));

        let skipped = list_job_items_inner(
            &state,
            ListJobItemsInput {
                job_id: job.id.clone(),
                filter: Some("skipped".into()),
                page: Some(1),
            },
        )
        .await
        .unwrap();
        assert!(skipped.items.iter().any(|item| item.file_id == "short"));
        assert!(
            skipped
                .items
                .iter()
                .any(|item| item.state == "SKIPPED_SHORTCUT_TARGET")
        );

        let shortcut = list_job_items_inner(
            &state,
            ListJobItemsInput {
                job_id: job.id.clone(),
                filter: Some("shortcut".into()),
                page: Some(1),
            },
        )
        .await
        .unwrap();
        assert_eq!(shortcut.items.len(), 1);
        assert_eq!(shortcut.items[0].file_id, "short");

        let ineligible = list_job_items_inner(
            &state,
            ListJobItemsInput {
                job_id: job.id.clone(),
                filter: Some("ineligible".into()),
                page: Some(1),
            },
        )
        .await
        .unwrap();
        assert!(ineligible.items.iter().all(|item| item.file_id != "short"));
        assert!(ineligible.items.iter().all(|item| {
            matches!(
                item.state.as_str(),
                "SKIPPED_NOT_OWNED_BY_SOURCE"
                    | "SKIPPED_SHARED_DRIVE"
                    | "SKIPPED_TRASHED"
                    | "SKIPPED_INELIGIBLE"
            )
        }));

        let dest = std::env::temp_dir().join("gdom-dry-run-export.txt");
        let exported = export_dry_run_inner(
            &state,
            ExportDryRunInput {
                job_id: job.id.clone(),
                destination: dest.to_string_lossy().into_owned(),
            },
        )
        .await
        .unwrap();
        assert_eq!(exported.eligible_items, scan.files + scan.folders);
        let body = std::fs::read_to_string(&dest).unwrap();
        assert!(body.contains("Eligible items:"));
        assert!(body.contains("Target remaining bytes:"));
        assert!(!body.contains(SOURCE_TOKEN));
        assert!(!body.contains(TARGET_TOKEN));
        assert!(!body.to_ascii_lowercase().contains("bearer"));

        let csv_dest = std::env::temp_dir().join("gdom-dry-run-export.csv");
        let csv_exported = export_dry_run_inner(
            &state,
            ExportDryRunInput {
                job_id: job.id.clone(),
                destination: csv_dest.to_string_lossy().into_owned(),
            },
        )
        .await
        .unwrap();
        assert_eq!(csv_exported.eligible_items, scan.files + scan.folders);
        let csv_body = std::fs::read_to_string(&csv_dest).unwrap();
        assert!(csv_body.starts_with("file_id,name,mime_type,depth,state,quota_bytes_used"));
        assert!(csv_body.contains("short"));
        assert!(!csv_body.contains(SOURCE_TOKEN));
        assert!(!csv_body.contains(TARGET_TOKEN));

        tokio::time::sleep(std::time::Duration::from_millis(40)).await;
        let requests = captured.lock().unwrap().clone();
        assert!(requests.iter().any(|req| request_is_list(req)));
        assert!(requests.iter().any(|req| request_is_quota(req)));
        for request in requests.iter().filter(|req| request_is_list(req)) {
            assert_eq!(
                crate::test_support::authorization_bearer(request).as_deref(),
                Some(format!("Bearer {SOURCE_TOKEN}").as_str())
            );
        }
        for request in requests.iter().filter(|req| request_is_quota(req)) {
            assert_eq!(
                crate::test_support::authorization_bearer(request).as_deref(),
                Some(format!("Bearer {TARGET_TOKEN}").as_str())
            );
        }
        assert!(!requests.iter().any(|req| req.contains("target-skip")));
    }

    #[tokio::test]
    async fn pause_scan_stops_further_pages() {
        let started = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let started_handler = std::sync::Arc::clone(&started);
        let (go_tx, go_rx) = std::sync::mpsc::channel::<()>();
        let go_rx = std::sync::Mutex::new(Some(go_rx));
        let (base_url, _) = spawn_http_handler(move |request| {
            if crate::test_support::request_path(request)
                .is_some_and(|path| path.split('?').next() == Some("/drive/v3/files/root-pause"))
            {
                return ("200 OK".into(), source_folder("root-pause", "Pause Root"));
            }
            if request_is_quota(request) {
                return (
                    "200 OK".into(),
                    r#"{"storageQuota":{"limit":"10000","usage":"1"}}"#.into(),
                );
            }
            if request_is_list(request) {
                started_handler.store(true, std::sync::atomic::Ordering::SeqCst);
                if let Some(rx) = go_rx.lock().unwrap().take() {
                    let _ = rx.recv_timeout(std::time::Duration::from_secs(5));
                }
                if query_param(request, "pageToken").is_some() {
                    return (
                        "200 OK".into(),
                        format!(r#"{{"files":[{}]}}"#, source_file("should-not", "Nope", 1)),
                    );
                }
                return (
                    "200 OK".into(),
                    format!(
                        r#"{{"files":[{}],"nextPageToken":"more"}}"#,
                        source_file("only-first", "First", 1)
                    ),
                );
            }
            ("404 Not Found".into(), "{}".into())
        });

        let state = std::sync::Arc::new(
            build_state_with_drive(GoogleDriveClient::for_test(base_url).unwrap()).await,
        );
        create_dummy_account(
            &state.account_store,
            1,
            "source@gmail.com",
            "Source User",
            SOURCE_PERM,
        )
        .await;
        create_dummy_account(
            &state.account_store,
            2,
            "target@gmail.com",
            "Target User",
            TARGET_PERM,
        )
        .await;
        state
            .token_provider
            .insert_cached_token_for_test(AccountId::new(1), AccessToken::new(SOURCE_TOKEN.into()))
            .await;
        state
            .token_provider
            .insert_cached_token_for_test(AccountId::new(2), AccessToken::new(TARGET_TOKEN.into()))
            .await;
        let job = create_job_inner(
            &state,
            CreateJobInput {
                source_account_id: "1".into(),
                target_account_id: "2".into(),
            },
        )
        .await
        .unwrap();
        let job_id: crate::domain::job::JobId = job.id.parse().unwrap();
        state
            .job_store
            .add_root(&MigrationRoot {
                id: RootId::new(99),
                job_id,
                root_file_id: "root-pause".into(),
                root_name: "Pause Root".into(),
                validation_status: RootValidationStatus::Validated,
                created_at: "2026-09-05T00:00:00Z".into(),
            })
            .await
            .unwrap();

        let started_job = start_scan_inner(
            &state,
            JobIdInput {
                job_id: job.id.clone(),
            },
        )
        .await
        .expect("scan starts");
        assert_eq!(started_job.status, "SCANNING");

        let wait_started = std::time::Instant::now();
        while !started.load(std::sync::atomic::Ordering::SeqCst) {
            if wait_started.elapsed() > std::time::Duration::from_secs(2) {
                panic!("scan did not list first page");
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        pause_scan_inner(
            &state,
            JobIdInput {
                job_id: job.id.clone(),
            },
        )
        .await
        .unwrap();
        let _ = go_tx.send(());
        await_job_idle(&state, &job.id).await;
        let scanned = get_job_inner(
            &state,
            JobIdInput {
                job_id: job.id.clone(),
            },
        )
        .await
        .unwrap();
        assert_eq!(scanned.status, "PAUSED");
        let items = list_job_items_inner(
            &state,
            ListJobItemsInput {
                job_id: job.id,
                filter: None,
                page: Some(1),
            },
        )
        .await
        .unwrap();
        let ids: Vec<_> = items
            .items
            .iter()
            .map(|item| item.file_id.as_str())
            .collect();
        assert!(ids.contains(&"only-first"));
        assert!(!ids.contains(&"should-not"));
    }

    async fn seed_job_ready_to_scan(
        state: &AppState,
        with_tokens: bool,
    ) -> (String, crate::domain::job::JobId) {
        create_dummy_account(
            &state.account_store,
            1,
            "source@gmail.com",
            "Source User",
            SOURCE_PERM,
        )
        .await;
        create_dummy_account(
            &state.account_store,
            2,
            "target@gmail.com",
            "Target User",
            TARGET_PERM,
        )
        .await;
        if with_tokens {
            state
                .token_provider
                .insert_cached_token_for_test(
                    AccountId::new(1),
                    AccessToken::new(SOURCE_TOKEN.into()),
                )
                .await;
            state
                .token_provider
                .insert_cached_token_for_test(
                    AccountId::new(2),
                    AccessToken::new(TARGET_TOKEN.into()),
                )
                .await;
        }
        let job = create_job_inner(
            state,
            CreateJobInput {
                source_account_id: "1".into(),
                target_account_id: "2".into(),
            },
        )
        .await
        .unwrap();
        let job_id: crate::domain::job::JobId = job.id.parse().unwrap();
        state
            .job_store
            .add_root(&MigrationRoot {
                id: RootId::new(77),
                job_id,
                root_file_id: "root-1".into(),
                root_name: "Root".into(),
                validation_status: RootValidationStatus::Validated,
                created_at: "2026-09-05T00:00:00Z".into(),
            })
            .await
            .unwrap();
        (job.id, job_id)
    }

    #[tokio::test]
    async fn second_start_scan_is_rejected_while_walker_is_in_flight() {
        let started = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let started_handler = std::sync::Arc::clone(&started);
        let (go_tx, go_rx) = std::sync::mpsc::channel::<()>();
        let go_rx = std::sync::Mutex::new(Some(go_rx));
        let (base_url, _) = spawn_http_handler(move |request| {
            if crate::test_support::request_path(request)
                .is_some_and(|path| path.split('?').next() == Some("/drive/v3/files/root-1"))
            {
                return ("200 OK".into(), source_folder("root-1", "Root"));
            }
            if request_is_quota(request) {
                return (
                    "200 OK".into(),
                    r#"{"storageQuota":{"limit":"10000","usage":"1"}}"#.into(),
                );
            }
            if request_is_list(request) {
                started_handler.store(true, std::sync::atomic::Ordering::SeqCst);
                if let Some(rx) = go_rx.lock().unwrap().take() {
                    let _ = rx.recv_timeout(std::time::Duration::from_secs(5));
                }
                return (
                    "200 OK".into(),
                    format!(r#"{{"files":[{}]}}"#, source_file("one", "One", 1)),
                );
            }
            ("404 Not Found".into(), "{}".into())
        });

        let state = std::sync::Arc::new(
            build_state_with_drive(GoogleDriveClient::for_test(base_url).unwrap()).await,
        );
        let (job_id_str, _) = seed_job_ready_to_scan(&state, true).await;

        let started_job = start_scan_inner(
            &state,
            JobIdInput {
                job_id: job_id_str.clone(),
            },
        )
        .await
        .expect("scan starts");
        assert_eq!(started_job.status, "SCANNING");

        let wait_started = std::time::Instant::now();
        while !started.load(std::sync::atomic::Ordering::SeqCst) {
            if wait_started.elapsed() > std::time::Duration::from_secs(2) {
                panic!("scan did not list first page");
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let overlap = start_scan_inner(
            &state,
            JobIdInput {
                job_id: job_id_str.clone(),
            },
        )
        .await;
        assert!(matches!(overlap, Err(CommandError::ScanInProgress(_))));

        let _ = go_tx.send(());
        await_job_idle(&state, &job_id_str).await;
        let finished = get_job_inner(&state, JobIdInput { job_id: job_id_str })
            .await
            .unwrap();
        assert_eq!(finished.status, "READY_FOR_REVIEW");
    }

    #[tokio::test]
    async fn token_error_on_draft_does_not_leave_scanning() {
        let (base_url, _) = spawn_http_handler(|_| ("200 OK".into(), r#"{"files":[]}"#.into()));
        let state = build_state_with_drive(GoogleDriveClient::for_test(base_url).unwrap()).await;
        let (job_id_str, _) = seed_job_ready_to_scan(&state, false).await;

        let err = start_scan_inner(
            &state,
            JobIdInput {
                job_id: job_id_str.clone(),
            },
        )
        .await
        .expect_err("missing token fails");
        assert!(matches!(err, CommandError::OAuth(_)));

        let job = get_job_inner(&state, JobIdInput { job_id: job_id_str })
            .await
            .unwrap();
        assert_eq!(job.status, "DRAFT");
    }

    #[tokio::test]
    async fn token_error_on_crash_scanning_persists_paused() {
        let (base_url, _) = spawn_http_handler(|_| ("200 OK".into(), r#"{"files":[]}"#.into()));
        let state = build_state_with_drive(GoogleDriveClient::for_test(base_url).unwrap()).await;
        let (job_id_str, _) = seed_job_ready_to_scan(&state, false).await;
        sqlx::query("UPDATE migration_jobs SET status = 'SCANNING' WHERE id = ?1")
            .bind(&job_id_str)
            .execute(state.account_store.pool())
            .await
            .unwrap();

        let err = start_scan_inner(
            &state,
            JobIdInput {
                job_id: job_id_str.clone(),
            },
        )
        .await
        .expect_err("missing token fails");
        assert!(matches!(err, CommandError::OAuth(_)));

        let job = get_job_inner(
            &state,
            JobIdInput {
                job_id: job_id_str.clone(),
            },
        )
        .await
        .unwrap();
        assert_eq!(job.status, "PAUSED");

        let paused = pause_scan_inner(&state, JobIdInput { job_id: job_id_str })
            .await
            .unwrap();
        assert_eq!(paused.status, "PAUSED");
    }

    #[tokio::test]
    async fn pause_scan_persists_paused_when_no_walker_is_running() {
        let (base_url, _) = spawn_http_handler(|_| ("200 OK".into(), r#"{"files":[]}"#.into()));
        let state = build_state_with_drive(GoogleDriveClient::for_test(base_url).unwrap()).await;
        let (job_id_str, _) = seed_job_ready_to_scan(&state, true).await;
        sqlx::query("UPDATE migration_jobs SET status = 'SCANNING' WHERE id = ?1")
            .bind(&job_id_str)
            .execute(state.account_store.pool())
            .await
            .unwrap();

        let paused = pause_scan_inner(
            &state,
            JobIdInput {
                job_id: job_id_str.clone(),
            },
        )
        .await
        .unwrap();
        assert_eq!(paused.status, "PAUSED");
    }

    #[tokio::test]
    async fn rate_limit_persists_paused_and_start_scan_can_resume() {
        let lists = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let lists_handler = std::sync::Arc::clone(&lists);
        let (base_url, _) = spawn_http_handler(move |request| {
            if crate::test_support::request_path(request)
                .is_some_and(|path| path.split('?').next() == Some("/drive/v3/files/root-1"))
            {
                return ("200 OK".into(), source_folder("root-1", "Root"));
            }
            if request_is_quota(request) {
                return (
                    "200 OK".into(),
                    r#"{"storageQuota":{"limit":"10000","usage":"1"}}"#.into(),
                );
            }
            if request_is_list(request) {
                let n = lists_handler.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                if n == 1 {
                    return (
                        "429 Too Many Requests".into(),
                        r#"{"error":{"code":429}}"#.into(),
                    );
                }
                return (
                    "200 OK".into(),
                    format!(r#"{{"files":[{}]}}"#, source_file("resumed", "Resumed", 2)),
                );
            }
            ("404 Not Found".into(), "{}".into())
        });

        let state = build_state_with_drive(GoogleDriveClient::for_test(base_url).unwrap()).await;
        let (job_id_str, _) = seed_job_ready_to_scan(&state, true).await;

        let started = start_scan_inner(
            &state,
            JobIdInput {
                job_id: job_id_str.clone(),
            },
        )
        .await
        .expect("scan starts");
        assert_eq!(started.status, "SCANNING");
        await_job_idle(&state, &job_id_str).await;

        let after_429 = get_job_inner(
            &state,
            JobIdInput {
                job_id: job_id_str.clone(),
            },
        )
        .await
        .unwrap();
        assert_eq!(after_429.status, "PAUSED");
        assert_ne!(after_429.status, "FAILED");
        assert_ne!(after_429.status, "SCANNING");

        let resumed = start_scan_inner(
            &state,
            JobIdInput {
                job_id: job_id_str.clone(),
            },
        )
        .await
        .unwrap();
        assert_eq!(resumed.status, "SCANNING");
        await_job_idle(&state, &job_id_str).await;
        let resumed = get_job_inner(
            &state,
            JobIdInput {
                job_id: job_id_str.clone(),
            },
        )
        .await
        .unwrap();
        assert_eq!(resumed.status, "READY_FOR_REVIEW");
        let items = list_job_items_inner(
            &state,
            ListJobItemsInput {
                job_id: job_id_str,
                filter: Some("eligible".into()),
                page: Some(1),
            },
        )
        .await
        .unwrap();
        assert!(items.items.iter().any(|item| item.file_id == "resumed"));
    }

    #[tokio::test]
    async fn unavailable_list_error_persists_paused_not_failed() {
        let (base_url, _) = spawn_http_handler(move |request| {
            if crate::test_support::request_path(request)
                .is_some_and(|path| path.split('?').next() == Some("/drive/v3/files/root-1"))
            {
                return ("200 OK".into(), source_folder("root-1", "Root"));
            }
            if request_is_list(request) {
                return ("503 Service Unavailable".into(), "{}".into());
            }
            (
                "200 OK".into(),
                r#"{"storageQuota":{"limit":"1","usage":"0"}}"#.into(),
            )
        });
        let state = build_state_with_drive(GoogleDriveClient::for_test(base_url).unwrap()).await;
        let (job_id_str, _) = seed_job_ready_to_scan(&state, true).await;

        let started = start_scan_inner(
            &state,
            JobIdInput {
                job_id: job_id_str.clone(),
            },
        )
        .await
        .expect("scan starts");
        assert_eq!(started.status, "SCANNING");
        await_job_idle(&state, &job_id_str).await;

        let job = get_job_inner(&state, JobIdInput { job_id: job_id_str })
            .await
            .unwrap();
        assert_eq!(job.status, "PAUSED");
        assert_ne!(job.status, "FAILED");
    }

    #[tokio::test]
    async fn pause_resume_cancel_retry_and_queue_command_inners_persist_state() {
        let (base_url, captured) = spawn_http_handler(|request| {
            if crate::test_support::request_method(request) == Some("GET") {
                let target_perm = if crate::test_support::request_path(request)
                    .is_some_and(|path| path.contains("queued-b"))
                {
                    "perm-target-b"
                } else {
                    TARGET_PERM
                };
                return (
                    "200 OK".into(),
                    format!(
                        r#"{{"id":"owned","name":"owned","mimeType":"text/plain","trashed":false,"parents":["parent"],"owners":[{{"permissionId":"{target_perm}","emailAddress":"target@gmail.com"}}],"permissions":[{{"id":"{target_perm}","type":"user","role":"owner","emailAddress":"target@gmail.com","pendingOwner":false}}]}}"#
                    ),
                );
            }
            ("404 Not Found".into(), "{}".into())
        });
        let state = build_state_with_drive(GoogleDriveClient::for_test(base_url).unwrap()).await;
        create_dummy_account(
            &state.account_store,
            1,
            "source@gmail.com",
            "Source User",
            SOURCE_PERM,
        )
        .await;
        create_dummy_account(
            &state.account_store,
            2,
            "target@gmail.com",
            "Target User",
            TARGET_PERM,
        )
        .await;
        create_dummy_account(
            &state.account_store,
            3,
            "source-b@gmail.com",
            "Source B",
            "perm-source-b",
        )
        .await;
        create_dummy_account(
            &state.account_store,
            4,
            "target-b@gmail.com",
            "Target B",
            "perm-target-b",
        )
        .await;
        state
            .token_provider
            .insert_cached_token_for_test(AccountId::new(1), AccessToken::new(SOURCE_TOKEN.into()))
            .await;
        state
            .token_provider
            .insert_cached_token_for_test(AccountId::new(2), AccessToken::new(TARGET_TOKEN.into()))
            .await;
        state
            .token_provider
            .insert_cached_token_for_test(AccountId::new(3), AccessToken::new(SOURCE_TOKEN.into()))
            .await;
        state
            .token_provider
            .insert_cached_token_for_test(AccountId::new(4), AccessToken::new(TARGET_TOKEN.into()))
            .await;

        let job_a = create_job_inner(
            &state,
            CreateJobInput {
                source_account_id: "1".into(),
                target_account_id: "2".into(),
            },
        )
        .await
        .unwrap();
        let job_b = create_job_inner(
            &state,
            CreateJobInput {
                source_account_id: "3".into(),
                target_account_id: "4".into(),
            },
        )
        .await
        .unwrap();

        sqlx::query("UPDATE migration_jobs SET status = 'READY_FOR_REVIEW' WHERE id = ?1")
            .bind(&job_a.id)
            .execute(state.account_store.pool())
            .await
            .unwrap();
        sqlx::query("UPDATE migration_jobs SET status = 'READY_FOR_REVIEW' WHERE id = ?1")
            .bind(&job_b.id)
            .execute(state.account_store.pool())
            .await
            .unwrap();

        let queued_a = queue_job_inner(
            &state,
            QueueJobInput {
                job_id: job_a.id.clone(),
                position: None,
            },
        )
        .await
        .unwrap();
        let queued_b = queue_job_inner(
            &state,
            QueueJobInput {
                job_id: job_b.id.clone(),
                position: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(queued_a.status, "QUEUED");
        assert_eq!(queued_a.queue_position, Some(1));
        assert_eq!(queued_b.queue_position, Some(2));
        let reordered = queue_job_inner(
            &state,
            QueueJobInput {
                job_id: job_b.id.clone(),
                position: Some(1),
            },
        )
        .await
        .unwrap();
        assert_eq!(reordered.queue_position, Some(1));
        assert_eq!(reordered.status, "QUEUED");

        sqlx::query("UPDATE migration_jobs SET status = 'RUNNING_CANARY', queue_position = NULL WHERE id = ?1")
            .bind(&job_a.id)
            .execute(state.account_store.pool())
            .await
            .unwrap();
        let job_a_id: crate::domain::job::JobId = job_a.id.parse().unwrap();
        state
            .job_store
            .commit_scan_batch(
                job_a_id,
                &ItemBatchCommit {
                    items: vec![
                        crate::domain::item::MigrationItem {
                            id: crate::domain::item::ItemId::new(1),
                            job_id: job_a_id,
                            file_id: "owned".into(),
                            name: "owned".into(),
                            mime_type: "text/plain".into(),
                            depth: 1,
                            original_parent_ids: vec!["parent".into()],
                            original_owner_permission_id: Some(GooglePermissionId::new(
                                SOURCE_PERM,
                            )),
                            quota_bytes_used: Some(1),
                            target_permission_id: None,
                            state: crate::domain::item::ItemState::Eligible,
                            canary_selected: true,
                            created_at: "t".into(),
                            updated_at: "t".into(),
                        },
                        crate::domain::item::MigrationItem {
                            id: crate::domain::item::ItemId::new(2),
                            job_id: job_a_id,
                            file_id: "todo".into(),
                            name: "todo".into(),
                            mime_type: "text/plain".into(),
                            depth: 0,
                            original_parent_ids: vec!["parent".into()],
                            original_owner_permission_id: Some(GooglePermissionId::new(
                                SOURCE_PERM,
                            )),
                            quota_bytes_used: Some(1),
                            target_permission_id: None,
                            state: crate::domain::item::ItemState::Eligible,
                            canary_selected: false,
                            created_at: "t".into(),
                            updated_at: "t".into(),
                        },
                    ],
                    checkpoints_upsert: Vec::new(),
                    checkpoints_delete: Vec::new(),
                },
            )
            .await
            .unwrap();
        sqlx::query("UPDATE migration_items SET state = 'TRANSFERRED' WHERE file_id = 'owned'")
            .execute(state.account_store.pool())
            .await
            .unwrap();
        sqlx::query("UPDATE migration_items SET state = 'RETRYABLE_FAILED' WHERE file_id = 'todo'")
            .execute(state.account_store.pool())
            .await
            .unwrap();

        let paused = pause_migration_inner(
            &state,
            JobIdInput {
                job_id: job_a.id.clone(),
            },
        )
        .await
        .unwrap();
        assert_eq!(paused.status, "PAUSED");

        let retried = retry_failed_items_inner(
            &state,
            JobIdInput {
                job_id: job_a.id.clone(),
            },
        )
        .await
        .unwrap();
        assert_ne!(retried.status, "PAUSED");
        state.job_service.await_idle(job_a_id).await;

        sqlx::query("UPDATE migration_jobs SET status = 'RUNNING' WHERE id = ?1")
            .bind(&job_a.id)
            .execute(state.account_store.pool())
            .await
            .unwrap();
        sqlx::query("UPDATE migration_items SET state = 'ELIGIBLE' WHERE file_id = 'todo'")
            .execute(state.account_store.pool())
            .await
            .unwrap();
        let cancelled = cancel_migration_inner(
            &state,
            JobIdInput {
                job_id: job_a.id.clone(),
            },
        )
        .await
        .unwrap();
        assert_eq!(cancelled.status, "CANCELLED");
        let page = list_job_items_inner(
            &state,
            ListJobItemsInput {
                job_id: job_a.id.clone(),
                filter: None,
                page: Some(1),
            },
        )
        .await
        .unwrap();
        let owned = page
            .items
            .iter()
            .find(|item| item.file_id == "owned")
            .unwrap();
        let todo = page
            .items
            .iter()
            .find(|item| item.file_id == "todo")
            .unwrap();
        assert!(
            owned.state == "TRANSFERRED" || owned.state == "VERIFIED",
            "already transferred items must not be rolled back, got {}",
            owned.state
        );
        assert_eq!(todo.state, "CANCELLED");

        let job_b_id: crate::domain::job::JobId = job_b.id.parse().unwrap();
        state
            .job_store
            .commit_scan_batch(
                job_b_id,
                &ItemBatchCommit {
                    items: vec![crate::domain::item::MigrationItem {
                        id: crate::domain::item::ItemId::new(9),
                        job_id: job_b_id,
                        file_id: "queued-b".into(),
                        name: "queued-b".into(),
                        mime_type: "text/plain".into(),
                        depth: 1,
                        original_parent_ids: vec!["parent".into()],
                        original_owner_permission_id: Some(GooglePermissionId::new(
                            "perm-source-b",
                        )),
                        quota_bytes_used: Some(1),
                        target_permission_id: None,
                        state: crate::domain::item::ItemState::Eligible,
                        canary_selected: false,
                        created_at: "t".into(),
                        updated_at: "t".into(),
                    }],
                    checkpoints_upsert: Vec::new(),
                    checkpoints_delete: Vec::new(),
                },
            )
            .await
            .unwrap();
        let still_queued = get_job_inner(
            &state,
            JobIdInput {
                job_id: job_b.id.clone(),
            },
        )
        .await
        .unwrap();
        assert_eq!(still_queued.status, "QUEUED");
        let started = start_canary_inner(
            &state,
            StartCanaryInput {
                job_id: job_b.id.clone(),
                confirmation: "target-b@gmail.com".into(),
            },
        )
        .await
        .expect("queued job starts after the previous mutation job is no longer running");
        assert_ne!(started.status, "QUEUED");
        await_job_idle(&state, &job_b.id).await;
        let lease = state.job_store.current_mutation_lease().await.unwrap();
        assert!(
            lease.is_none(),
            "durable lease must be released after start_canary"
        );
        let requests = captured.lock().unwrap().clone();
        let mutations = requests.iter().filter(|request| {
            matches!(
                crate::test_support::request_method(request),
                Some("POST" | "PATCH" | "PUT")
            )
        });
        assert_eq!(mutations.count(), 0);
    }
}
