-- 004_migration_items.sql
-- Migration 004: Scan inventory, item state, and folder listing checkpoints

CREATE TABLE IF NOT EXISTS migration_items (
    id TEXT PRIMARY KEY NOT NULL,
    job_id TEXT NOT NULL REFERENCES migration_jobs(id) ON DELETE CASCADE,
    file_id TEXT NOT NULL,
    name TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    depth INTEGER NOT NULL,
    original_parent_ids_json TEXT NOT NULL,
    original_owner_permission_id TEXT,
    quota_bytes_used INTEGER,
    target_permission_id TEXT,
    state TEXT NOT NULL,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    next_retry_at TEXT,
    last_error_code TEXT,
    last_error_reason TEXT,
    last_error_message TEXT,
    transferred_at TEXT,
    verified_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE(job_id, file_id)
) STRICT;

CREATE INDEX IF NOT EXISTS idx_migration_items_job_state
    ON migration_items(job_id, state);

CREATE TABLE IF NOT EXISTS scan_checkpoints (
    job_id TEXT NOT NULL REFERENCES migration_jobs(id) ON DELETE CASCADE,
    folder_id TEXT NOT NULL,
    page_token TEXT,
    depth INTEGER NOT NULL,
    PRIMARY KEY (job_id, folder_id)
) STRICT;
