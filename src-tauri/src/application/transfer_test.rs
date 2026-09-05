use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::application::AccessToken;
use crate::application::backoff::{Sleeper, ZeroJitter};
use crate::application::item_store::ItemStorePort;
use crate::application::job_store::JobStorePort;
use crate::application::transfer::{TransferHalt, TransferRun, execute_bulk, execute_canary};
use crate::domain::item::{ItemId, ItemState, MigrationItem};
use crate::domain::job::{
    AccountSnapshot, JobId, JobStatus, MigrationJob, MigrationRoot, RootId, RootValidationStatus,
};
use crate::domain::{AccountId, GooglePermissionId};
use crate::infrastructure::SqliteJobStore;
use crate::infrastructure::account_store::SqliteAccountStore;
use crate::infrastructure::google_drive::GoogleDriveClient;
use crate::test_support::{
    SOURCE_PERM, SOURCE_TOKEN, TARGET_PERM, TARGET_TOKEN, authorization_bearer, query_param,
    request_body, request_method, request_path, spawn_http_handler,
};

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

struct DriveScript {
    transferred: Mutex<HashSet<String>>,
    already_owned: Mutex<HashSet<String>>,
    writer_without_pending: Mutex<HashSet<String>>,
    trash_on_target_get: Mutex<HashSet<String>>,
    remaining_failures: AtomicUsize,
    fail_status: Mutex<String>,
    fail_body: Mutex<String>,
    fail_on_verify_only: Mutex<bool>,
}

impl DriveScript {
    fn new() -> Self {
        Self {
            transferred: Mutex::new(HashSet::new()),
            already_owned: Mutex::new(HashSet::new()),
            writer_without_pending: Mutex::new(HashSet::new()),
            trash_on_target_get: Mutex::new(HashSet::new()),
            remaining_failures: AtomicUsize::new(0),
            fail_status: Mutex::new("429 Too Many Requests".into()),
            fail_body: Mutex::new("{}".into()),
            fail_on_verify_only: Mutex::new(false),
        }
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

fn file_json(
    file_id: &str,
    owner: &str,
    transferred: bool,
    writer_without_pending: bool,
    trashed: bool,
) -> String {
    let owner_email = if owner == TARGET_PERM {
        "target@gmail.com"
    } else {
        "source@gmail.com"
    };
    let permissions = if transferred {
        format!(
            r#"[{{"id":"{TARGET_PERM}","type":"user","role":"owner","emailAddress":"target@gmail.com","pendingOwner":false}}]"#
        )
    } else if writer_without_pending {
        format!(
            r#"[{{"id":"{TARGET_PERM}","type":"user","role":"writer","emailAddress":"target@gmail.com","pendingOwner":false}}]"#
        )
    } else {
        "[]".to_string()
    };
    format!(
        r#"{{"id":"{file_id}","name":"{file_id}","mimeType":"text/plain","trashed":{trashed},"parents":["parent"],"owners":[{{"permissionId":"{owner}","emailAddress":"{owner_email}"}}],"permissions":{permissions}}}"#
    )
}

fn path_file_id(path: &str) -> Option<&str> {
    let rest = path.strip_prefix("/drive/v3/files/")?;
    rest.split(['/', '?']).next()
}

fn is_permissions_request(path: &str) -> bool {
    path.contains("/permissions")
}

fn handle_drive(script: &DriveScript, request: &str) -> (String, String) {
    let method = request_method(request).unwrap_or("");
    let path = request_path(request).unwrap_or("");
    let bearer = authorization_bearer(request).unwrap_or_default();
    let file_id = path_file_id(path).unwrap_or("missing").to_string();
    let verify_only = *script
        .fail_on_verify_only
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let is_verify_get =
        method == "GET" && !is_permissions_request(path) && bearer.contains(TARGET_TOKEN);
    let should_fail = if verify_only { is_verify_get } else { true };
    if should_fail && script.remaining_failures.load(Ordering::SeqCst) > 0 {
        script.remaining_failures.fetch_sub(1, Ordering::SeqCst);
        let status = script
            .fail_status
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        let body = script
            .fail_body
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        return (status, body);
    }

    if method == "GET" && !is_permissions_request(path) {
        let already_owned = script
            .already_owned
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .contains(&file_id);
        let transferred = already_owned
            || script
                .transferred
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .contains(&file_id);
        let writer_without_pending = script
            .writer_without_pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .contains(&file_id);
        let trash_on_target = is_verify_get
            && script
                .trash_on_target_get
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .contains(&file_id);
        let owner = if transferred {
            TARGET_PERM
        } else {
            SOURCE_PERM
        };
        return (
            "200 OK".into(),
            file_json(
                &file_id,
                owner,
                transferred,
                writer_without_pending && !transferred,
                trash_on_target,
            ),
        );
    }

    if method == "POST" && is_permissions_request(path) {
        return (
            "200 OK".into(),
            format!(
                r#"{{"id":"{TARGET_PERM}","type":"user","role":"writer","emailAddress":"target@gmail.com","pendingOwner":true}}"#
            ),
        );
    }

    if method == "PATCH" && is_permissions_request(path) {
        if query_param(request, "transferOwnership").as_deref() == Some("true") {
            script
                .transferred
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .insert(file_id);
            return (
                "200 OK".into(),
                format!(
                    r#"{{"id":"{TARGET_PERM}","type":"user","role":"owner","emailAddress":"target@gmail.com","pendingOwner":false}}"#
                ),
            );
        }
        return (
            "200 OK".into(),
            format!(
                r#"{{"id":"{TARGET_PERM}","type":"user","role":"writer","emailAddress":"target@gmail.com","pendingOwner":true}}"#
            ),
        );
    }

    ("404 Not Found".into(), "{}".into())
}

async fn seed_job(store: &SqliteJobStore, canary_size: usize) -> MigrationJob {
    sqlx::query(
        "INSERT INTO accounts (id, google_permission_id, email, display_name, auth_status, connected_at, last_authenticated_at, updated_at)
         VALUES ('1', 'perm-source', 'source@gmail.com', 'Source', 'CONNECTED', datetime('now'), datetime('now'), datetime('now')),
                ('2', 'perm-target', 'target@gmail.com', 'Target', 'CONNECTED', datetime('now'), datetime('now'), datetime('now'))",
    )
    .execute(store.pool())
    .await
    .expect("accounts");

    let mut job = MigrationJob::new(
        JobId::new(42),
        snapshot(1, "source@gmail.com", SOURCE_PERM),
        snapshot(2, "target@gmail.com", TARGET_PERM),
        "2026-09-06T00:00:00Z".into(),
    )
    .expect("job");
    job.add_root(MigrationRoot {
        id: RootId::new(1),
        job_id: job.id(),
        root_file_id: "root".into(),
        root_name: "Root".into(),
        validation_status: RootValidationStatus::Validated,
        created_at: "t".into(),
    })
    .expect("root");
    store.create_job(&job).await.expect("create job");
    store.add_root(&job.roots()[0]).await.expect("add root");
    job.start_scanning("2026-09-06T00:01:00Z".into())
        .expect("scan");
    job.complete_scanning().expect("ready");
    let job = MigrationJob::reconstitute(
        job.id(),
        job.accounts(),
        job.snapshots().clone(),
        JobStatus::ReadyForReview,
        None,
        canary_size,
        job.created_at().to_string(),
        job.started_at().map(ToOwned::to_owned),
        None,
        None,
        job.roots().to_vec(),
    );
    store.update_job(&job).await.expect("persist ready job");
    job
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
        created_at: "t".into(),
        updated_at: "t".into(),
    }
}

struct Fixture {
    store: SqliteJobStore,
    job: MigrationJob,
    client: GoogleDriveClient,
    captured: Arc<Mutex<Vec<String>>>,
    script: Arc<DriveScript>,
    sleeper: RecordingSleeper,
    jitter: ZeroJitter,
    source_token: AccessToken,
    target_token: AccessToken,
    source_permission_id: GooglePermissionId,
    target_permission_id: GooglePermissionId,
    target_email: String,
}

async fn setup(canary_size: usize, items: Vec<(&str, i64)>) -> Fixture {
    let accounts = SqliteAccountStore::open_in_memory().await.expect("db");
    let store = SqliteJobStore::new(accounts.pool().clone());
    let job = seed_job(&store, canary_size).await;
    let seeded: Vec<MigrationItem> = items
        .iter()
        .enumerate()
        .map(|(index, (file_id, depth))| {
            eligible_item(job.id(), 100 + index as u128, file_id, *depth)
        })
        .collect();
    store
        .commit_scan_batch(
            job.id(),
            &crate::application::item_store::ItemBatchCommit {
                items: seeded,
                checkpoints_upsert: Vec::new(),
                checkpoints_delete: Vec::new(),
            },
        )
        .await
        .expect("items");

    let script = Arc::new(DriveScript::new());
    let script_handler = Arc::clone(&script);
    let (base_url, captured) =
        spawn_http_handler(move |request| handle_drive(&script_handler, request));
    let client = GoogleDriveClient::for_test(base_url).expect("client");
    Fixture {
        source_permission_id: job.snapshots().source.permission_id.clone(),
        target_permission_id: job.snapshots().target.permission_id.clone(),
        target_email: job.snapshots().target.email.clone(),
        store,
        job,
        client,
        captured,
        script,
        sleeper: RecordingSleeper::new(),
        jitter: ZeroJitter,
        source_token: AccessToken::new(SOURCE_TOKEN.to_string()),
        target_token: AccessToken::new(TARGET_TOKEN.to_string()),
    }
}

fn run_of<'a>(fixture: &'a Fixture) -> TransferRun<'a> {
    TransferRun {
        drive: &fixture.client,
        store: &fixture.store,
        sleeper: &fixture.sleeper,
        jitter: &fixture.jitter,
        source_token: &fixture.source_token,
        target_token: &fixture.target_token,
        source_permission_id: &fixture.source_permission_id,
        target_permission_id: &fixture.target_permission_id,
        target_email: &fixture.target_email,
    }
}

fn captured_requests(fixture: &Fixture) -> Vec<String> {
    fixture
        .captured
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

fn mutation_requests(requests: &[String]) -> Vec<&String> {
    requests
        .iter()
        .filter(|request| matches!(request_method(request), Some("POST" | "PATCH" | "PUT")))
        .collect()
}

#[tokio::test]
async fn transfer_routes_tokens_preserves_parents_and_follows_depth_desc() {
    let fixture = setup(
        5,
        vec![
            ("root-folder", 0),
            ("child-folder", 1),
            ("deep-file", 3),
            ("mid-file", 2),
        ],
    )
    .await;
    let mut job = fixture.job.clone();
    let run = run_of(&fixture);
    let halt = execute_canary(&run, &mut job).await.expect("canary");
    assert!(matches!(halt, TransferHalt::Exhausted { verified: 4, .. }));
    assert_eq!(job.status(), JobStatus::CanaryReview);

    let requests = captured_requests(&fixture);
    let pending = requests.iter().filter(|request| {
        request_method(request) == Some("POST")
            && request_path(request).is_some_and(|path| path.contains("/permissions"))
    });
    for request in pending {
        assert_eq!(
            authorization_bearer(request),
            Some(format!("Bearer {SOURCE_TOKEN}"))
        );
        assert_eq!(
            query_param(request, "sendNotificationEmail").as_deref(),
            Some("true")
        );
        let body = request_body(request);
        assert!(body.contains("\"pendingOwner\":true"));
        assert!(!body.contains("moveToNewOwnersRoot"));
        assert!(!body.contains("addParents"));
        assert!(!body.contains("removeParents"));
        assert!(!body.contains("parents"));
    }

    let accepts = requests.iter().filter(|request| {
        request_method(request) == Some("PATCH")
            && query_param(request, "transferOwnership").as_deref() == Some("true")
    });
    for request in accepts {
        assert_eq!(
            authorization_bearer(request),
            Some(format!("Bearer {TARGET_TOKEN}"))
        );
        let body = request_body(request);
        assert!(body.contains("\"role\":\"owner\""));
        assert!(!body.contains("moveToNewOwnersRoot"));
        assert!(!body.contains("parents"));
    }

    let verify_gets: Vec<&String> = requests
        .iter()
        .filter(|request| {
            request_method(request) == Some("GET")
                && !request_path(request).is_some_and(is_permissions_request)
                && authorization_bearer(request) == Some(format!("Bearer {TARGET_TOKEN}"))
        })
        .collect();
    assert!(!verify_gets.is_empty());

    for request in mutation_requests(&requests) {
        assert!(!request.contains("moveToNewOwnersRoot=true"));
        assert!(!request.contains("addParents"));
        assert!(!request.contains("removeParents"));
    }

    let pending_order: Vec<String> = requests
        .iter()
        .filter(|request| request_method(request) == Some("POST"))
        .filter_map(|request| {
            path_file_id(request_path(request).unwrap_or("")).map(ToOwned::to_owned)
        })
        .collect();
    assert_eq!(
        pending_order,
        vec![
            "deep-file".to_string(),
            "mid-file".to_string(),
            "child-folder".to_string(),
            "root-folder".to_string()
        ]
    );

    let items = fixture
        .store
        .list_items_page(job.id(), None, 1, 50)
        .await
        .expect("items")
        .items;
    assert!(items.iter().all(|item| item.state == ItemState::Verified));
    assert!(
        items
            .iter()
            .all(|item| item.original_parent_ids == vec!["parent".to_string()])
    );
}

#[tokio::test]
async fn canary_halts_before_remaining_items_until_explicit_confirmation() {
    let fixture = setup(
        2,
        vec![
            ("deep-file", 3),
            ("mid-file", 2),
            ("child-folder", 1),
            ("root-folder", 0),
        ],
    )
    .await;
    let mut job = fixture.job.clone();
    let run = run_of(&fixture);
    execute_canary(&run, &mut job).await.expect("canary");
    assert_eq!(job.status(), JobStatus::CanaryReview);

    let items = fixture
        .store
        .list_items_for_transfer(fixture.job.id())
        .await
        .expect("remaining");
    assert_eq!(
        items
            .iter()
            .map(|item| item.file_id.as_str())
            .collect::<Vec<_>>(),
        vec!["child-folder", "root-folder"]
    );
    assert!(items.iter().all(|item| item.state == ItemState::Eligible));

    let mutated: HashSet<String> = captured_requests(&fixture)
        .iter()
        .filter(|request| matches!(request_method(request), Some("POST" | "PATCH")))
        .filter_map(|request| {
            path_file_id(request_path(request).unwrap_or("")).map(ToOwned::to_owned)
        })
        .collect();
    assert!(mutated.contains("deep-file"));
    assert!(mutated.contains("mid-file"));
    assert!(!mutated.contains("child-folder"));
    assert!(!mutated.contains("root-folder"));

    let run = run_of(&fixture);
    execute_bulk(&run, &mut job).await.expect("bulk");
    assert_eq!(job.status(), JobStatus::Completed);
    let remaining = fixture
        .store
        .list_items_for_transfer(fixture.job.id())
        .await
        .expect("none remaining");
    assert!(remaining.is_empty());
}

#[tokio::test]
async fn retryable_errors_use_backoff_without_wall_clock_sleep() {
    let fixture = setup(1, vec![("deep-file", 1)]).await;
    fixture.script.remaining_failures.store(2, Ordering::SeqCst);
    *fixture
        .script
        .fail_status
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = "429 Too Many Requests".into();
    let mut job = fixture.job.clone();
    let run = run_of(&fixture);
    execute_canary(&run, &mut job)
        .await
        .expect("canary after retries");
    assert_eq!(
        fixture.sleeper.delays(),
        vec![Duration::from_secs(1), Duration::from_secs(2)]
    );

    let fixture = setup(1, vec![("deep-file", 1)]).await;
    fixture.script.remaining_failures.store(1, Ordering::SeqCst);
    *fixture
        .script
        .fail_status
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = "503 Service Unavailable".into();
    let mut job = fixture.job.clone();
    let run = run_of(&fixture);
    execute_canary(&run, &mut job)
        .await
        .expect("canary after 503");
    assert_eq!(fixture.sleeper.delays(), vec![Duration::from_secs(1)]);

    let fixture = setup(1, vec![("deep-file", 1)]).await;
    fixture.script.remaining_failures.store(1, Ordering::SeqCst);
    *fixture
        .script
        .fail_on_verify_only
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
    *fixture
        .script
        .fail_status
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = "404 Not Found".into();
    let mut job = fixture.job.clone();
    let run = run_of(&fixture);
    execute_canary(&run, &mut job)
        .await
        .expect("canary after eventual 404");
    assert_eq!(fixture.sleeper.delays(), vec![Duration::from_secs(1)]);
}

#[tokio::test]
async fn sharing_rate_limit_pauses_without_fast_retry() {
    let fixture = setup(2, vec![("deep-file", 2), ("root-folder", 0)]).await;
    fixture
        .script
        .remaining_failures
        .store(20, Ordering::SeqCst);
    *fixture
        .script
        .fail_status
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = "403 Forbidden".into();
    *fixture
        .script
        .fail_body
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) =
        r#"{"error":{"errors":[{"reason":"sharingRateLimitExceeded"}]}}"#.into();
    let mut job = fixture.job.clone();
    let run = run_of(&fixture);
    let halt = execute_canary(&run, &mut job).await.expect("halt");
    assert!(matches!(halt, TransferHalt::SharingRateLimited { .. }));
    assert_eq!(job.status(), JobStatus::SourceRateLimited);
    assert!(fixture.sleeper.delays().is_empty());

    let remaining = fixture
        .store
        .list_items_for_transfer(fixture.job.id())
        .await
        .expect("items");
    assert_eq!(remaining.len(), 2);
    assert!(remaining.iter().any(|item| item.file_id == "root-folder"));
}

#[tokio::test]
async fn already_owned_item_verifies_without_permission_mutations() {
    let fixture = setup(1, vec![("owned-file", 1)]).await;
    fixture
        .script
        .already_owned
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert("owned-file".into());
    let mut job = fixture.job.clone();
    let run = run_of(&fixture);
    execute_canary(&run, &mut job).await.expect("canary");
    let requests = captured_requests(&fixture);
    let mutations = mutation_requests(&requests);
    assert!(mutations.is_empty());
    let items = fixture
        .store
        .list_items_page(job.id(), None, 1, 50)
        .await
        .expect("items")
        .items;
    assert_eq!(items[0].state, ItemState::Verified);
}

#[tokio::test]
async fn existing_writer_patches_pending_owner_with_source_token() {
    let fixture = setup(1, vec![("shared-file", 1)]).await;
    fixture
        .script
        .writer_without_pending
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert("shared-file".into());
    let mut job = fixture.job.clone();
    let run = run_of(&fixture);
    execute_canary(&run, &mut job).await.expect("canary");
    let requests = captured_requests(&fixture);
    assert!(
        !requests
            .iter()
            .any(|request| request_method(request) == Some("POST"))
    );
    let pending_patch = requests.iter().find(|request| {
        request_method(request) == Some("PATCH")
            && query_param(request, "transferOwnership").is_none()
    });
    let pending_patch = pending_patch.expect("pendingOwner patch");
    assert_eq!(
        authorization_bearer(pending_patch),
        Some(format!("Bearer {SOURCE_TOKEN}"))
    );
    assert_eq!(
        query_param(pending_patch, "sendNotificationEmail").as_deref(),
        Some("true")
    );
    assert!(request_body(pending_patch).contains("\"pendingOwner\":true"));
}

#[tokio::test]
async fn accept_required_resume_does_not_create_pending_owner_again() {
    let fixture = setup(1, vec![("resume-file", 1)]).await;
    let mut item = fixture
        .store
        .list_items_for_transfer(fixture.job.id())
        .await
        .expect("item")
        .remove(0);
    item.state = ItemState::AcceptRequired;
    item.target_permission_id = Some(GooglePermissionId::new(TARGET_PERM));
    fixture.store.save_item(&item).await.expect("seed");
    let mut job = fixture.job.clone();
    let run = run_of(&fixture);
    execute_canary(&run, &mut job).await.expect("canary");
    let requests = captured_requests(&fixture);
    assert!(
        !requests
            .iter()
            .any(|request| request_method(request) == Some("POST"))
    );
    assert!(requests.iter().any(|request| {
        request_method(request) == Some("PATCH")
            && query_param(request, "transferOwnership").as_deref() == Some("true")
    }));
}

#[tokio::test]
async fn trashed_during_verify_skips_item_and_continues_batch() {
    let fixture = setup(2, vec![("trash-file", 2), ("keep-file", 1)]).await;
    fixture
        .script
        .trash_on_target_get
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert("trash-file".into());
    let mut job = fixture.job.clone();
    let run = run_of(&fixture);
    let halt = execute_canary(&run, &mut job).await.expect("canary");
    assert!(matches!(halt, TransferHalt::Exhausted { verified: 1, .. }));
    assert_eq!(job.status(), JobStatus::CanaryReview);
    let items = fixture
        .store
        .list_items_page(job.id(), None, 1, 50)
        .await
        .expect("items")
        .items;
    let by_id: std::collections::HashMap<_, _> = items
        .into_iter()
        .map(|item| (item.file_id.clone(), item.state))
        .collect();
    assert_eq!(by_id["trash-file"], ItemState::SkippedTrashed);
    assert_eq!(by_id["keep-file"], ItemState::Verified);
}
