-- 006_worker_leases.sql
-- Global single-mutation lease (at most one row) and append-only audit log.

CREATE TABLE IF NOT EXISTS worker_leases (
    lease_id INTEGER PRIMARY KEY CHECK (lease_id = 1),
    job_id TEXT NOT NULL UNIQUE REFERENCES migration_jobs(id) ON DELETE CASCADE,
    owner_instance_id TEXT NOT NULL,
    acquired_at TEXT NOT NULL,
    heartbeat_at TEXT NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS migration_events (
    id TEXT PRIMARY KEY NOT NULL,
    job_id TEXT NOT NULL REFERENCES migration_jobs(id) ON DELETE CASCADE,
    file_id TEXT,
    account_id TEXT,
    event_type TEXT NOT NULL,
    previous_state TEXT,
    new_state TEXT,
    sanitized_detail_json TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

CREATE INDEX IF NOT EXISTS idx_migration_events_job_created
    ON migration_events(job_id, created_at);
