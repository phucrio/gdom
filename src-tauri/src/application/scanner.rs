use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::application::AccessToken;
use crate::application::drive_tree::{
    DrivePort, DriveTreeError, FOLDER_MIME_TYPE, SCAN_CHECKPOINT_BATCH_SIZE,
};
use crate::application::entity_id::next_entity_id;
use crate::application::item_classifier::classify_drive_child;
use crate::application::item_store::{ItemBatchCommit, ItemStoreError, ItemStorePort};
use crate::application::time::iso_now;
use crate::domain::GooglePermissionId;
use crate::domain::item::{ItemId, ItemState, MigrationItem, ScanCheckpoint};
use crate::domain::job::{JobId, MigrationRoot};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScanOutcome {
    Completed,
    Paused,
}

#[derive(Debug)]
pub enum ScanError {
    RateLimited,
    Drive(DriveTreeError),
    Store(ItemStoreError),
}

impl std::fmt::Display for ScanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RateLimited => write!(f, "Google Drive rate limit reached during scan"),
            Self::Drive(err) => write!(f, "Drive listing failed: {err}"),
            Self::Store(err) => write!(f, "scan persistence failed: {err}"),
        }
    }
}

impl std::error::Error for ScanError {}

impl From<DriveTreeError> for ScanError {
    fn from(err: DriveTreeError) -> Self {
        match err {
            DriveTreeError::RateLimited => Self::RateLimited,
            other => Self::Drive(other),
        }
    }
}

impl From<ItemStoreError> for ScanError {
    fn from(err: ItemStoreError) -> Self {
        Self::Store(err)
    }
}

struct WorkItem {
    folder_id: String,
    page_token: Option<String>,
    depth: i64,
}

pub struct ScanRun<'a> {
    pub drive: Arc<dyn DrivePort>,
    pub store: &'a dyn ItemStorePort,
    pub job_id: JobId,
    pub roots: &'a [MigrationRoot],
    pub source_token: &'a AccessToken,
    pub source_permission_id: &'a GooglePermissionId,
    pub target_permission_id: &'a GooglePermissionId,
    pub pause: &'a AtomicBool,
    pub concurrency: usize,
}

struct PageContext<'a> {
    store: &'a dyn ItemStorePort,
    job_id: JobId,
    source_permission_id: &'a GooglePermissionId,
    target_permission_id: &'a GooglePermissionId,
    visited: &'a mut HashSet<String>,
    queue: &'a mut VecDeque<WorkItem>,
}

pub async fn run_scan(run: &ScanRun<'_>) -> Result<ScanOutcome, ScanError> {
    let mut visited: HashSet<String> = run
        .store
        .list_committed_file_ids(run.job_id)
        .await?
        .into_iter()
        .collect();

    let mut checkpoints = run.store.list_scan_checkpoints(run.job_id).await?;
    if checkpoints.is_empty() && visited.is_empty() {
        seed_roots(
            run.store,
            run.job_id,
            run.roots,
            run.source_permission_id,
            &mut visited,
            &mut checkpoints,
        )
        .await?;
    }

    let mut queue: VecDeque<WorkItem> = checkpoints
        .into_iter()
        .map(|checkpoint| WorkItem {
            folder_id: checkpoint.folder_id,
            page_token: checkpoint.page_token,
            depth: checkpoint.depth,
        })
        .collect();

    let concurrency = run.concurrency.max(1);

    while !queue.is_empty() {
        if run.pause.load(Ordering::SeqCst) {
            return Ok(ScanOutcome::Paused);
        }

        let wave_size = queue.len().min(concurrency);
        let mut wave = Vec::with_capacity(wave_size);
        for _ in 0..wave_size {
            if let Some(work) = queue.pop_front() {
                wave.push(work);
            }
        }

        let mut join_set = tokio::task::JoinSet::new();
        for work in wave {
            let drive = Arc::clone(&run.drive);
            let token = run.source_token.clone();
            let folder_id = work.folder_id.clone();
            let page_token = work.page_token.clone();
            let depth = work.depth;
            join_set.spawn(async move {
                let result = drive
                    .list_children(&token, &folder_id, page_token.as_deref())
                    .await;
                (folder_id, page_token, depth, result)
            });
        }

        while let Some(joined) = join_set.join_next().await {
            let (folder_id, _page_token, depth, result) =
                joined.map_err(|_| ScanError::Drive(DriveTreeError::Transport))?;
            let page = result?;
            apply_page(
                &mut PageContext {
                    store: run.store,
                    job_id: run.job_id,
                    source_permission_id: run.source_permission_id,
                    target_permission_id: run.target_permission_id,
                    visited: &mut visited,
                    queue: &mut queue,
                },
                folder_id,
                depth,
                page,
            )
            .await?;
        }
    }

    Ok(ScanOutcome::Completed)
}

async fn seed_roots(
    store: &dyn ItemStorePort,
    job_id: JobId,
    roots: &[MigrationRoot],
    source_permission_id: &GooglePermissionId,
    visited: &mut HashSet<String>,
    checkpoints: &mut Vec<ScanCheckpoint>,
) -> Result<(), ScanError> {
    let now = iso_now();
    let mut items = Vec::new();
    let mut upserts = Vec::new();
    for root in roots {
        if !visited.insert(root.root_file_id.clone()) {
            continue;
        }
        items.push(MigrationItem {
            id: ItemId::new(next_entity_id()),
            job_id,
            file_id: root.root_file_id.clone(),
            name: root.root_name.clone(),
            mime_type: FOLDER_MIME_TYPE.to_string(),
            depth: 0,
            original_parent_ids: Vec::new(),
            original_owner_permission_id: Some(source_permission_id.clone()),
            quota_bytes_used: None,
            target_permission_id: None,
            state: ItemState::Eligible,
            created_at: now.clone(),
            updated_at: now.clone(),
        });
        upserts.push(ScanCheckpoint {
            job_id,
            folder_id: root.root_file_id.clone(),
            page_token: None,
            depth: 0,
        });
    }

    if items.is_empty() {
        return Ok(());
    }

    for offset in (0..items.len()).step_by(SCAN_CHECKPOINT_BATCH_SIZE) {
        let end = (offset + SCAN_CHECKPOINT_BATCH_SIZE).min(items.len());
        let batch = ItemBatchCommit {
            items: items[offset..end].to_vec(),
            checkpoints_upsert: upserts[offset..end].to_vec(),
            checkpoints_delete: Vec::new(),
        };
        store.commit_scan_batch(job_id, &batch).await?;
    }

    checkpoints.extend(upserts);
    Ok(())
}

async fn apply_page(
    ctx: &mut PageContext<'_>,
    folder_id: String,
    depth: i64,
    page: crate::application::drive_tree::DriveChildPage,
) -> Result<(), ScanError> {
    let now = iso_now();
    let child_depth = depth + 1;
    let mut discovered = Vec::new();
    let mut folder_checkpoints = Vec::new();

    for child in page.files {
        if !ctx.visited.insert(child.id.clone()) {
            continue;
        }
        let disposition =
            classify_drive_child(&child, ctx.source_permission_id, ctx.target_permission_id);
        let owner = child
            .owners
            .first()
            .map(|owner| owner.permission_id.clone());
        discovered.push(MigrationItem {
            id: ItemId::new(next_entity_id()),
            job_id: ctx.job_id,
            file_id: child.id.clone(),
            name: child.name,
            mime_type: child.mime_type,
            depth: child_depth,
            original_parent_ids: child.parents,
            original_owner_permission_id: owner,
            quota_bytes_used: child.quota_bytes_used,
            target_permission_id: None,
            state: disposition.item_state(),
            created_at: now.clone(),
            updated_at: now.clone(),
        });
        if disposition.should_recurse() {
            folder_checkpoints.push(ScanCheckpoint {
                job_id: ctx.job_id,
                folder_id: child.id,
                page_token: None,
                depth: child_depth,
            });
        }
    }

    let mut remaining_items = discovered;
    let mut remaining_folders = folder_checkpoints;
    let enqueue_folders = remaining_folders.clone();
    while !remaining_items.is_empty() {
        let take = remaining_items.len().min(SCAN_CHECKPOINT_BATCH_SIZE);
        let items: Vec<MigrationItem> = remaining_items.drain(..take).collect();
        let upsert_take = remaining_folders.len().min(take);
        let checkpoints_upsert: Vec<ScanCheckpoint> =
            remaining_folders.drain(..upsert_take).collect();
        ctx.store
            .commit_scan_batch(
                ctx.job_id,
                &ItemBatchCommit {
                    items,
                    checkpoints_upsert,
                    checkpoints_delete: Vec::new(),
                },
            )
            .await?;
    }

    if !remaining_folders.is_empty() {
        ctx.store
            .commit_scan_batch(
                ctx.job_id,
                &ItemBatchCommit {
                    items: Vec::new(),
                    checkpoints_upsert: remaining_folders,
                    checkpoints_delete: Vec::new(),
                },
            )
            .await?;
    }

    if let Some(next) = page.next_page_token {
        ctx.store
            .commit_scan_batch(
                ctx.job_id,
                &ItemBatchCommit {
                    items: Vec::new(),
                    checkpoints_upsert: vec![ScanCheckpoint {
                        job_id: ctx.job_id,
                        folder_id: folder_id.clone(),
                        page_token: Some(next.clone()),
                        depth,
                    }],
                    checkpoints_delete: Vec::new(),
                },
            )
            .await?;
        ctx.queue.push_back(WorkItem {
            folder_id,
            page_token: Some(next),
            depth,
        });
    } else {
        ctx.store
            .commit_scan_batch(
                ctx.job_id,
                &ItemBatchCommit {
                    items: Vec::new(),
                    checkpoints_upsert: Vec::new(),
                    checkpoints_delete: vec![folder_id],
                },
            )
            .await?;
    }

    for checkpoint in enqueue_folders {
        ctx.queue.push_back(WorkItem {
            folder_id: checkpoint.folder_id,
            page_token: checkpoint.page_token,
            depth: checkpoint.depth,
        });
    }

    Ok(())
}
