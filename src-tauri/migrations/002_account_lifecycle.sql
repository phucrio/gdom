-- Migration 002_account_lifecycle.sql: Account lifecycle & durable timestamps
-- Enforces STRICT mode and expands accounts schema to support full lifecycle tracking.

ALTER TABLE accounts ADD COLUMN label TEXT;
ALTER TABLE accounts ADD COLUMN auth_status TEXT NOT NULL DEFAULT 'CONNECTED';
ALTER TABLE accounts ADD COLUMN connected_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
ALTER TABLE accounts ADD COLUMN last_authenticated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
ALTER TABLE accounts ADD COLUMN updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
ALTER TABLE accounts ADD COLUMN removed_at TEXT;
