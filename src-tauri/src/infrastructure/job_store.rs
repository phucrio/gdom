use sqlx::{Row, SqlitePool};
use std::str::FromStr;

use crate::application::job_store::{JobStoreFuture, JobStorePort, JobStorePortError};
use crate::domain::job::{
    AccountPair, AccountSnapshot, JobAccountSnapshots, JobId, JobStatus, MigrationJob,
    MigrationRoot, RootId, RootValidationStatus,
};
use crate::domain::{AccountId, GooglePermissionId};

#[derive(Clone)]
pub struct SqliteJobStore {
    pool: SqlitePool,
}

impl SqliteJobStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

impl JobStorePort for SqliteJobStore {
    fn create_job<'a>(&'a self, job: &'a MigrationJob) -> JobStoreFuture<'a, ()> {
        Box::pin(async move {
            let id = job.id().value().to_string();
            let source_acc = job.source_account_id().value().to_string();
            let target_acc = job.target_account_id().value().to_string();
            let source_snap = &job.snapshots().source;
            let target_snap = &job.snapshots().target;
            let status = job.status().as_str();
            let canary_size = job.canary_size() as i64;
            let created_at = job.created_at();

            sqlx::query(
                "INSERT INTO migration_jobs (
                    id, source_account_id, target_account_id,
                    source_email_snapshot, target_email_snapshot,
                    source_display_name_snapshot, target_display_name_snapshot,
                    source_permission_id_snapshot, target_permission_id_snapshot,
                    status, queue_position, canary_size, created_at, started_at, completed_at, last_error
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            )
            .bind(id)
            .bind(source_acc)
            .bind(target_acc)
            .bind(&source_snap.email)
            .bind(&target_snap.email)
            .bind(&source_snap.display_name)
            .bind(&target_snap.display_name)
            .bind(source_snap.permission_id.as_str())
            .bind(target_snap.permission_id.as_str())
            .bind(status)
            .bind(job.queue_position())
            .bind(canary_size)
            .bind(created_at)
            .bind(job.started_at())
            .bind(job.completed_at())
            .bind(job.last_error())
            .execute(&self.pool)
            .await
            .map_err(|e| JobStorePortError::Database(e.to_string()))?;

            Ok(())
        })
    }

    fn update_job<'a>(&'a self, job: &'a MigrationJob) -> JobStoreFuture<'a, ()> {
        Box::pin(async move {
            let id = job.id().value().to_string();
            let source_acc = job.source_account_id().value().to_string();
            let target_acc = job.target_account_id().value().to_string();
            let source_snap = &job.snapshots().source;
            let target_snap = &job.snapshots().target;
            let status = job.status().as_str();
            let canary_size = job.canary_size() as i64;

            let res = sqlx::query(
                "UPDATE migration_jobs SET
                    source_account_id = ?1,
                    target_account_id = ?2,
                    source_email_snapshot = ?3,
                    target_email_snapshot = ?4,
                    source_display_name_snapshot = ?5,
                    target_display_name_snapshot = ?6,
                    source_permission_id_snapshot = ?7,
                    target_permission_id_snapshot = ?8,
                    status = ?9,
                    queue_position = ?10,
                    canary_size = ?11,
                    started_at = ?12,
                    completed_at = ?13,
                    last_error = ?14
                WHERE id = ?15",
            )
            .bind(source_acc)
            .bind(target_acc)
            .bind(&source_snap.email)
            .bind(&target_snap.email)
            .bind(&source_snap.display_name)
            .bind(&target_snap.display_name)
            .bind(source_snap.permission_id.as_str())
            .bind(target_snap.permission_id.as_str())
            .bind(status)
            .bind(job.queue_position())
            .bind(canary_size)
            .bind(job.started_at())
            .bind(job.completed_at())
            .bind(job.last_error())
            .bind(&id)
            .execute(&self.pool)
            .await
            .map_err(|e| JobStorePortError::Database(e.to_string()))?;

            if res.rows_affected() == 0 {
                return Err(JobStorePortError::JobNotFound(job.id()));
            }

            Ok(())
        })
    }

    fn update_draft_job<'a>(&'a self, job: &'a MigrationJob) -> JobStoreFuture<'a, ()> {
        Box::pin(async move {
            let mut tx = self
                .pool
                .begin()
                .await
                .map_err(|e| JobStorePortError::Database(e.to_string()))?;

            let id = job.id().value().to_string();
            let source_acc = job.source_account_id().value().to_string();
            let target_acc = job.target_account_id().value().to_string();
            let source_snap = &job.snapshots().source;
            let target_snap = &job.snapshots().target;
            let canary_size = job.canary_size() as i64;

            let res = sqlx::query(
                "UPDATE migration_jobs SET
                    source_account_id = ?1,
                    target_account_id = ?2,
                    source_email_snapshot = ?3,
                    target_email_snapshot = ?4,
                    source_display_name_snapshot = ?5,
                    target_display_name_snapshot = ?6,
                    source_permission_id_snapshot = ?7,
                    target_permission_id_snapshot = ?8,
                    queue_position = ?9,
                    canary_size = ?10,
                    started_at = ?11,
                    completed_at = ?12,
                    last_error = ?13
                WHERE id = ?14 AND status = 'DRAFT'",
            )
            .bind(&source_acc)
            .bind(&target_acc)
            .bind(&source_snap.email)
            .bind(&target_snap.email)
            .bind(&source_snap.display_name)
            .bind(&target_snap.display_name)
            .bind(source_snap.permission_id.as_str())
            .bind(target_snap.permission_id.as_str())
            .bind(job.queue_position())
            .bind(canary_size)
            .bind(job.started_at())
            .bind(job.completed_at())
            .bind(job.last_error())
            .bind(&id)
            .execute(&mut *tx)
            .await
            .map_err(|e| JobStorePortError::Database(e.to_string()))?;

            if res.rows_affected() == 0 {
                let exists: i64 =
                    sqlx::query_scalar("SELECT COUNT(*) FROM migration_jobs WHERE id = ?1")
                        .bind(&id)
                        .fetch_one(&mut *tx)
                        .await
                        .map_err(|e| JobStorePortError::Database(e.to_string()))?;
                if exists > 0 {
                    return Err(JobStorePortError::AccountPairLocked);
                }
                return Err(JobStorePortError::JobNotFound(job.id()));
            }

            sqlx::query("DELETE FROM migration_roots WHERE job_id = ?1")
                .bind(&id)
                .execute(&mut *tx)
                .await
                .map_err(|e| JobStorePortError::Database(e.to_string()))?;

            for root in job.roots() {
                sqlx::query(
                    "INSERT INTO migration_roots (id, job_id, root_file_id, root_name, validation_status, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                )
                .bind(root.id.value().to_string())
                .bind(root.job_id.value().to_string())
                .bind(&root.root_file_id)
                .bind(&root.root_name)
                .bind(root.validation_status.as_str())
                .bind(&root.created_at)
                .execute(&mut *tx)
                .await
                .map_err(|e| JobStorePortError::Database(e.to_string()))?;
            }

            tx.commit()
                .await
                .map_err(|e| JobStorePortError::Database(e.to_string()))?;
            Ok(())
        })
    }

    fn find_job_by_id<'a>(&'a self, job_id: JobId) -> JobStoreFuture<'a, Option<MigrationJob>> {
        Box::pin(async move {
            let row = sqlx::query(
                "SELECT id, source_account_id, target_account_id,
                        source_email_snapshot, target_email_snapshot,
                        source_display_name_snapshot, target_display_name_snapshot,
                        source_permission_id_snapshot, target_permission_id_snapshot,
                        status, queue_position, canary_size, created_at, started_at, completed_at, last_error
                 FROM migration_jobs WHERE id = ?1",
            )
            .bind(job_id.value().to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| JobStorePortError::Database(e.to_string()))?;

            let Some(row) = row else {
                return Ok(None);
            };

            let id_str: String = row.get(0);
            let source_acc_str: String = row.get(1);
            let target_acc_str: String = row.get(2);
            let source_email: String = row.get(3);
            let target_email: String = row.get(4);
            let source_display_name: String = row.get(5);
            let target_display_name: String = row.get(6);
            let source_perm: String = row.get(7);
            let target_perm: String = row.get(8);
            let status_str: String = row.get(9);
            let queue_position: Option<i64> = row.get(10);
            let canary_size_i64: i64 = row.get(11);
            let created_at: String = row.get(12);
            let started_at: Option<String> = row.get(13);
            let completed_at: Option<String> = row.get(14);
            let last_error: Option<String> = row.get(15);

            let id =
                JobId::from_str(&id_str).map_err(|e| JobStorePortError::Database(e.to_string()))?;
            let source_id = AccountId::new(
                source_acc_str
                    .parse::<u128>()
                    .map_err(|e| JobStorePortError::Database(e.to_string()))?,
            );
            let target_id = AccountId::new(
                target_acc_str
                    .parse::<u128>()
                    .map_err(|e| JobStorePortError::Database(e.to_string()))?,
            );

            let accounts = AccountPair::new(source_id, target_id)
                .map_err(|_| JobStorePortError::SameSourceAndTarget)?;

            let snapshots = JobAccountSnapshots {
                source: AccountSnapshot {
                    account_id: source_id,
                    email: source_email,
                    display_name: source_display_name,
                    permission_id: GooglePermissionId::new(source_perm),
                },
                target: AccountSnapshot {
                    account_id: target_id,
                    email: target_email,
                    display_name: target_display_name,
                    permission_id: GooglePermissionId::new(target_perm),
                },
            };

            let status = JobStatus::from_str(&status_str)
                .map_err(|_| JobStorePortError::Database("invalid job status in db".to_string()))?;

            let roots = self.list_roots_for_job(id).await?;

            Ok(Some(MigrationJob::reconstitute(
                id,
                accounts,
                snapshots,
                status,
                queue_position,
                canary_size_i64 as usize,
                created_at,
                started_at,
                completed_at,
                last_error,
                roots,
            )))
        })
    }

    fn list_jobs<'a>(&'a self) -> JobStoreFuture<'a, Vec<MigrationJob>> {
        Box::pin(async move {
            let rows = sqlx::query(
                "SELECT id, source_account_id, target_account_id,
                        source_email_snapshot, target_email_snapshot,
                        source_display_name_snapshot, target_display_name_snapshot,
                        source_permission_id_snapshot, target_permission_id_snapshot,
                        status, queue_position, canary_size, created_at, started_at, completed_at, last_error
                 FROM migration_jobs ORDER BY created_at DESC",
            )
            .fetch_all(&self.pool)
            .await
            .map_err(|e| JobStorePortError::Database(e.to_string()))?;

            let mut jobs = Vec::with_capacity(rows.len());
            for row in rows {
                let id_str: String = row.get(0);
                let source_acc_str: String = row.get(1);
                let target_acc_str: String = row.get(2);
                let source_email: String = row.get(3);
                let target_email: String = row.get(4);
                let source_display_name: String = row.get(5);
                let target_display_name: String = row.get(6);
                let source_perm: String = row.get(7);
                let target_perm: String = row.get(8);
                let status_str: String = row.get(9);
                let queue_position: Option<i64> = row.get(10);
                let canary_size_i64: i64 = row.get(11);
                let created_at: String = row.get(12);
                let started_at: Option<String> = row.get(13);
                let completed_at: Option<String> = row.get(14);
                let last_error: Option<String> = row.get(15);

                let id = JobId::from_str(&id_str)
                    .map_err(|e| JobStorePortError::Database(e.to_string()))?;
                let source_id = AccountId::new(
                    source_acc_str
                        .parse::<u128>()
                        .map_err(|e| JobStorePortError::Database(e.to_string()))?,
                );
                let target_id = AccountId::new(
                    target_acc_str
                        .parse::<u128>()
                        .map_err(|e| JobStorePortError::Database(e.to_string()))?,
                );

                let accounts = AccountPair::new(source_id, target_id)
                    .map_err(|_| JobStorePortError::SameSourceAndTarget)?;

                let snapshots = JobAccountSnapshots {
                    source: AccountSnapshot {
                        account_id: source_id,
                        email: source_email,
                        display_name: source_display_name,
                        permission_id: GooglePermissionId::new(source_perm),
                    },
                    target: AccountSnapshot {
                        account_id: target_id,
                        email: target_email,
                        display_name: target_display_name,
                        permission_id: GooglePermissionId::new(target_perm),
                    },
                };

                let status = JobStatus::from_str(&status_str).map_err(|_| {
                    JobStorePortError::Database("invalid job status in db".to_string())
                })?;

                let roots = self.list_roots_for_job(id).await?;

                jobs.push(MigrationJob::reconstitute(
                    id,
                    accounts,
                    snapshots,
                    status,
                    queue_position,
                    canary_size_i64 as usize,
                    created_at,
                    started_at,
                    completed_at,
                    last_error,
                    roots,
                ));
            }

            Ok(jobs)
        })
    }

    fn add_root<'a>(&'a self, root: &'a MigrationRoot) -> JobStoreFuture<'a, ()> {
        Box::pin(async move {
            let id = root.id.value().to_string();
            let job_id = root.job_id.value().to_string();
            let validation_status = root.validation_status.as_str();

            let res = sqlx::query(
                "INSERT INTO migration_roots (id, job_id, root_file_id, root_name, validation_status, created_at)
                 SELECT ?1, ?2, ?3, ?4, ?5, ?6
                 FROM migration_jobs
                 WHERE id = ?2 AND status = 'DRAFT'",
            )
            .bind(&id)
            .bind(&job_id)
            .bind(&root.root_file_id)
            .bind(&root.root_name)
            .bind(validation_status)
            .bind(&root.created_at)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                if let sqlx::Error::Database(ref dbe) = e
                    && dbe.is_unique_violation()
                {
                    return JobStorePortError::DuplicateRoot(root.root_file_id.clone());
                }
                JobStorePortError::Database(e.to_string())
            })?;

            if res.rows_affected() == 0 {
                let exists: i64 =
                    sqlx::query_scalar("SELECT COUNT(*) FROM migration_jobs WHERE id = ?1")
                        .bind(&job_id)
                        .fetch_one(&self.pool)
                        .await
                        .map_err(|e| JobStorePortError::Database(e.to_string()))?;
                if exists > 0 {
                    return Err(JobStorePortError::RootsLocked);
                }
                return Err(JobStorePortError::JobNotFound(root.job_id));
            }

            Ok(())
        })
    }

    fn remove_root<'a>(&'a self, job_id: JobId, root_id: RootId) -> JobStoreFuture<'a, ()> {
        Box::pin(async move {
            let res = sqlx::query(
                "DELETE FROM migration_roots
                 WHERE job_id = ?1 AND id = ?2
                   AND EXISTS (SELECT 1 FROM migration_jobs WHERE id = ?1 AND status = 'DRAFT')",
            )
            .bind(job_id.value().to_string())
            .bind(root_id.value().to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| JobStorePortError::Database(e.to_string()))?;

            if res.rows_affected() == 0 {
                return Err(JobStorePortError::RootNotFound(root_id));
            }

            Ok(())
        })
    }

    fn list_roots_for_job<'a>(&'a self, job_id: JobId) -> JobStoreFuture<'a, Vec<MigrationRoot>> {
        Box::pin(async move {
            let rows = sqlx::query(
                "SELECT id, job_id, root_file_id, root_name, validation_status, created_at
                 FROM migration_roots WHERE job_id = ?1 ORDER BY created_at ASC",
            )
            .bind(job_id.value().to_string())
            .fetch_all(&self.pool)
            .await
            .map_err(|e| JobStorePortError::Database(e.to_string()))?;

            let mut roots = Vec::with_capacity(rows.len());
            for row in rows {
                let id_str: String = row.get(0);
                let job_id_str: String = row.get(1);
                let root_file_id: String = row.get(2);
                let root_name: String = row.get(3);
                let val_status_str: String = row.get(4);
                let created_at: String = row.get(5);

                let id = RootId::from_str(&id_str)
                    .map_err(|e| JobStorePortError::Database(e.to_string()))?;
                let jid = JobId::from_str(&job_id_str)
                    .map_err(|e| JobStorePortError::Database(e.to_string()))?;
                let validation_status =
                    RootValidationStatus::from_str(&val_status_str).map_err(|_| {
                        JobStorePortError::Database("invalid root validation status".to_string())
                    })?;

                roots.push(MigrationRoot {
                    id,
                    job_id: jid,
                    root_file_id,
                    root_name,
                    validation_status,
                    created_at,
                });
            }

            Ok(roots)
        })
    }

    fn has_active_jobs_for_account<'a>(
        &'a self,
        account_id: AccountId,
    ) -> JobStoreFuture<'a, bool> {
        Box::pin(async move {
            let acc_str = account_id.value().to_string();
            let count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM migration_jobs
                 WHERE (source_account_id = ?1 OR target_account_id = ?1)
                   AND status NOT IN ('COMPLETED', 'COMPLETED_WITH_ERRORS', 'CANCELLED', 'FAILED')",
            )
            .bind(acc_str)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| JobStorePortError::Database(e.to_string()))?;

            Ok(count > 0)
        })
    }

    fn has_jobs_for_account<'a>(&'a self, account_id: AccountId) -> JobStoreFuture<'a, bool> {
        Box::pin(async move {
            let acc_str = account_id.value().to_string();
            let count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM migration_jobs
                 WHERE source_account_id = ?1 OR target_account_id = ?1",
            )
            .bind(acc_str)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| JobStorePortError::Database(e.to_string()))?;

            Ok(count > 0)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::account_store::SqliteAccountStore;

    fn sample_snapshot(id: u128, email: &str, perm: &str) -> AccountSnapshot {
        AccountSnapshot {
            account_id: AccountId::new(id),
            email: email.to_string(),
            display_name: format!("User {id}"),
            permission_id: GooglePermissionId::new(perm),
        }
    }

    #[tokio::test]
    async fn job_store_crud_lifecycle() {
        let store = SqliteAccountStore::open_in_memory().await.unwrap();
        let job_store = SqliteJobStore::new(store.pool().clone());

        // Insert dummy accounts first because of foreign keys
        sqlx::query(
            "INSERT INTO accounts (id, google_permission_id, email, display_name, auth_status, connected_at, last_authenticated_at, updated_at)
             VALUES ('1', 'perm_1', 'source@gmail.com', 'Source', 'CONNECTED', datetime('now'), datetime('now'), datetime('now')),
                    ('2', 'perm_2', 'target@gmail.com', 'Target', 'CONNECTED', datetime('now'), datetime('now'), datetime('now'))"
        )
        .execute(job_store.pool())
        .await
        .unwrap();

        let source = sample_snapshot(1, "source@gmail.com", "perm_1");
        let target = sample_snapshot(2, "target@gmail.com", "perm_2");
        let job = MigrationJob::new(
            JobId::new(1001),
            source,
            target,
            "2026-09-05T00:00:00Z".to_string(),
        )
        .unwrap();

        // Create job
        job_store.create_job(&job).await.unwrap();

        // Find job
        let found = job_store.find_job_by_id(job.id()).await.unwrap();
        assert!(found.is_some());
        let found = found.unwrap();
        assert_eq!(found.id(), job.id());
        assert_eq!(found.source_account_id(), AccountId::new(1));
        assert_eq!(found.target_account_id(), AccountId::new(2));

        // Add root
        let root = MigrationRoot {
            id: RootId::new(5001),
            job_id: job.id(),
            root_file_id: "folder_123".to_string(),
            root_name: "Test Folder".to_string(),
            validation_status: RootValidationStatus::Validated,
            created_at: "2026-09-05T00:00:00Z".to_string(),
        };
        job_store.add_root(&root).await.unwrap();

        // Duplicate root rejected
        let dup_err = job_store.add_root(&root).await;
        assert!(matches!(dup_err, Err(JobStorePortError::DuplicateRoot(_))));

        // Roots loaded with job
        let found_with_root = job_store.find_job_by_id(job.id()).await.unwrap().unwrap();
        assert_eq!(found_with_root.roots().len(), 1);
        assert_eq!(found_with_root.roots()[0].root_file_id, "folder_123");

        // Active jobs check
        assert!(
            job_store
                .has_active_jobs_for_account(AccountId::new(1))
                .await
                .unwrap()
        );
        assert!(
            job_store
                .has_active_jobs_for_account(AccountId::new(2))
                .await
                .unwrap()
        );
        assert!(
            !job_store
                .has_active_jobs_for_account(AccountId::new(999))
                .await
                .unwrap()
        );

        // Remove root
        job_store.remove_root(job.id(), root.id).await.unwrap();
        let roots_after_remove = job_store.list_roots_for_job(job.id()).await.unwrap();
        assert!(roots_after_remove.is_empty());
    }
}
