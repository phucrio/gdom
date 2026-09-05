-- 005_canary_selected.sql
-- Durable canary cohort membership so resume cannot expand past CANARY_REVIEW.

ALTER TABLE migration_items ADD COLUMN canary_selected INTEGER NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_migration_items_job_canary
    ON migration_items(job_id, canary_selected);
