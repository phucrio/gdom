use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::application::AccessToken;
use crate::application::account_token_provider::AccountTokenProvider;
use crate::application::backoff::{Sleeper, ZeroJitter};
use crate::application::item_store::{ItemBatchCommit, ItemStorePort};
use crate::application::job_service::JobService;
use crate::application::job_store::JobStorePort;
use crate::application::time::iso_now;
use crate::domain::item::{ItemId, ItemState, MigrationItem};
use crate::domain::job::{
    AccountSnapshot, JobId, JobStatus, MigrationJob, MigrationRoot, RootId, RootValidationStatus,
};
use crate::domain::{AccountId, GooglePermissionId};
use crate::infrastructure::SqliteJobStore;
use crate::infrastructure::account_store::SqliteAccountStore;
use crate::infrastructure::google_drive::GoogleDriveClient;
use crate::infrastructure::google_token::DynamicGoogleTokenClient;
use crate::infrastructure::secrets::WindowsCredentialStore;
use crate::state::OAuthConfig;
use crate::test_support::{
    SOURCE_PERM, SOURCE_TOKEN, TARGET_PERM, TARGET_TOKEN, request_method, request_path,
    spawn_http_handler,
};
use tokio::sync::RwLock;

struct RecordingSleeper {
    delays: Mutex<Vec<Duration>>,
}

impl RecordingSleeper {
    fn new() -> Self {
        Self {
            delays: Mutex::new(Vec::new()),
        }
    }

    fn delays(&self) -> Vec<Duration> {
        self.delays
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

impl Sleeper for RecordingSleeper {
    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        self.delays
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(duration);
        Box::pin(async {})
    }
}

fn snapshot(id: u128, email: &str, perm: &str) -> AccountSnapshot {
    AccountSnapshot {
        account_id: AccountId::new(id),
        email: email.to_string(),
        display_name: format!("User {id}"),
        permission_id: GooglePermissionId::new(perm),
    }
}

fn eligible_item(job_id: JobId, id: u128, file_id: &str, depth: i64) -> MigrationItem {
    MigrationItem {
        id: ItemId::new(id),
        job_id,
        file_id: file_id.into(),
        name: file_id.into(),
        mime_type: "text/plain".into(),
        depth,
        original_parent_ids: vec!["parent".into()],
        original_owner_permission_id: Some(GooglePermissionId::new(SOURCE_PERM)),
        quota_bytes_used: Some(1),
        target_permission_id: None,
        state: ItemState::Eligible,
        canary_selected: false,
        created_at: "t".into(),
        updated_at: "t".into(),
    }
}

fn file_json(file_id: &str, pending: bool, transferred: bool) -> String {
    let owner = if transferred {
        TARGET_PERM
    } else {
        SOURCE_PERM
    };
    let owner_email = if transferred {
        "target@gmail.com"
    } else {
        "source@gmail.com"
    };
    let permissions = if transferred {
        format!(
            r#"[{{"id":"{TARGET_PERM}","type":"user","role":"owner","emailAddress":"target@gmail.com","pendingOwner":false}}]"#
        )
    } else if pending {
        format!(
            r#"[{{"id":"{TARGET_PERM}","type":"user","role":"writer","emailAddress":"target@gmail.com","pendingOwner":true}}]"#
        )
    } else {
        "[]".to_string()
    };
    format!(
        r#"{{"id":"{file_id}","name":"{file_id}","mimeType":"text/plain","trashed":false,"parents":["parent"],"owners":[{{"permissionId":"{owner}","emailAddress":"{owner_email}"}}],"permissions":{permissions}}}"#
    )
}

struct Env {
    account_store: Arc<SqliteAccountStore>,
    job_store: Arc<SqliteJobStore>,
    job_service: Arc<JobService<SqliteAccountStore, SqliteJobStore>>,
    captured: Arc<Mutex<Vec<String>>>,
    sleeper: Arc<RecordingSleeper>,
    events: Arc<crate::application::RecordingJobEventSink>,
}

async fn seed_accounts(store: &SqliteAccountStore) {
    for (id, email, perm) in [
        (1_u128, "source@gmail.com", SOURCE_PERM),
        (2, "target@gmail.com", TARGET_PERM),
        (3, "source-b@gmail.com", "perm-source-b"),
        (4, "target-b@gmail.com", "perm-target-b"),
    ] {
        let acc = crate::domain::ConnectedAccount::new_personal(
            AccountId::new(id),
            GooglePermissionId::new(perm),
            email,
            format!("User {id}"),
        )
        .unwrap();
        store.connect(&acc).await.unwrap();
    }
}

#[allow(clippy::too_many_arguments)]
async fn seed_ready_job(
    store: &SqliteJobStore,
    job_id: u128,
    source: u128,
    target: u128,
    source_email: &str,
    target_email: &str,
    source_perm: &str,
    target_perm: &str,
) -> MigrationJob {
    let mut job = MigrationJob::new(
        JobId::new(job_id),
        snapshot(source, source_email, source_perm),
        snapshot(target, target_email, target_perm),
        "2026-09-06T00:00:00Z".into(),
    )
    .unwrap();
    job.add_root(MigrationRoot {
        id: RootId::new(job_id * 10),
        job_id: job.id(),
        root_file_id: format!("root-{job_id}"),
        root_name: "Root".into(),
        validation_status: RootValidationStatus::Validated,
        created_at: "t".into(),
    })
    .unwrap();
    store.create_job(&job).await.unwrap();
    store.add_root(&job.roots()[0]).await.unwrap();
    job.start_scanning("2026-09-06T00:01:00Z".into()).unwrap();
    job.complete_scanning().unwrap();
    store.update_job(&job).await.unwrap();
    job
}

async fn build_env<F>(handler: F) -> Env
where
    F: Fn(&str) -> (String, String) + Send + Sync + 'static,
{
    let account_store = Arc::new(SqliteAccountStore::open_in_memory().await.unwrap());
    seed_accounts(&account_store).await;
    let job_store = Arc::new(SqliteJobStore::new(account_store.pool().clone()));
    let cred_store = Arc::new(WindowsCredentialStore::new_mock());
    let oauth_config = Arc::new(RwLock::new(Some(OAuthConfig::new("test-client", None))));
    let token_service = Arc::new(DynamicGoogleTokenClient::new(oauth_config));
    let token_provider = Arc::new(AccountTokenProvider::new(
        token_service,
        cred_store,
        account_store.clone(),
    ));
    token_provider
        .insert_cached_token_for_test(AccountId::new(1), AccessToken::new(SOURCE_TOKEN.into()))
        .await;
    token_provider
        .insert_cached_token_for_test(AccountId::new(2), AccessToken::new(TARGET_TOKEN.into()))
        .await;
    token_provider
        .insert_cached_token_for_test(AccountId::new(3), AccessToken::new(SOURCE_TOKEN.into()))
        .await;
    token_provider
        .insert_cached_token_for_test(AccountId::new(4), AccessToken::new(TARGET_TOKEN.into()))
        .await;

    let (base_url, captured) = spawn_http_handler(handler);
    let drive = GoogleDriveClient::for_test(base_url).unwrap();
    let sleeper = Arc::new(RecordingSleeper::new());
    let events = Arc::new(crate::application::RecordingJobEventSink::new());
    let job_service = Arc::new(
        JobService::new(
            account_store.clone(),
            job_store.clone(),
            Arc::new(drive) as Arc<dyn crate::application::DrivePort>,
            token_provider.clone(),
        )
        .with_sleeper(sleeper.clone(), Arc::new(ZeroJitter))
        .with_event_sink(events.clone()),
    );
    Env {
        account_store,
        job_store,
        job_service,
        captured,
        sleeper,
        events,
    }
}

fn mutation_methods(captured: &Mutex<Vec<String>>) -> Vec<String> {
    captured
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .iter()
        .filter_map(|request| {
            request_method(request)
                .filter(|method| matches!(*method, "POST" | "PATCH" | "PUT"))
                .map(ToOwned::to_owned)
        })
        .collect()
}

#[tokio::test]
async fn schema_v6_creates_lease_and_event_tables() {
    let env = build_env(|_| ("404 Not Found".into(), "{}".into())).await;
    let version: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM _schema_migrations WHERE version = 6")
            .fetch_one(env.account_store.pool())
            .await
            .unwrap();
    assert_eq!(version, 1);
    sqlx::query("SELECT job_id, owner_instance_id, acquired_at, heartbeat_at FROM worker_leases")
        .fetch_all(env.account_store.pool())
        .await
        .unwrap();
    sqlx::query("SELECT id, job_id, event_type, previous_state, new_state FROM migration_events")
        .fetch_all(env.account_store.pool())
        .await
        .unwrap();
}

#[tokio::test]
async fn durable_lease_is_globally_exclusive() {
    let env = build_env(|_| ("404 Not Found".into(), "{}".into())).await;
    let job_a = seed_ready_job(
        &env.job_store,
        10,
        1,
        2,
        "source@gmail.com",
        "target@gmail.com",
        SOURCE_PERM,
        TARGET_PERM,
    )
    .await;
    let job_b = seed_ready_job(
        &env.job_store,
        11,
        3,
        4,
        "source-b@gmail.com",
        "target-b@gmail.com",
        "perm-source-b",
        "perm-target-b",
    )
    .await;

    env.job_store
        .acquire_mutation_lease(job_a.id(), "instance-a", &iso_now())
        .await
        .expect("first lease");
    let second = env
        .job_store
        .acquire_mutation_lease(job_b.id(), "instance-b", &iso_now())
        .await;
    assert!(matches!(
        second,
        Err(crate::application::JobStorePortError::MutationLeaseHeld)
    ));
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM worker_leases")
        .fetch_one(env.account_store.pool())
        .await
        .unwrap();
    assert_eq!(count, 1);
    let holder = env
        .job_store
        .current_mutation_lease()
        .await
        .unwrap()
        .expect("holder");
    assert_eq!(holder.job_id, job_a.id());
}

#[tokio::test]
async fn second_start_canary_queues_without_mutations_or_auto_start() {
    let env = build_env(|request| {
        if request_method(request) == Some("GET") {
            let transferred = request_path(request).is_some_and(|path| path.contains("file-b"));
            return (
                "200 OK".into(),
                file_json("file-b", false, transferred),
            );
        }
        (
            "200 OK".into(),
            format!(
                r#"{{"id":"{TARGET_PERM}","type":"user","role":"writer","emailAddress":"target@gmail.com","pendingOwner":true}}"#
            ),
        )
    })
    .await;
    let job_a = seed_ready_job(
        &env.job_store,
        20,
        1,
        2,
        "source@gmail.com",
        "target@gmail.com",
        SOURCE_PERM,
        TARGET_PERM,
    )
    .await;
    let job_b = seed_ready_job(
        &env.job_store,
        21,
        1,
        2,
        "source@gmail.com",
        "target@gmail.com",
        SOURCE_PERM,
        TARGET_PERM,
    )
    .await;
    env.job_store
        .commit_scan_batch(
            job_b.id(),
            &ItemBatchCommit {
                items: vec![eligible_item(job_b.id(), 201, "file-b", 1)],
                checkpoints_upsert: Vec::new(),
                checkpoints_delete: Vec::new(),
            },
        )
        .await
        .unwrap();

    env.job_store
        .acquire_mutation_lease(job_a.id(), "other-instance", &iso_now())
        .await
        .unwrap();

    let queued = env
        .job_service
        .start_canary(job_b.id(), "target@gmail.com")
        .await
        .expect("queued behind the lease holder");
    assert_eq!(queued.status(), JobStatus::Queued);
    assert_eq!(queued.queue_position(), Some(1));
    assert!(mutation_methods(&env.captured).is_empty());

    let leaked = env
        .job_service
        .continue_migration(job_b.id())
        .await
        .expect_err("queued canary job cannot skip review");
    assert!(matches!(
        leaked,
        crate::application::JobServiceError::IllegalTransition
    ));
    assert!(
        env.job_store
            .current_mutation_lease()
            .await
            .unwrap()
            .is_some_and(|lease| lease.job_id == job_a.id()),
        "illegal continue must not steal or leak onto a second lease row"
    );

    env.job_store
        .release_mutation_lease(job_a.id(), "other-instance")
        .await
        .unwrap();
    let still = env.job_service.get_job(job_b.id()).await.unwrap();
    assert_eq!(still.status(), JobStatus::Queued);
    assert!(
        env.job_store
            .current_mutation_lease()
            .await
            .unwrap()
            .is_none()
    );

    let started = env
        .job_service
        .start_canary(job_b.id(), "target@gmail.com")
        .await
        .expect("explicit start after the previous holder released");
    assert_ne!(started.status(), JobStatus::Queued);
    env.job_service.await_idle(job_b.id()).await;
    assert!(
        env.job_store
            .current_mutation_lease()
            .await
            .unwrap()
            .is_none(),
        "completed start_canary must release the durable lease"
    );
}

#[tokio::test]
async fn resume_queued_job_after_holder_releases_does_not_stick_lease() {
    let env = build_env(|request| {
        if request_method(request) == Some("GET") {
            let transferred = request_path(request).is_some_and(|path| path.contains("file-b"));
            return ("200 OK".into(), file_json("file-b", false, transferred));
        }
        (
            "200 OK".into(),
            format!(
                r#"{{"id":"{TARGET_PERM}","type":"user","role":"writer","emailAddress":"target@gmail.com","pendingOwner":true}}"#
            ),
        )
    })
    .await;
    let job_a = seed_ready_job(
        &env.job_store,
        22,
        1,
        2,
        "source@gmail.com",
        "target@gmail.com",
        SOURCE_PERM,
        TARGET_PERM,
    )
    .await;
    let job_b = seed_ready_job(
        &env.job_store,
        23,
        1,
        2,
        "source@gmail.com",
        "target@gmail.com",
        SOURCE_PERM,
        TARGET_PERM,
    )
    .await;
    env.job_store
        .commit_scan_batch(
            job_b.id(),
            &ItemBatchCommit {
                items: vec![eligible_item(job_b.id(), 231, "file-b", 1)],
                checkpoints_upsert: Vec::new(),
                checkpoints_delete: Vec::new(),
            },
        )
        .await
        .unwrap();
    env.job_store
        .acquire_mutation_lease(job_a.id(), "other-instance", &iso_now())
        .await
        .unwrap();
    env.job_service
        .start_canary(job_b.id(), "target@gmail.com")
        .await
        .expect("queued");
    env.job_store
        .release_mutation_lease(job_a.id(), "other-instance")
        .await
        .unwrap();

    let resumed = env
        .job_service
        .resume_migration(job_b.id())
        .await
        .expect("explicit resume of queued job after holder released");
    assert_ne!(resumed.status(), JobStatus::Queued);
    env.job_service.await_idle(job_b.id()).await;
    assert!(
        env.job_store
            .current_mutation_lease()
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn queue_job_reorders_without_starting_mutations() {
    let env = build_env(|_| ("404 Not Found".into(), "{}".into())).await;
    let job_a = seed_ready_job(
        &env.job_store,
        30,
        1,
        2,
        "source@gmail.com",
        "target@gmail.com",
        SOURCE_PERM,
        TARGET_PERM,
    )
    .await;
    let job_b = seed_ready_job(
        &env.job_store,
        31,
        3,
        4,
        "source-b@gmail.com",
        "target-b@gmail.com",
        "perm-source-b",
        "perm-target-b",
    )
    .await;

    let first = env.job_service.queue_job(job_a.id(), None).await.unwrap();
    let second = env.job_service.queue_job(job_b.id(), None).await.unwrap();
    assert_eq!(first.status(), JobStatus::Queued);
    assert_eq!(first.queue_position(), Some(1));
    assert_eq!(second.queue_position(), Some(2));

    let reordered = env
        .job_service
        .queue_job(job_b.id(), Some(1))
        .await
        .unwrap();
    assert_eq!(reordered.id(), job_b.id());
    assert_eq!(reordered.queue_position(), Some(1));
    let job_a_after = env.job_service.get_job(job_a.id()).await.unwrap();
    assert_eq!(job_a_after.queue_position(), Some(2));
    assert_eq!(job_a_after.status(), JobStatus::Queued);
    assert_eq!(reordered.status(), JobStatus::Queued);
    assert!(mutation_methods(&env.captured).is_empty());
}

#[tokio::test]
async fn startup_reconcile_pauses_unfinished_jobs_without_mutations() {
    let env = build_env(|request| {
        if request_method(request) == Some("GET") {
            return ("200 OK".into(), file_json("mid-file", true, false));
        }
        panic!("startup reconcile must not mutate ownership: {request}");
    })
    .await;
    let mut job = seed_ready_job(
        &env.job_store,
        40,
        1,
        2,
        "source@gmail.com",
        "target@gmail.com",
        SOURCE_PERM,
        TARGET_PERM,
    )
    .await;
    job.start_canary().unwrap();
    env.job_store.update_job(&job).await.unwrap();
    env.job_store
        .commit_scan_batch(
            job.id(),
            &ItemBatchCommit {
                items: vec![eligible_item(job.id(), 401, "mid-file", 1)],
                checkpoints_upsert: Vec::new(),
                checkpoints_delete: Vec::new(),
            },
        )
        .await
        .unwrap();
    sqlx::query("UPDATE migration_items SET state = 'PENDING_OWNER_CREATED', target_permission_id = ?1 WHERE file_id = 'mid-file'")
        .bind(TARGET_PERM)
        .execute(env.account_store.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE migration_jobs SET status = 'RUNNING' WHERE id = ?1")
        .bind(job.id().value().to_string())
        .execute(env.account_store.pool())
        .await
        .unwrap();
    env.job_store
        .acquire_mutation_lease(job.id(), "stale-instance", &iso_now())
        .await
        .unwrap();

    env.job_service.reconcile_on_startup().await.unwrap();

    let paused = env.job_service.get_job(job.id()).await.unwrap();
    assert_eq!(paused.status(), JobStatus::Paused);
    assert!(
        env.job_store
            .current_mutation_lease()
            .await
            .unwrap()
            .is_none()
    );
    assert!(mutation_methods(&env.captured).is_empty());
    let items = env
        .job_store
        .list_items_page(job.id(), None, 1, 50)
        .await
        .unwrap()
        .items;
    assert_eq!(items[0].state, ItemState::AcceptRequired);
}

#[tokio::test]
async fn cancel_leaves_transferred_items_and_cancels_unstarted() {
    let env = build_env(|_| ("404 Not Found".into(), "{}".into())).await;
    let mut job = seed_ready_job(
        &env.job_store,
        50,
        1,
        2,
        "source@gmail.com",
        "target@gmail.com",
        SOURCE_PERM,
        TARGET_PERM,
    )
    .await;
    job.start_canary().unwrap();
    job.complete_canary().unwrap();
    job.start_bulk().unwrap();
    env.job_store.update_job(&job).await.unwrap();
    env.job_store
        .commit_scan_batch(
            job.id(),
            &ItemBatchCommit {
                items: vec![
                    eligible_item(job.id(), 501, "done-file", 2),
                    eligible_item(job.id(), 502, "todo-file", 1),
                ],
                checkpoints_upsert: Vec::new(),
                checkpoints_delete: Vec::new(),
            },
        )
        .await
        .unwrap();
    sqlx::query("UPDATE migration_items SET state = 'TRANSFERRED' WHERE file_id = 'done-file'")
        .execute(env.account_store.pool())
        .await
        .unwrap();

    let cancelled = env.job_service.cancel_migration(job.id()).await.unwrap();
    assert_eq!(cancelled.status(), JobStatus::Cancelled);
    let items = env
        .job_store
        .list_items_page(job.id(), None, 1, 50)
        .await
        .unwrap()
        .items;
    let done = items
        .iter()
        .find(|item| item.file_id == "done-file")
        .unwrap();
    let todo = items
        .iter()
        .find(|item| item.file_id == "todo-file")
        .unwrap();
    assert_eq!(done.state, ItemState::Transferred);
    assert_eq!(todo.state, ItemState::Cancelled);
}

#[tokio::test]
async fn sharing_rate_limit_persists_without_fast_retry_and_releases_lease() {
    let env = build_env(|request| {
        if request_method(request) == Some("GET") {
            return ("200 OK".into(), file_json("limited-file", false, false));
        }
        (
            "403 Forbidden".into(),
            r#"{"error":{"errors":[{"reason":"sharingRateLimitExceeded"}]}}"#.into(),
        )
    })
    .await;
    let job = seed_ready_job(
        &env.job_store,
        60,
        1,
        2,
        "source@gmail.com",
        "target@gmail.com",
        SOURCE_PERM,
        TARGET_PERM,
    )
    .await;
    env.job_store
        .commit_scan_batch(
            job.id(),
            &ItemBatchCommit {
                items: vec![eligible_item(job.id(), 601, "limited-file", 1)],
                checkpoints_upsert: Vec::new(),
                checkpoints_delete: Vec::new(),
            },
        )
        .await
        .unwrap();

    let started = env
        .job_service
        .start_canary(job.id(), "target@gmail.com")
        .await
        .expect("rate limit is a persisted halt, not a command failure");
    assert_ne!(started.status(), JobStatus::Queued);
    env.job_service.await_idle(job.id()).await;
    let halted = env.job_service.get_job(job.id()).await.unwrap();
    assert_eq!(halted.status(), JobStatus::SourceRateLimited);
    let events = env.events.snapshot();
    assert!(events.iter().any(|event| matches!(
        event,
        crate::application::JobRuntimeEvent::JobStatusChanged { status, .. }
            if status == JobStatus::RunningCanary.as_str()
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        crate::application::JobRuntimeEvent::JobStatusChanged { status, .. }
            if status == JobStatus::SourceRateLimited.as_str()
    )));
    assert!(env.sleeper.delays().is_empty());
    assert!(
        env.job_store
            .current_mutation_lease()
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn pause_and_retry_drive_real_job_service_entry_points() {
    let env = build_env(|request| {
        if request_method(request) == Some("GET") {
            return ("200 OK".into(), file_json("retry-file", false, true));
        }
        panic!("retry after pause should verify already-owned files without mutations: {request}");
    })
    .await;
    let mut job = seed_ready_job(
        &env.job_store,
        70,
        1,
        2,
        "source@gmail.com",
        "target@gmail.com",
        SOURCE_PERM,
        TARGET_PERM,
    )
    .await;
    job.start_canary().unwrap();
    env.job_store.update_job(&job).await.unwrap();
    env.job_store
        .commit_scan_batch(
            job.id(),
            &ItemBatchCommit {
                items: vec![eligible_item(job.id(), 701, "retry-file", 1)],
                checkpoints_upsert: Vec::new(),
                checkpoints_delete: Vec::new(),
            },
        )
        .await
        .unwrap();
    sqlx::query(
        "UPDATE migration_items SET state = 'RETRYABLE_FAILED' WHERE file_id = 'retry-file'",
    )
    .execute(env.account_store.pool())
    .await
    .unwrap();

    let paused = env.job_service.pause_migration(job.id()).await.unwrap();
    assert_eq!(paused.status(), JobStatus::Paused);

    let retried = env
        .job_service
        .retry_failed_items(job.id())
        .await
        .expect("explicit retry resumes from the checkpoint");
    assert_ne!(retried.status(), JobStatus::Paused);
    env.job_service.await_idle(job.id()).await;
    assert!(mutation_methods(&env.captured).is_empty());
}
