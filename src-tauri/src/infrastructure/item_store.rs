use sqlx::Row;
use std::str::FromStr;

use crate::application::drive_tree::{
    FOLDER_MIME_TYPE, SCAN_CHECKPOINT_BATCH_SIZE, SHORTCUT_MIME_TYPE,
};
use crate::application::item_store::{
    ItemAggregates, ItemBatchCommit, ItemPage, ItemStoreError, ItemStoreFuture, ItemStorePort,
};
use crate::domain::GooglePermissionId;
use crate::domain::item::{ItemId, ItemState, MigrationItem, ScanCheckpoint};
use crate::domain::job::JobId;
use crate::infrastructure::job_store::SqliteJobStore;

impl ItemStorePort for SqliteJobStore {
    fn commit_scan_batch<'a>(
        &'a self,
        job_id: JobId,
        batch: &'a ItemBatchCommit,
    ) -> ItemStoreFuture<'a, usize> {
        Box::pin(async move {
            if batch.items.len() > SCAN_CHECKPOINT_BATCH_SIZE {
                return Err(ItemStoreError::BatchTooLarge {
                    size: batch.items.len(),
                    max: SCAN_CHECKPOINT_BATCH_SIZE,
                });
            }

            let mut tx = self
                .pool()
                .begin()
                .await
                .map_err(|e| ItemStoreError::Database(e.to_string()))?;

            let job_id_str = job_id.value().to_string();
            for item in &batch.items {
                let parents = serde_json::to_string(&item.original_parent_ids)
                    .map_err(|e| ItemStoreError::Database(e.to_string()))?;
                sqlx::query(
                    "INSERT INTO migration_items (
                        id, job_id, file_id, name, mime_type, depth,
                        original_parent_ids_json, original_owner_permission_id,
                        quota_bytes_used, target_permission_id, state,
                        created_at, updated_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                    ON CONFLICT(job_id, file_id) DO NOTHING",
                )
                .bind(item.id.value().to_string())
                .bind(&job_id_str)
                .bind(&item.file_id)
                .bind(&item.name)
                .bind(&item.mime_type)
                .bind(item.depth)
                .bind(parents)
                .bind(
                    item.original_owner_permission_id
                        .as_ref()
                        .map(|id| id.as_str().to_string()),
                )
                .bind(item.quota_bytes_used)
                .bind(
                    item.target_permission_id
                        .as_ref()
                        .map(|id| id.as_str().to_string()),
                )
                .bind(item.state.as_str())
                .bind(&item.created_at)
                .bind(&item.updated_at)
                .execute(&mut *tx)
                .await
                .map_err(|e| ItemStoreError::Database(e.to_string()))?;
            }

            for checkpoint in &batch.checkpoints_upsert {
                sqlx::query(
                    "INSERT INTO scan_checkpoints (job_id, folder_id, page_token, depth)
                     VALUES (?1, ?2, ?3, ?4)
                     ON CONFLICT(job_id, folder_id) DO UPDATE SET
                        page_token = excluded.page_token,
                        depth = excluded.depth",
                )
                .bind(&job_id_str)
                .bind(&checkpoint.folder_id)
                .bind(&checkpoint.page_token)
                .bind(checkpoint.depth)
                .execute(&mut *tx)
                .await
                .map_err(|e| ItemStoreError::Database(e.to_string()))?;
            }

            for folder_id in &batch.checkpoints_delete {
                sqlx::query("DELETE FROM scan_checkpoints WHERE job_id = ?1 AND folder_id = ?2")
                    .bind(&job_id_str)
                    .bind(folder_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| ItemStoreError::Database(e.to_string()))?;
            }

            tx.commit()
                .await
                .map_err(|e| ItemStoreError::Database(e.to_string()))?;
            Ok(batch.items.len())
        })
    }

    fn list_committed_file_ids<'a>(&'a self, job_id: JobId) -> ItemStoreFuture<'a, Vec<String>> {
        Box::pin(async move {
            let rows = sqlx::query("SELECT file_id FROM migration_items WHERE job_id = ?1")
                .bind(job_id.value().to_string())
                .fetch_all(self.pool())
                .await
                .map_err(|e| ItemStoreError::Database(e.to_string()))?;
            Ok(rows
                .into_iter()
                .map(|row| row.get::<String, _>(0))
                .collect())
        })
    }

    fn list_scan_checkpoints<'a>(
        &'a self,
        job_id: JobId,
    ) -> ItemStoreFuture<'a, Vec<ScanCheckpoint>> {
        Box::pin(async move {
            let rows = sqlx::query(
                "SELECT folder_id, page_token, depth FROM scan_checkpoints WHERE job_id = ?1",
            )
            .bind(job_id.value().to_string())
            .fetch_all(self.pool())
            .await
            .map_err(|e| ItemStoreError::Database(e.to_string()))?;

            Ok(rows
                .into_iter()
                .map(|row| ScanCheckpoint {
                    job_id,
                    folder_id: row.get(0),
                    page_token: row.get(1),
                    depth: row.get(2),
                })
                .collect())
        })
    }

    fn list_items_page<'a>(
        &'a self,
        job_id: JobId,
        filter: Option<&'a str>,
        page: u32,
        page_size: u32,
    ) -> ItemStoreFuture<'a, ItemPage> {
        Box::pin(async move {
            let page = page.max(1);
            let page_size = page_size.clamp(1, 500);
            let offset = i64::from((page - 1) * page_size);
            let limit = i64::from(page_size);
            let job_id_str = job_id.value().to_string();

            let (total, rows) = match filter.map(str::trim).filter(|value| !value.is_empty()) {
                None | Some("all") => {
                    let total: i64 = sqlx::query_scalar(
                        "SELECT COUNT(*) FROM migration_items WHERE job_id = ?1",
                    )
                    .bind(&job_id_str)
                    .fetch_one(self.pool())
                    .await
                    .map_err(|e| ItemStoreError::Database(e.to_string()))?;
                    let rows = sqlx::query(
                        "SELECT id, job_id, file_id, name, mime_type, depth, original_parent_ids_json,
                                original_owner_permission_id, quota_bytes_used, target_permission_id,
                                state, created_at, updated_at
                         FROM migration_items
                         WHERE job_id = ?1
                         ORDER BY depth DESC, name COLLATE NOCASE ASC
                         LIMIT ?2 OFFSET ?3",
                    )
                    .bind(&job_id_str)
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(self.pool())
                    .await
                    .map_err(|e| ItemStoreError::Database(e.to_string()))?;
                    (total, rows)
                }
                Some("eligible") => {
                    let total: i64 = sqlx::query_scalar(
                        "SELECT COUNT(*) FROM migration_items WHERE job_id = ?1 AND state = 'ELIGIBLE'",
                    )
                    .bind(&job_id_str)
                    .fetch_one(self.pool())
                    .await
                    .map_err(|e| ItemStoreError::Database(e.to_string()))?;
                    let rows = sqlx::query(
                        "SELECT id, job_id, file_id, name, mime_type, depth, original_parent_ids_json,
                                original_owner_permission_id, quota_bytes_used, target_permission_id,
                                state, created_at, updated_at
                         FROM migration_items
                         WHERE job_id = ?1 AND state = 'ELIGIBLE'
                         ORDER BY depth DESC, name COLLATE NOCASE ASC
                         LIMIT ?2 OFFSET ?3",
                    )
                    .bind(&job_id_str)
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(self.pool())
                    .await
                    .map_err(|e| ItemStoreError::Database(e.to_string()))?;
                    (total, rows)
                }
                Some("skipped") => {
                    let total: i64 = sqlx::query_scalar(
                        "SELECT COUNT(*) FROM migration_items
                         WHERE job_id = ?1 AND state LIKE 'SKIPPED_%'",
                    )
                    .bind(&job_id_str)
                    .fetch_one(self.pool())
                    .await
                    .map_err(|e| ItemStoreError::Database(e.to_string()))?;
                    let rows = sqlx::query(
                        "SELECT id, job_id, file_id, name, mime_type, depth, original_parent_ids_json,
                                original_owner_permission_id, quota_bytes_used, target_permission_id,
                                state, created_at, updated_at
                         FROM migration_items
                         WHERE job_id = ?1 AND state LIKE 'SKIPPED_%'
                         ORDER BY depth DESC, name COLLATE NOCASE ASC
                         LIMIT ?2 OFFSET ?3",
                    )
                    .bind(&job_id_str)
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(self.pool())
                    .await
                    .map_err(|e| ItemStoreError::Database(e.to_string()))?;
                    (total, rows)
                }
                Some("shortcut") | Some("shortcuts") => {
                    let total: i64 = sqlx::query_scalar(
                        "SELECT COUNT(*) FROM migration_items WHERE job_id = ?1 AND mime_type = ?2",
                    )
                    .bind(&job_id_str)
                    .bind(SHORTCUT_MIME_TYPE)
                    .fetch_one(self.pool())
                    .await
                    .map_err(|e| ItemStoreError::Database(e.to_string()))?;
                    let rows = sqlx::query(
                        "SELECT id, job_id, file_id, name, mime_type, depth, original_parent_ids_json,
                                original_owner_permission_id, quota_bytes_used, target_permission_id,
                                state, created_at, updated_at
                         FROM migration_items
                         WHERE job_id = ?1 AND mime_type = ?2
                         ORDER BY depth DESC, name COLLATE NOCASE ASC
                         LIMIT ?3 OFFSET ?4",
                    )
                    .bind(&job_id_str)
                    .bind(SHORTCUT_MIME_TYPE)
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(self.pool())
                    .await
                    .map_err(|e| ItemStoreError::Database(e.to_string()))?;
                    (total, rows)
                }
                Some(state) => {
                    let total: i64 = sqlx::query_scalar(
                        "SELECT COUNT(*) FROM migration_items WHERE job_id = ?1 AND state = ?2",
                    )
                    .bind(&job_id_str)
                    .bind(state)
                    .fetch_one(self.pool())
                    .await
                    .map_err(|e| ItemStoreError::Database(e.to_string()))?;
                    let rows = sqlx::query(
                        "SELECT id, job_id, file_id, name, mime_type, depth, original_parent_ids_json,
                                original_owner_permission_id, quota_bytes_used, target_permission_id,
                                state, created_at, updated_at
                         FROM migration_items
                         WHERE job_id = ?1 AND state = ?2
                         ORDER BY depth DESC, name COLLATE NOCASE ASC
                         LIMIT ?3 OFFSET ?4",
                    )
                    .bind(&job_id_str)
                    .bind(state)
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(self.pool())
                    .await
                    .map_err(|e| ItemStoreError::Database(e.to_string()))?;
                    (total, rows)
                }
            };

            let mut items = Vec::with_capacity(rows.len());
            for row in rows {
                items.push(item_from_row(row)?);
            }

            Ok(ItemPage {
                items,
                page,
                page_size,
                total: total as u64,
            })
        })
    }

    fn item_aggregates<'a>(&'a self, job_id: JobId) -> ItemStoreFuture<'a, ItemAggregates> {
        Box::pin(async move {
            let job_id_str = job_id.value().to_string();
            let total: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM migration_items WHERE job_id = ?1")
                    .bind(&job_id_str)
                    .fetch_one(self.pool())
                    .await
                    .map_err(|e| ItemStoreError::Database(e.to_string()))?;

            let eligible: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM migration_items WHERE job_id = ?1 AND state = 'ELIGIBLE'",
            )
            .bind(&job_id_str)
            .fetch_one(self.pool())
            .await
            .map_err(|e| ItemStoreError::Database(e.to_string()))?;

            let eligible_folders: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM migration_items
                 WHERE job_id = ?1 AND state = 'ELIGIBLE' AND mime_type = ?2",
            )
            .bind(&job_id_str)
            .bind(FOLDER_MIME_TYPE)
            .fetch_one(self.pool())
            .await
            .map_err(|e| ItemStoreError::Database(e.to_string()))?;

            let skipped_already: i64 =
                count_state(self, &job_id_str, "SKIPPED_ALREADY_OWNED_BY_TARGET").await?;
            let skipped_not_owned: i64 =
                count_state(self, &job_id_str, "SKIPPED_NOT_OWNED_BY_SOURCE").await?;
            let skipped_shared: i64 =
                count_state(self, &job_id_str, "SKIPPED_SHARED_DRIVE").await?;
            let skipped_trashed: i64 = count_state(self, &job_id_str, "SKIPPED_TRASHED").await?;
            let skipped_ineligible: i64 =
                count_state(self, &job_id_str, "SKIPPED_INELIGIBLE").await?;

            let skipped_shortcuts: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM migration_items WHERE job_id = ?1 AND mime_type = ?2",
            )
            .bind(&job_id_str)
            .bind(SHORTCUT_MIME_TYPE)
            .fetch_one(self.pool())
            .await
            .map_err(|e| ItemStoreError::Database(e.to_string()))?;

            let estimated: i64 = sqlx::query_scalar(
                "SELECT COALESCE(SUM(quota_bytes_used), 0) FROM migration_items
                 WHERE job_id = ?1 AND state = 'ELIGIBLE'",
            )
            .bind(&job_id_str)
            .fetch_one(self.pool())
            .await
            .map_err(|e| ItemStoreError::Database(e.to_string()))?;

            Ok(ItemAggregates {
                total: total as u64,
                eligible: eligible as u64,
                eligible_files: (eligible - eligible_folders).max(0) as u64,
                eligible_folders: eligible_folders as u64,
                skipped_already_owned_by_target: skipped_already as u64,
                skipped_not_owned_by_source: skipped_not_owned as u64,
                skipped_shared_drive: skipped_shared as u64,
                skipped_shortcuts: skipped_shortcuts as u64,
                skipped_trashed: skipped_trashed as u64,
                skipped_ineligible: (skipped_ineligible - skipped_shortcuts).max(0) as u64,
                estimated_quota_bytes: estimated.max(0) as u64,
            })
        })
    }

    fn list_items_for_transfer<'a>(
        &'a self,
        job_id: JobId,
    ) -> ItemStoreFuture<'a, Vec<MigrationItem>> {
        Box::pin(async move {
            let rows = sqlx::query(
                "SELECT id, job_id, file_id, name, mime_type, depth, original_parent_ids_json,
                        original_owner_permission_id, quota_bytes_used, target_permission_id,
                        state, created_at, updated_at
                 FROM migration_items
                 WHERE job_id = ?1 AND state IN (
                    'ELIGIBLE',
                    'PENDING_OWNER_REQUIRED',
                    'PENDING_OWNER_CREATED',
                    'ACCEPT_REQUIRED',
                    'ACCEPTING',
                    'TRANSFERRED',
                    'VERIFYING',
                    'RETRYABLE_FAILED'
                 )
                 ORDER BY depth DESC, name COLLATE NOCASE ASC",
            )
            .bind(job_id.value().to_string())
            .fetch_all(self.pool())
            .await
            .map_err(|e| ItemStoreError::Database(e.to_string()))?;

            let mut items = Vec::with_capacity(rows.len());
            for row in rows {
                items.push(item_from_row(row)?);
            }
            Ok(items)
        })
    }

    fn save_item<'a>(&'a self, item: &'a MigrationItem) -> ItemStoreFuture<'a, ()> {
        Box::pin(async move {
            let result = sqlx::query(
                "UPDATE migration_items
                 SET state = ?1, target_permission_id = ?2, updated_at = ?3
                 WHERE id = ?4 AND job_id = ?5",
            )
            .bind(item.state.as_str())
            .bind(
                item.target_permission_id
                    .as_ref()
                    .map(|id| id.as_str().to_string()),
            )
            .bind(&item.updated_at)
            .bind(item.id.value().to_string())
            .bind(item.job_id.value().to_string())
            .execute(self.pool())
            .await
            .map_err(|e| ItemStoreError::Database(e.to_string()))?;
            if result.rows_affected() == 0 {
                return Err(ItemStoreError::Database(format!(
                    "migration item {} was not found",
                    item.id
                )));
            }
            Ok(())
        })
    }
}

async fn count_state(
    store: &SqliteJobStore,
    job_id: &str,
    state: &str,
) -> Result<i64, ItemStoreError> {
    sqlx::query_scalar("SELECT COUNT(*) FROM migration_items WHERE job_id = ?1 AND state = ?2")
        .bind(job_id)
        .bind(state)
        .fetch_one(store.pool())
        .await
        .map_err(|e| ItemStoreError::Database(e.to_string()))
}

fn item_from_row(row: sqlx::sqlite::SqliteRow) -> Result<MigrationItem, ItemStoreError> {
    let id_str: String = row.get(0);
    let job_id_str: String = row.get(1);
    let file_id: String = row.get(2);
    let name: String = row.get(3);
    let mime_type: String = row.get(4);
    let depth: i64 = row.get(5);
    let parents_json: String = row.get(6);
    let owner: Option<String> = row.get(7);
    let quota: Option<i64> = row.get(8);
    let target: Option<String> = row.get(9);
    let state_str: String = row.get(10);
    let created_at: String = row.get(11);
    let updated_at: String = row.get(12);

    let original_parent_ids: Vec<String> = serde_json::from_str(&parents_json).unwrap_or_default();
    let state = ItemState::from_str(&state_str).map_err(|_| ItemStoreError::InvalidState)?;

    Ok(MigrationItem {
        id: ItemId::from_str(&id_str).map_err(|e| ItemStoreError::Database(e.to_string()))?,
        job_id: JobId::from_str(&job_id_str)
            .map_err(|e| ItemStoreError::Database(e.to_string()))?,
        file_id,
        name,
        mime_type,
        depth,
        original_parent_ids,
        original_owner_permission_id: owner.map(GooglePermissionId::new),
        quota_bytes_used: quota,
        target_permission_id: target.map(GooglePermissionId::new),
        state,
        created_at,
        updated_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::drive_tree::SCAN_CHECKPOINT_BATCH_SIZE;
    use crate::application::item_store::ItemStorePort;
    use crate::application::job_store::JobStorePort;
    use crate::domain::item::{ItemId, ItemState, MigrationItem};
    use crate::domain::job::{
        AccountSnapshot, JobId, MigrationJob, MigrationRoot, RootId, RootValidationStatus,
    };
    use crate::domain::{AccountId, GooglePermissionId};
    use crate::infrastructure::SqliteJobStore;
    use crate::infrastructure::account_store::SqliteAccountStore;

    fn snapshot(id: u128, perm: &str) -> AccountSnapshot {
        AccountSnapshot {
            account_id: AccountId::new(id),
            email: format!("user{id}@gmail.com"),
            display_name: format!("User {id}"),
            permission_id: GooglePermissionId::new(perm),
        }
    }

    async fn setup() -> (SqliteJobStore, JobId) {
        let accounts = SqliteAccountStore::open_in_memory().await.unwrap();
        let store = SqliteJobStore::new(accounts.pool().clone());
        sqlx::query(
            "INSERT INTO accounts (id, google_permission_id, email, display_name, auth_status, connected_at, last_authenticated_at, updated_at)
             VALUES ('1', 'p1', 'a@gmail.com', 'A', 'CONNECTED', datetime('now'), datetime('now'), datetime('now')),
                    ('2', 'p2', 'b@gmail.com', 'B', 'CONNECTED', datetime('now'), datetime('now'), datetime('now'))",
        )
        .execute(store.pool())
        .await
        .unwrap();
        let mut job = MigrationJob::new(
            JobId::new(42),
            snapshot(1, "p1"),
            snapshot(2, "p2"),
            "2026-09-05T00:00:00Z".into(),
        )
        .unwrap();
        job.add_root(MigrationRoot {
            id: RootId::new(1),
            job_id: job.id(),
            root_file_id: "root".into(),
            root_name: "Root".into(),
            validation_status: RootValidationStatus::Validated,
            created_at: "t".into(),
        })
        .unwrap();
        store.create_job(&job).await.unwrap();
        store.add_root(&job.roots()[0]).await.unwrap();
        (store, job.id())
    }

    fn item(job_id: JobId, file_id: &str) -> MigrationItem {
        MigrationItem {
            id: ItemId::new(crate::application::entity_id::next_entity_id()),
            job_id,
            file_id: file_id.into(),
            name: file_id.into(),
            mime_type: "text/plain".into(),
            depth: 1,
            original_parent_ids: vec!["root".into()],
            original_owner_permission_id: None,
            quota_bytes_used: Some(1),
            target_permission_id: None,
            state: ItemState::Eligible,
            created_at: "t".into(),
            updated_at: "t".into(),
        }
    }

    #[tokio::test]
    async fn unique_job_file_id_and_batch_limit() {
        let (store, job_id) = setup().await;
        let first = item(job_id, "same-file");
        store
            .commit_scan_batch(
                job_id,
                &ItemBatchCommit {
                    items: vec![first.clone(), item(job_id, "other")],
                    checkpoints_upsert: Vec::new(),
                    checkpoints_delete: Vec::new(),
                },
            )
            .await
            .unwrap();
        store
            .commit_scan_batch(
                job_id,
                &ItemBatchCommit {
                    items: vec![item(job_id, "same-file")],
                    checkpoints_upsert: Vec::new(),
                    checkpoints_delete: Vec::new(),
                },
            )
            .await
            .unwrap();
        let ids = store.list_committed_file_ids(job_id).await.unwrap();
        assert_eq!(ids.iter().filter(|id| *id == "same-file").count(), 1);

        let oversized: Vec<_> = (0..SCAN_CHECKPOINT_BATCH_SIZE + 1)
            .map(|i| item(job_id, &format!("x{i}")))
            .collect();
        let err = store
            .commit_scan_batch(
                job_id,
                &ItemBatchCommit {
                    items: oversized,
                    checkpoints_upsert: Vec::new(),
                    checkpoints_delete: Vec::new(),
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            ItemStoreError::BatchTooLarge {
                size: 101,
                max: 100
            }
        ));
    }

    #[tokio::test]
    async fn schema_migration_creates_items_table() {
        let accounts = SqliteAccountStore::open_in_memory().await.unwrap();
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM _schema_migrations WHERE version = 4")
                .fetch_one(accounts.pool())
                .await
                .unwrap();
        assert_eq!(count, 1);
        sqlx::query("SELECT job_id, file_id, depth, mime_type, original_parent_ids_json, state FROM migration_items LIMIT 1")
            .fetch_optional(accounts.pool())
            .await
            .unwrap();
    }
}
