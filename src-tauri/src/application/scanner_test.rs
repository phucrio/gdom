use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::Notify;

use crate::application::AccessToken;
use crate::application::drive_folder::{
    DriveFolderLookupError, DriveFolderLookupFuture, DriveFolderLookupPort, DriveFolderMetadata,
};
use crate::application::drive_tree::{
    DriveChildPage, DriveListFuture, DrivePort, DriveQuotaFuture, DriveQuotaPort, DriveTreePort,
    FOLDER_MIME_TYPE, SCAN_CHECKPOINT_BATCH_SIZE, SHORTCUT_MIME_TYPE, StorageQuota,
};
use crate::application::item_store::{
    ItemAggregates, ItemBatchCommit, ItemPage, ItemStoreFuture, ItemStorePort,
};
use crate::application::job_store::JobStorePort;
use crate::application::scanner::{ScanError, ScanOutcome, ScanRun, run_scan};
use crate::domain::item::ItemState;
use crate::domain::job::{JobId, MigrationJob, MigrationRoot, RootId, RootValidationStatus};
use crate::domain::{AccountId, AccountSnapshot, GooglePermissionId};
use crate::infrastructure::SqliteJobStore;
use crate::infrastructure::account_store::SqliteAccountStore;
use crate::infrastructure::google_drive::GoogleDriveClient;
use crate::test_support::{
    SOURCE_PERM, SOURCE_TOKEN, TARGET_PERM, authorization_bearer, drive_child,
    folder_id_from_list_request, query_param, request_is_list, request_is_quota, shortcut_json,
    source_file, source_folder, spawn_http_handler,
};

struct MockDrive {
    listings: Mutex<HashMap<(String, Option<String>), DriveChildPage>>,
    quota: Mutex<StorageQuota>,
    list_calls: Mutex<Vec<(String, Option<String>, String)>>,
    quota_calls: Mutex<Vec<String>>,
    fail_at_call: Mutex<Option<usize>>,
    list_gate: Notify,
    wait_before_call: Mutex<Option<usize>>,
    calls: AtomicUsize,
}

impl MockDrive {
    fn new() -> Self {
        Self {
            listings: Mutex::new(HashMap::new()),
            quota: Mutex::new(StorageQuota {
                limit_bytes: Some(10_000),
                usage_bytes: 100,
            }),
            list_calls: Mutex::new(Vec::new()),
            quota_calls: Mutex::new(Vec::new()),
            fail_at_call: Mutex::new(None),
            list_gate: Notify::new(),
            wait_before_call: Mutex::new(None),
            calls: AtomicUsize::new(0),
        }
    }

    fn insert_page(&self, folder_id: &str, page_token: Option<&str>, page: DriveChildPage) {
        self.listings.lock().expect("lock").insert(
            (folder_id.to_string(), page_token.map(ToOwned::to_owned)),
            page,
        );
    }
}

impl DriveFolderLookupPort for MockDrive {
    fn get_folder_metadata<'a>(
        &'a self,
        _token: &'a AccessToken,
        folder_id: &'a str,
    ) -> DriveFolderLookupFuture<'a> {
        Box::pin(async move {
            Ok(DriveFolderMetadata {
                id: folder_id.to_string(),
                name: folder_id.to_string(),
                mime_type: FOLDER_MIME_TYPE.to_string(),
                trashed: false,
                drive_id: None,
                owners: Vec::new(),
            })
        })
    }
}

impl DriveTreePort for MockDrive {
    fn list_children<'a>(
        &'a self,
        token: &'a AccessToken,
        folder_id: &'a str,
        page_token: Option<&'a str>,
    ) -> DriveListFuture<'a> {
        Box::pin(async move {
            let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            self.list_calls.lock().expect("lock").push((
                folder_id.to_string(),
                page_token.map(ToOwned::to_owned),
                token.expose_secret().to_string(),
            ));
            if self
                .wait_before_call
                .lock()
                .expect("lock")
                .is_some_and(|n| call >= n)
            {
                self.list_gate.notified().await;
            }
            if self
                .fail_at_call
                .lock()
                .expect("lock")
                .is_some_and(|n| call == n)
            {
                return Err(DriveFolderLookupError::RateLimited);
            }
            self.listings
                .lock()
                .expect("lock")
                .get(&(folder_id.to_string(), page_token.map(ToOwned::to_owned)))
                .cloned()
                .ok_or(DriveFolderLookupError::NotFound)
        })
    }
}

impl DriveQuotaPort for MockDrive {
    fn get_storage_quota<'a>(&'a self, token: &'a AccessToken) -> DriveQuotaFuture<'a> {
        Box::pin(async move {
            self.quota_calls
                .lock()
                .expect("lock")
                .push(token.expose_secret().to_string());
            Ok(self.quota.lock().expect("lock").clone())
        })
    }
}

struct CountingStore {
    inner: SqliteJobStore,
    max_batch: Arc<AtomicUsize>,
}

impl ItemStorePort for CountingStore {
    fn commit_scan_batch<'a>(
        &'a self,
        job_id: crate::domain::job::JobId,
        batch: &'a ItemBatchCommit,
    ) -> ItemStoreFuture<'a, usize> {
        let size = batch.items.len();
        self.max_batch.fetch_max(size, Ordering::SeqCst);
        Box::pin(async move { self.inner.commit_scan_batch(job_id, batch).await })
    }

    fn list_committed_file_ids<'a>(
        &'a self,
        job_id: crate::domain::job::JobId,
    ) -> ItemStoreFuture<'a, Vec<String>> {
        self.inner.list_committed_file_ids(job_id)
    }

    fn list_scan_checkpoints<'a>(
        &'a self,
        job_id: crate::domain::job::JobId,
    ) -> ItemStoreFuture<'a, Vec<crate::domain::item::ScanCheckpoint>> {
        self.inner.list_scan_checkpoints(job_id)
    }

    fn list_items_page<'a>(
        &'a self,
        job_id: crate::domain::job::JobId,
        filter: Option<&'a str>,
        page: u32,
        page_size: u32,
    ) -> ItemStoreFuture<'a, ItemPage> {
        self.inner.list_items_page(job_id, filter, page, page_size)
    }

    fn item_aggregates<'a>(
        &'a self,
        job_id: crate::domain::job::JobId,
    ) -> ItemStoreFuture<'a, ItemAggregates> {
        self.inner.item_aggregates(job_id)
    }
}

async fn execute_scan(
    drive: Arc<dyn DrivePort>,
    store: &dyn ItemStorePort,
    job_id: JobId,
    roots: &[MigrationRoot],
    pause: &std::sync::atomic::AtomicBool,
    concurrency: usize,
) -> Result<ScanOutcome, ScanError> {
    let token = AccessToken::new(SOURCE_TOKEN.to_string());
    let source = GooglePermissionId::new(SOURCE_PERM);
    let target = GooglePermissionId::new(TARGET_PERM);
    run_scan(&ScanRun {
        drive,
        store,
        job_id,
        roots,
        source_token: &token,
        source_permission_id: &source,
        target_permission_id: &target,
        pause,
        concurrency,
    })
    .await
}

fn sample_snapshot(id: u128, email: &str, perm: &str) -> AccountSnapshot {
    AccountSnapshot {
        account_id: AccountId::new(id),
        email: email.to_string(),
        display_name: format!("User {id}"),
        permission_id: GooglePermissionId::new(perm),
    }
}

async fn seed_job_with_roots(store: &SqliteJobStore, roots: &[&str]) -> JobId {
    sqlx::query(
        "INSERT INTO accounts (id, google_permission_id, email, display_name, auth_status, connected_at, last_authenticated_at, updated_at)
         VALUES ('1', 'perm-source', 'source@gmail.com', 'Source', 'CONNECTED', datetime('now'), datetime('now'), datetime('now')),
                ('2', 'perm-target', 'target@gmail.com', 'Target', 'CONNECTED', datetime('now'), datetime('now'), datetime('now'))",
    )
    .execute(store.pool())
    .await
    .unwrap();

    let mut job = MigrationJob::new(
        JobId::new(9001),
        sample_snapshot(1, "source@gmail.com", SOURCE_PERM),
        sample_snapshot(2, "target@gmail.com", TARGET_PERM),
        "2026-09-05T00:00:00Z".to_string(),
    )
    .unwrap();
    for (index, root_id) in roots.iter().enumerate() {
        job.add_root(MigrationRoot {
            id: RootId::new(5000 + index as u128),
            job_id: job.id(),
            root_file_id: (*root_id).to_string(),
            root_name: format!("Root {root_id}"),
            validation_status: RootValidationStatus::Validated,
            created_at: "2026-09-05T00:00:00Z".to_string(),
        })
        .unwrap();
    }
    store.create_job(&job).await.unwrap();
    for root in job.roots() {
        store.add_root(root).await.unwrap();
    }
    job.id()
}

#[tokio::test]
async fn scan_dedupes_overlapping_roots_and_skips_ineligible_items() {
    let account_store = SqliteAccountStore::open_in_memory().await.unwrap();
    let store = SqliteJobStore::new(account_store.pool().clone());
    let job_id = seed_job_with_roots(&store, &["root-a", "root-b"]).await;

    let drive = Arc::new(MockDrive::new());
    drive.insert_page(
        "root-a",
        None,
        DriveChildPage {
            files: vec![
                drive_child(
                    "root-b",
                    "Nested",
                    FOLDER_MIME_TYPE,
                    SOURCE_PERM,
                    None,
                    None,
                    None,
                    false,
                ),
                drive_child(
                    "file-1",
                    "Doc",
                    "text/plain",
                    SOURCE_PERM,
                    None,
                    None,
                    Some(20),
                    false,
                ),
                drive_child(
                    "shortcut-1",
                    "Link",
                    SHORTCUT_MIME_TYPE,
                    SOURCE_PERM,
                    None,
                    Some("shortcut-target"),
                    None,
                    false,
                ),
                drive_child(
                    "shared-1",
                    "Shared",
                    "text/plain",
                    SOURCE_PERM,
                    Some("0AShared"),
                    None,
                    None,
                    false,
                ),
                drive_child(
                    "other-1",
                    "Other",
                    "text/plain",
                    "perm-other",
                    None,
                    None,
                    None,
                    false,
                ),
                drive_child(
                    "target-owned-1",
                    "Already",
                    "text/plain",
                    TARGET_PERM,
                    None,
                    None,
                    None,
                    false,
                ),
                drive_child(
                    "trashed-1",
                    "Trash",
                    "text/plain",
                    SOURCE_PERM,
                    None,
                    None,
                    None,
                    true,
                ),
            ],
            next_page_token: None,
        },
    );
    drive.insert_page(
        "root-b",
        None,
        DriveChildPage {
            files: vec![drive_child(
                "file-2",
                "Nested Doc",
                "text/plain",
                SOURCE_PERM,
                None,
                None,
                Some(5),
                false,
            )],
            next_page_token: None,
        },
    );

    let pause = std::sync::atomic::AtomicBool::new(false);
    let roots = [
        MigrationRoot {
            id: RootId::new(1),
            job_id,
            root_file_id: "root-a".into(),
            root_name: "A".into(),
            validation_status: RootValidationStatus::Validated,
            created_at: "t".into(),
        },
        MigrationRoot {
            id: RootId::new(2),
            job_id,
            root_file_id: "root-b".into(),
            root_name: "B".into(),
            validation_status: RootValidationStatus::Validated,
            created_at: "t".into(),
        },
    ];
    let outcome = execute_scan(
        drive.clone() as Arc<dyn DrivePort>,
        &store,
        job_id,
        &roots,
        &pause,
        4,
    )
    .await
    .expect("scan completes");
    assert_eq!(outcome, ScanOutcome::Completed);

    let items = store
        .list_items_page(job_id, None, 1, 50)
        .await
        .unwrap()
        .items;
    let ids: Vec<_> = items.iter().map(|item| item.file_id.as_str()).collect();
    assert_eq!(ids.iter().filter(|id| **id == "root-b").count(), 1);
    assert!(ids.contains(&"file-1"));
    assert!(ids.contains(&"file-2"));
    assert!(ids.contains(&"shortcut-1"));
    assert!(!ids.contains(&"shortcut-target"));
    let listed_folders: Vec<_> = drive
        .list_calls
        .lock()
        .unwrap()
        .iter()
        .map(|call| call.0.clone())
        .collect();
    assert!(!listed_folders.iter().any(|id| id == "shortcut-target"));
    assert!(
        drive
            .list_calls
            .lock()
            .unwrap()
            .iter()
            .all(|call| call.2 == SOURCE_TOKEN)
    );

    let by_id: HashMap<_, _> = items
        .iter()
        .map(|item| (item.file_id.clone(), item.state))
        .collect();
    assert_eq!(by_id["file-1"], ItemState::Eligible);
    assert_eq!(by_id["shortcut-1"], ItemState::SkippedIneligible);
    assert_eq!(by_id["shared-1"], ItemState::SkippedSharedDrive);
    assert_eq!(by_id["other-1"], ItemState::SkippedNotOwnedBySource);
    assert_eq!(
        by_id["target-owned-1"],
        ItemState::SkippedAlreadyOwnedByTarget
    );
    assert_eq!(by_id["trashed-1"], ItemState::SkippedTrashed);
}

#[tokio::test]
async fn scan_follows_page_tokens_and_commits_bounded_batches() {
    let account_store = SqliteAccountStore::open_in_memory().await.unwrap();
    let inner = SqliteJobStore::new(account_store.pool().clone());
    let job_id = seed_job_with_roots(&inner, &["root-page"]).await;
    let max_batch = Arc::new(AtomicUsize::new(0));
    let store = CountingStore {
        inner: inner.clone(),
        max_batch: Arc::clone(&max_batch),
    };

    let drive = Arc::new(MockDrive::new());
    let mut page_one = Vec::new();
    for index in 0..80 {
        page_one.push(drive_child(
            &format!("f-{index}"),
            &format!("File {index}"),
            "text/plain",
            SOURCE_PERM,
            None,
            None,
            Some(1),
            false,
        ));
    }
    drive.insert_page(
        "root-page",
        None,
        DriveChildPage {
            files: page_one,
            next_page_token: Some("page-2".into()),
        },
    );
    let mut page_two = Vec::new();
    for index in 80..150 {
        page_two.push(drive_child(
            &format!("f-{index}"),
            &format!("File {index}"),
            "text/plain",
            SOURCE_PERM,
            None,
            None,
            Some(1),
            false,
        ));
    }
    drive.insert_page(
        "root-page",
        Some("page-2"),
        DriveChildPage {
            files: page_two,
            next_page_token: None,
        },
    );

    let pause = std::sync::atomic::AtomicBool::new(false);
    execute_scan(
        drive.clone() as Arc<dyn DrivePort>,
        &store,
        job_id,
        &[MigrationRoot {
            id: RootId::new(1),
            job_id,
            root_file_id: "root-page".into(),
            root_name: "Paged".into(),
            validation_status: RootValidationStatus::Validated,
            created_at: "t".into(),
        }],
        &pause,
        1,
    )
    .await
    .unwrap();

    assert!(max_batch.load(Ordering::SeqCst) <= SCAN_CHECKPOINT_BATCH_SIZE);
    let ids = store.list_committed_file_ids(job_id).await.unwrap();
    assert_eq!(ids.len(), 151);
    let unique: std::collections::HashSet<_> = ids.iter().cloned().collect();
    assert_eq!(unique.len(), ids.len());
    assert_eq!(
        drive.list_calls.lock().unwrap()[1].1.as_deref(),
        Some("page-2")
    );
}

#[tokio::test]
async fn scan_resumes_from_committed_checkpoints_without_duplicates() {
    let account_store = SqliteAccountStore::open_in_memory().await.unwrap();
    let store = SqliteJobStore::new(account_store.pool().clone());
    let job_id = seed_job_with_roots(&store, &["root-resume"]).await;
    let drive = Arc::new(MockDrive::new());
    drive.insert_page(
        "root-resume",
        None,
        DriveChildPage {
            files: vec![drive_child(
                "keep-me",
                "Kept",
                "text/plain",
                SOURCE_PERM,
                None,
                None,
                Some(3),
                false,
            )],
            next_page_token: Some("more".into()),
        },
    );
    drive.insert_page(
        "root-resume",
        Some("more"),
        DriveChildPage {
            files: vec![drive_child(
                "after-crash",
                "Later",
                "text/plain",
                SOURCE_PERM,
                None,
                None,
                Some(4),
                false,
            )],
            next_page_token: None,
        },
    );
    *drive.fail_at_call.lock().unwrap() = Some(2);

    let pause = std::sync::atomic::AtomicBool::new(false);
    let roots = [MigrationRoot {
        id: RootId::new(1),
        job_id,
        root_file_id: "root-resume".into(),
        root_name: "Resume".into(),
        validation_status: RootValidationStatus::Validated,
        created_at: "t".into(),
    }];
    let first = execute_scan(
        drive.clone() as Arc<dyn DrivePort>,
        &store,
        job_id,
        &roots,
        &pause,
        1,
    )
    .await;
    assert!(matches!(first, Err(ScanError::RateLimited)));
    let committed = store.list_committed_file_ids(job_id).await.unwrap();
    assert!(committed.contains(&"keep-me".to_string()));
    assert!(!committed.contains(&"after-crash".to_string()));

    *drive.fail_at_call.lock().unwrap() = None;
    drive.calls.store(0, Ordering::SeqCst);
    execute_scan(
        drive.clone() as Arc<dyn DrivePort>,
        &store,
        job_id,
        &roots,
        &pause,
        1,
    )
    .await
    .unwrap();

    let ids = store.list_committed_file_ids(job_id).await.unwrap();
    let unique: std::collections::HashSet<_> = ids.iter().cloned().collect();
    assert_eq!(unique.len(), ids.len());
    assert!(ids.contains(&"keep-me".to_string()));
    assert!(ids.contains(&"after-crash".to_string()));
}

#[tokio::test]
async fn pause_stops_accepting_new_pages() {
    let account_store = SqliteAccountStore::open_in_memory().await.unwrap();
    let store = SqliteJobStore::new(account_store.pool().clone());
    let job_id = seed_job_with_roots(&store, &["root-pause"]).await;
    let drive = Arc::new(MockDrive::new());
    drive.insert_page(
        "root-pause",
        None,
        DriveChildPage {
            files: vec![drive_child(
                "first",
                "First",
                "text/plain",
                SOURCE_PERM,
                None,
                None,
                Some(1),
                false,
            )],
            next_page_token: Some("two".into()),
        },
    );
    drive.insert_page(
        "root-pause",
        Some("two"),
        DriveChildPage {
            files: vec![drive_child(
                "second",
                "Second",
                "text/plain",
                SOURCE_PERM,
                None,
                None,
                Some(1),
                false,
            )],
            next_page_token: None,
        },
    );
    *drive.wait_before_call.lock().unwrap() = Some(1);

    let pause = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let pause_flag = Arc::clone(&pause);
    let drive_scan = Arc::clone(&drive);
    let store = Arc::new(store);
    let store_scan = Arc::clone(&store);
    let handle = tokio::spawn(async move {
        execute_scan(
            drive_scan as Arc<dyn DrivePort>,
            store_scan.as_ref(),
            job_id,
            &[MigrationRoot {
                id: RootId::new(1),
                job_id,
                root_file_id: "root-pause".into(),
                root_name: "Pause".into(),
                validation_status: RootValidationStatus::Validated,
                created_at: "t".into(),
            }],
            pause_flag.as_ref(),
            1,
        )
        .await
    });

    let started = std::time::Instant::now();
    while drive.list_calls.lock().unwrap().is_empty() {
        if started.elapsed() > Duration::from_secs(2) {
            panic!("scan did not start listing");
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    pause.store(true, Ordering::SeqCst);
    drive.list_gate.notify_waiters();
    let outcome = handle.await.unwrap().unwrap();
    assert_eq!(outcome, ScanOutcome::Paused);
    let ids = store.list_committed_file_ids(job_id).await.unwrap();
    assert!(ids.contains(&"first".to_string()));
    assert!(!ids.contains(&"second".to_string()));
}

#[tokio::test]
async fn mock_http_scan_routes_source_and_target_tokens() {
    let listings = Arc::new(Mutex::new(HashMap::<String, String>::new()));
    listings.lock().unwrap().insert(
        "root-http".into(),
        format!(
            r#"{{"files":[{},{},{}],"nextPageToken":"p2"}}"#,
            source_folder("child-folder", "Child"),
            source_file("doc-1", "Doc 1", 11),
            shortcut_json("short-1", "not-listed-target"),
        ),
    );
    listings.lock().unwrap().insert(
        "p2".into(),
        format!(r#"{{"files":[{}]}}"#, source_file("doc-2", "Doc 2", 9)),
    );
    listings
        .lock()
        .unwrap()
        .insert("child-folder".into(), r#"{"files":[]}"#.into());

    let (base_url, captured) = spawn_http_handler(move |request| {
        if request_is_quota(request) {
            return (
                "200 OK".into(),
                r#"{"storageQuota":{"limit":"1000","usage":"10"}}"#.into(),
            );
        }
        if request_is_list(request) {
            if let Some(page) = query_param(request, "pageToken") {
                let body = listings
                    .lock()
                    .unwrap()
                    .get(&page)
                    .cloned()
                    .unwrap_or_else(|| r#"{"files":[]}"#.into());
                return ("200 OK".into(), body);
            }
            let folder = folder_id_from_list_request(request).unwrap_or_default();
            let body = listings
                .lock()
                .unwrap()
                .get(&folder)
                .cloned()
                .unwrap_or_else(|| r#"{"files":[]}"#.into());
            return ("200 OK".into(), body);
        }
        ("404 Not Found".into(), "{}".into())
    });

    let account_store = SqliteAccountStore::open_in_memory().await.unwrap();
    let store = SqliteJobStore::new(account_store.pool().clone());
    let job_id = seed_job_with_roots(&store, &["root-http"]).await;
    let client = GoogleDriveClient::for_test(base_url).unwrap();
    let pause = std::sync::atomic::AtomicBool::new(false);
    execute_scan(
        Arc::new(client) as Arc<dyn DrivePort>,
        &store,
        job_id,
        &[MigrationRoot {
            id: RootId::new(1),
            job_id,
            root_file_id: "root-http".into(),
            root_name: "HTTP".into(),
            validation_status: RootValidationStatus::Validated,
            created_at: "t".into(),
        }],
        &pause,
        2,
    )
    .await
    .unwrap();

    let ids = store.list_committed_file_ids(job_id).await.unwrap();
    assert!(ids.contains(&"doc-1".to_string()));
    assert!(ids.contains(&"doc-2".to_string()));
    assert!(ids.contains(&"short-1".to_string()));
    assert!(!ids.contains(&"not-listed-target".to_string()));

    tokio::time::sleep(Duration::from_millis(30)).await;
    let requests = captured.lock().unwrap().clone();
    assert!(requests.iter().any(|req| request_is_list(req)));
    assert!(
        requests
            .iter()
            .filter(|req| request_is_list(req))
            .all(|req| {
                authorization_bearer(req).as_deref() == Some(&format!("Bearer {SOURCE_TOKEN}"))
            })
    );
    assert!(requests.iter().all(|req| !req.starts_with("POST ")));
    assert!(requests.iter().all(|req| !req.starts_with("PATCH ")));
}

#[tokio::test]
async fn mock_http_rate_limit_does_not_mutate() {
    let (base_url, captured) = spawn_http_handler(|request| {
        if request_is_list(request) {
            return (
                "429 Too Many Requests".into(),
                r#"{"error":{"code":429}}"#.into(),
            );
        }
        ("200 OK".into(), "{}".into())
    });
    let account_store = SqliteAccountStore::open_in_memory().await.unwrap();
    let store = SqliteJobStore::new(account_store.pool().clone());
    let job_id = seed_job_with_roots(&store, &["root-429"]).await;
    let client = GoogleDriveClient::for_test(base_url).unwrap();
    let pause = std::sync::atomic::AtomicBool::new(false);
    let err = execute_scan(
        Arc::new(client) as Arc<dyn DrivePort>,
        &store,
        job_id,
        &[MigrationRoot {
            id: RootId::new(1),
            job_id,
            root_file_id: "root-429".into(),
            root_name: "Limited".into(),
            validation_status: RootValidationStatus::Validated,
            created_at: "t".into(),
        }],
        &pause,
        1,
    )
    .await
    .expect_err("429 is surfaced");
    assert!(matches!(err, ScanError::RateLimited));
    tokio::time::sleep(Duration::from_millis(30)).await;
    let requests = captured.lock().unwrap().clone();
    assert!(requests.iter().any(|req| request_is_list(req)));
    assert!(requests.iter().all(|req| !req.starts_with("POST ")));
    assert!(requests.iter().all(|req| !req.starts_with("PATCH ")));
}
