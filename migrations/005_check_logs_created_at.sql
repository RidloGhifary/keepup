-- Migration 005: Add created_at column to check_logs table for timestamp flexibility
ALTER TABLE check_logs ADD COLUMN created_at DATETIME;
UPDATE check_logs SET created_at = checked_at WHERE created_at IS NULL;
