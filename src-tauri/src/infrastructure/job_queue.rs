use sqlx::{Row, SqlitePool};

use crate::application::job_store::{JobStorePortError, MigrationEvent};
use crate::domain::job::{JobStatus, MigrationJob};

pub(super) async fn persist_queue_changes(
    pool: &SqlitePool,
    expected: &[MigrationJob],
    changes: &[(MigrationJob, MigrationEvent)],
) -> Result<(), JobStorePortError> {
    let database_error = |error: sqlx::Error| JobStorePortError::Database(error.to_string());
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(database_error)?;
    let queued =
        sqlx::query("SELECT id, queue_position FROM migration_jobs WHERE status = 'QUEUED'")
            .fetch_all(&mut *transaction)
            .await
            .map_err(database_error)?;
    let expected_queued: Vec<_> = expected
        .iter()
        .filter(|job| job.status() == JobStatus::Queued)
        .collect();
    if queued.len() != expected_queued.len()
        || queued.iter().any(|row| {
            let id: String = row.get("id");
            let position: Option<i64> = row.get("queue_position");
            !expected_queued
                .iter()
                .any(|job| job.id().to_string() == id && job.queue_position() == position)
        })
    {
        return Err(JobStorePortError::Database(
            "queue changed; refresh and retry".into(),
        ));
    }
    for (job, event) in changes {
        let original = expected
            .iter()
            .find(|original| original.id() == job.id())
            .ok_or_else(|| JobStorePortError::Database("queue snapshot is incomplete".into()))?;
        let result = sqlx::query(
            "UPDATE migration_jobs SET status = ?1, queue_position = ?2
             WHERE id = ?3 AND status = ?4 AND queue_position IS ?5
             AND NOT EXISTS (SELECT 1 FROM worker_leases WHERE job_id = ?3)",
        )
        .bind(job.status().as_str())
        .bind(job.queue_position())
        .bind(job.id().to_string())
        .bind(original.status().as_str())
        .bind(original.queue_position())
        .execute(&mut *transaction)
        .await
        .map_err(database_error)?;
        if result.rows_affected() != 1 {
            return Err(JobStorePortError::Database(
                "job changed; refresh and retry".into(),
            ));
        }
        super::job_store::insert_migration_event(&mut transaction, event)
            .await
            .map_err(database_error)?;
    }
    transaction.commit().await.map_err(database_error)
}
