use super::*;
use crate::application::JobServiceError;

async fn ready_environment() -> (Env, MigrationJob) {
    let env = build_env(|_| ("500 Internal Server Error".into(), "{}".into())).await;
    let job = seed_ready_job(
        &env.job_store,
        901,
        1,
        2,
        "source@gmail.com",
        "target@gmail.com",
        SOURCE_PERM,
        TARGET_PERM,
    )
    .await;
    (env, job)
}

#[tokio::test]
async fn installation_blocks_every_job_entry_point_across_clones() {
    let (env, job) = ready_environment().await;
    let installation = env
        .job_service
        .try_begin_update_installation()
        .await
        .expect("safe installation");
    let service = env.job_service.as_ref().clone();

    let rejected = [
        service.start_scan(job.id()).await,
        service.start_canary(job.id(), "target@gmail.com").await,
        service.continue_migration(job.id()).await,
        service.resume_migration(job.id()).await,
        service.queue_job(job.id(), None).await,
        service
            .start_transfer_operation(
                AccountId::new(1),
                AccountId::new(2),
                vec!["root".into()],
                true,
            )
            .await,
    ];
    for result in rejected {
        assert_eq!(
            result.err(),
            Some(JobServiceError::UpdateInstallationInProgress)
        );
    }
    assert_eq!(
        service.run_auto_mutation_if_ready(job.id()).await,
        Err(JobServiceError::UpdateInstallationInProgress)
    );
    assert!(env.captured.lock().expect("captured requests").is_empty());
    assert_eq!(
        env.job_store
            .find_job_by_id(job.id())
            .await
            .unwrap()
            .unwrap()
            .status(),
        JobStatus::ReadyForReview
    );

    drop(installation);
    assert!(service.queue_job(job.id(), None).await.is_ok());
    assert!(service.try_begin_update_installation().await.is_ok());
}

#[tokio::test]
async fn installation_defers_when_persisted_worker_state_is_busy() {
    let (env, mut job) = ready_environment().await;
    job.start_canary().expect("canary status");
    env.job_store
        .update_job(&job)
        .await
        .expect("persist running");

    assert!(matches!(
        env.job_service.try_begin_update_installation().await,
        Err(JobServiceError::UpdateInstallationBusy)
    ));

    job.pause_transfer().expect("paused status");
    env.job_store
        .update_job(&job)
        .await
        .expect("persist paused");
    assert!(
        env.job_service
            .try_begin_update_installation()
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn installation_defers_until_durable_lease_is_released() {
    let (env, job) = ready_environment().await;
    env.job_store
        .acquire_mutation_lease(job.id(), "test-worker", "2026-09-08T00:00:00Z")
        .await
        .expect("durable lease");

    assert!(matches!(
        env.job_service.try_begin_update_installation().await,
        Err(JobServiceError::UpdateInstallationBusy)
    ));

    env.job_store
        .release_mutation_lease(job.id(), "test-worker")
        .await
        .expect("release durable lease");
    assert!(
        env.job_service
            .try_begin_update_installation()
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn installation_fails_closed_when_persistence_cannot_be_read() {
    let (env, _) = ready_environment().await;
    env.account_store.pool().close().await;

    assert!(matches!(
        env.job_service.try_begin_update_installation().await,
        Err(JobServiceError::StoreError(_))
    ));
}

#[tokio::test]
async fn installation_rejects_draft_changes_before_database_or_network_access() {
    let (env, _) = ready_environment().await;
    let draft = env
        .job_service
        .create_job(AccountId::new(1), AccountId::new(2))
        .await
        .expect("draft");
    let installation = env
        .job_service
        .try_begin_update_installation()
        .await
        .expect("installation");

    // A closed pool makes an accidental database access distinguishable from gate rejection.
    env.account_store.pool().close().await;
    let service = env.job_service.as_ref().clone();
    let rejected = [
        service
            .create_job(AccountId::new(1), AccountId::new(2))
            .await
            .err(),
        service
            .update_draft_job_accounts(draft.id(), AccountId::new(3), AccountId::new(4))
            .await
            .err(),
        service.delete_draft_job(draft.id()).await.err(),
        service.validate_root(draft.id(), "root-folder").await.err(),
        service.add_root(draft.id(), "root-folder").await.err(),
        service.remove_root(draft.id(), RootId::new(1)).await.err(),
    ];
    for error in rejected {
        assert_eq!(error, Some(JobServiceError::UpdateInstallationInProgress));
    }
    assert!(env.captured.lock().expect("captured requests").is_empty());
    drop(installation);
}
