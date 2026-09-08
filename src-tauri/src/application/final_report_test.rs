use super::*;
use std::sync::Arc;

use crate::application::{AccountTokenProvider, JobService, JobServiceError, JobStorePort};
use crate::infrastructure::SqliteJobStore;
use crate::infrastructure::account_store::SqliteAccountStore;
use crate::infrastructure::google_drive::GoogleDriveClient;
use crate::infrastructure::google_token::DynamicGoogleTokenClient;
use crate::infrastructure::secrets::WindowsCredentialStore;

async fn report_service() -> (
    JobService<SqliteAccountStore, SqliteJobStore>,
    Arc<SqliteJobStore>,
) {
    let accounts = Arc::new(SqliteAccountStore::open_in_memory().await.unwrap());
    let store = Arc::new(SqliteJobStore::new(accounts.pool().clone()));
    sqlx::query("INSERT INTO accounts (id, google_permission_id, email, display_name, auth_status, connected_at, last_authenticated_at, updated_at) VALUES ('1','source-perm','source@gmail.com','Source','DISCONNECTED','t','t','t'), ('2','target-perm','target@gmail.com','Target','DISCONNECTED','t','t','t')")
        .execute(store.pool()).await.unwrap();
    sqlx::query("INSERT INTO migration_jobs (id,source_account_id,target_account_id,source_email_snapshot,target_email_snapshot,source_display_name_snapshot,target_display_name_snapshot,source_permission_id_snapshot,target_permission_id_snapshot,status,created_at,started_at,completed_at,last_error) VALUES ('10','1','2','=source@gmail.com','target@gmail.com','Source','Target','source-perm','target-perm','COMPLETED','created','started','finished','Bearer hidden-secret')")
        .execute(store.pool()).await.unwrap();
    sqlx::query("INSERT INTO migration_roots (id,job_id,root_file_id,root_name) VALUES ('20','10','root-id','+Root')")
        .execute(store.pool()).await.unwrap();
    let token_provider = Arc::new(AccountTokenProvider::new(
        Arc::new(DynamicGoogleTokenClient::new(Arc::new(
            tokio::sync::RwLock::new(None),
        ))),
        Arc::new(WindowsCredentialStore::new_mock()),
        accounts.clone(),
    ));
    let service = JobService::new(
        accounts,
        store.clone(),
        Arc::new(GoogleDriveClient::for_test("http://127.0.0.1:1".into()).unwrap()),
        token_provider,
    );
    (service, store)
}

async fn insert_item(store: &SqliteJobStore, index: usize, state: ItemState) {
    sqlx::query("INSERT INTO migration_items (id,job_id,file_id,name,mime_type,depth,original_parent_ids_json,state,last_error_reason,last_error_message) VALUES (?,'10',?,'=Item','text/plain',0,'[]',?,'@reason','access_token=hidden-token')")
        .bind(index.to_string()).bind(format!("file-{index}")).bind(state.as_str())
        .execute(store.pool()).await.unwrap();
}

#[tokio::test]
async fn report_counts_transferred_as_unfinished_and_keeps_all_inventory() {
    let (_, store) = report_service().await;
    for (index, state) in [
        ItemState::Verified,
        ItemState::RetryableFailed,
        ItemState::PermanentFailed,
        ItemState::Cancelled,
        ItemState::SkippedIneligible,
        ItemState::Transferred,
        ItemState::Verifying,
    ]
    .into_iter()
    .enumerate()
    {
        insert_item(&store, index, state).await;
    }
    for index in 7..507 {
        insert_item(&store, index, ItemState::Verified).await;
    }
    let report = store.final_report_snapshot(JobId::new(10)).await.unwrap();
    assert_eq!(
        report.counts(),
        FinalReportCounts {
            total: 507,
            verified: 501,
            failed: 2,
            cancelled: 1,
            skipped: 1,
            unfinished: 2
        }
    );
    assert_eq!(report.roots, vec![("root-id".into(), "+Root".into())]);
}

#[tokio::test]
async fn csv_escapes_metadata_roots_items_and_redacts_errors() {
    let (_, store) = report_service().await;
    insert_item(&store, 1, ItemState::PermanentFailed).await;
    let report = store
        .final_report_snapshot(JobId::new(10))
        .await
        .unwrap()
        .render(true);
    assert!(report.contains("source_email,\"'=source@gmail.com\""));
    assert!(report.contains("root-id,\"'+Root\""));
    assert!(report.contains("\"'=Item\""));
    assert!(report.contains("\"'@reason\""));
    assert!(!report.contains("hidden-secret"));
    assert!(!report.contains("hidden-token"));
}

#[tokio::test]
async fn export_terminal_jobs_to_local_files_without_connected_accounts() {
    let (service, store) = report_service().await;
    let directory = std::env::temp_dir().join(format!(
        "gdom-report-{}",
        crate::application::entity_id::next_entity_id()
    ));
    for status in [
        JobStatus::Completed,
        JobStatus::CompletedWithErrors,
        JobStatus::Cancelled,
        JobStatus::Failed,
    ] {
        sqlx::query("UPDATE migration_jobs SET status = ? WHERE id = '10'")
            .bind(status.as_str())
            .execute(store.pool())
            .await
            .unwrap();
        for extension in ["txt", "csv"] {
            let path = directory.join(format!("{}.{}", status.as_str(), extension));
            let result = service
                .export_final_report(JobId::new(10), path.to_str().unwrap())
                .await
                .unwrap();
            assert_eq!(result.status, status);
            assert_eq!(result.counts.total, 0);
            let contents = std::fs::read_to_string(path).unwrap();
            assert!(contents.contains("finished"));
            assert!(!contents.contains("hidden-secret"));
        }
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[tokio::test]
async fn export_rejects_active_jobs_and_invalid_destinations() {
    let (service, store) = report_service().await;
    for destination in ["", "report.json"] {
        assert!(matches!(
            service
                .export_final_report(JobId::new(10), destination)
                .await,
            Err(JobServiceError::ExportFailed(_))
        ));
    }
    sqlx::query("UPDATE migration_jobs SET status = 'RUNNING' WHERE id = '10'")
        .execute(store.pool())
        .await
        .unwrap();
    assert!(matches!(
        service
            .export_final_report(JobId::new(10), "unused.txt")
            .await,
        Err(JobServiceError::IllegalTransition)
    ));
}

#[tokio::test]
async fn export_rejects_terminal_job_while_worker_lease_remains() {
    let (service, store) = report_service().await;
    store
        .acquire_mutation_lease(JobId::new(10), "worker", "t")
        .await
        .unwrap();
    assert!(
        service
            .export_final_report(JobId::new(10), "unused.txt")
            .await
            .is_err()
    );
}

#[tokio::test]
async fn export_surfaces_write_failure() {
    let (service, _) = report_service().await;
    let path = std::env::temp_dir().join(format!(
        "gdom-report-{}.txt",
        crate::application::entity_id::next_entity_id()
    ));
    std::fs::create_dir(&path).unwrap();
    assert!(matches!(
        service
            .export_final_report(JobId::new(10), path.to_str().unwrap())
            .await,
        Err(JobServiceError::ExportFailed(_))
    ));
    std::fs::remove_dir(path).unwrap();
}
