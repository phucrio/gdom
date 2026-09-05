-- 003_migration_jobs.sql
-- Migration 003: Migration jobs and root folders schema

CREATE TABLE IF NOT EXISTS migration_jobs (
    id TEXT PRIMARY KEY NOT NULL,
    source_account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    target_account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    source_email_snapshot TEXT NOT NULL,
    target_email_snapshot TEXT NOT NULL,
    source_display_name_snapshot TEXT NOT NULL,
    target_display_name_snapshot TEXT NOT NULL,
    source_permission_id_snapshot TEXT NOT NULL,
    target_permission_id_snapshot TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'DRAFT',
    queue_position INTEGER,
    canary_size INTEGER NOT NULL DEFAULT 5,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    started_at TEXT,
    completed_at TEXT,
    last_error TEXT,
    CHECK(source_account_id != target_account_id)
) STRICT;

CREATE TABLE IF NOT EXISTS migration_roots (
    id TEXT PRIMARY KEY NOT NULL,
    job_id TEXT NOT NULL REFERENCES migration_jobs(id) ON DELETE CASCADE,
    root_file_id TEXT NOT NULL,
    root_name TEXT NOT NULL,
    validation_status TEXT NOT NULL DEFAULT 'VALIDATED',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE(job_id, root_file_id)
) STRICT;
