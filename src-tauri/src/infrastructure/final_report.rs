use std::str::FromStr;

use sqlx::{Row, SqlitePool};

use crate::application::final_report::{FinalReportItem, FinalReportSnapshot};
use crate::application::job_store::JobStorePortError;
use crate::domain::item::ItemState;
use crate::domain::job::{JobId, JobStatus};

pub(super) async fn read_snapshot(
    pool: &SqlitePool,
    job_id: JobId,
) -> Result<FinalReportSnapshot, JobStorePortError> {
    let mut transaction = pool.begin().await.map_err(database_error)?;
    // The first read pins job, roots, items and lease to one SQLite snapshot.
    let row = sqlx::query("SELECT * FROM migration_jobs WHERE id = ?")
        .bind(job_id.to_string())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(database_error)?
        .ok_or(JobStorePortError::JobNotFound(job_id))?;
    let status = JobStatus::from_str(row.get("status"))
        .map_err(|error| JobStorePortError::Database(error.to_string()))?;
    if !matches!(
        status,
        JobStatus::Completed
            | JobStatus::CompletedWithErrors
            | JobStatus::Cancelled
            | JobStatus::Failed
    ) {
        return Err(JobStorePortError::Database(
            "final report requires a finished job".into(),
        ));
    }
    let lease_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM worker_leases WHERE job_id = ?")
            .bind(job_id.to_string())
            .fetch_one(&mut *transaction)
            .await
            .map_err(database_error)?;
    if lease_count > 0 {
        return Err(JobStorePortError::MutationLeaseHeld);
    }
    let roots = sqlx::query(
        "SELECT root_file_id, root_name FROM migration_roots WHERE job_id = ? ORDER BY id",
    )
    .bind(job_id.to_string())
    .fetch_all(&mut *transaction)
    .await
    .map_err(database_error)?
    .iter()
    .map(|root| (root.get("root_file_id"), root.get("root_name")))
    .collect();
    let rows = sqlx::query("SELECT file_id, name, mime_type, depth, quota_bytes_used, original_parent_ids_json, state, last_error_code, last_error_reason, last_error_message, transferred_at, verified_at FROM migration_items WHERE job_id = ? ORDER BY depth, file_id")
        .bind(job_id.to_string())
        .fetch_all(&mut *transaction)
        .await
        .map_err(database_error)?;
    let mut items = Vec::with_capacity(rows.len());
    for item in rows {
        items.push(FinalReportItem {
            file_id: item.get("file_id"),
            name: item.get("name"),
            mime_type: item.get("mime_type"),
            depth: item.get("depth"),
            quota_bytes_used: item.get("quota_bytes_used"),
            original_parent_ids: serde_json::from_str(item.get("original_parent_ids_json"))
                .map_err(|error| JobStorePortError::Database(error.to_string()))?,
            state: ItemState::from_str(item.get("state"))
                .map_err(|error| JobStorePortError::Database(error.to_string()))?,
            error_code: item.get("last_error_code"),
            error_reason: item.get("last_error_reason"),
            error_message: item.get("last_error_message"),
            transferred_at: item.get("transferred_at"),
            verified_at: item.get("verified_at"),
        });
    }
    transaction.commit().await.map_err(database_error)?;
    Ok(FinalReportSnapshot {
        job_id,
        status,
        source_email: row.get("source_email_snapshot"),
        source_display_name: row.get("source_display_name_snapshot"),
        target_email: row.get("target_email_snapshot"),
        target_display_name: row.get("target_display_name_snapshot"),
        created_at: row.get("created_at"),
        started_at: row.get("started_at"),
        completed_at: row.get("completed_at"),
        last_error: row.get("last_error"),
        roots,
        items,
    })
}

fn database_error(error: sqlx::Error) -> JobStorePortError {
    JobStorePortError::Database(error.to_string())
}
