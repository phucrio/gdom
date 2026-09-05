#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use tokio::sync::RwLock;

    use crate::application::{
        AccountLifecycleUseCase, AccountTokenProvider, ConnectAccountUseCase, JobService,
    };
    use crate::commands::dto::{CreateJobInput, JobIdInput, UpdateDraftJobAccountsInput};
    use crate::commands::error::CommandError;
    use crate::commands::job::{
        create_job_inner, get_job_inner, list_jobs_inner, update_draft_job_accounts_inner,
    };
    use crate::domain::{AccountId, ConnectedAccount, GooglePermissionId};
    use crate::infrastructure::SqliteJobStore;
    use crate::infrastructure::account_store::SqliteAccountStore;
    use crate::infrastructure::google_drive::GoogleDriveClient;
    use crate::infrastructure::google_token::DynamicGoogleTokenClient;
    use crate::infrastructure::secrets::WindowsCredentialStore;
    use crate::state::{AppState, OAuthConfig};

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
            Arc::new(drive_client),
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
            job_service,
        )
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
        )
        .unwrap();
        store.connect(&acc).await.unwrap()
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
}
