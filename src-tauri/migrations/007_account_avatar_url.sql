-- Migration 007_account_avatar_url.sql: Add avatar_url column to accounts
ALTER TABLE accounts ADD COLUMN avatar_url TEXT;
