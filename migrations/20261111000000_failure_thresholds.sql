-- Migration: Add current_consecutive_fails to monitors table to track flapping and consecutive retries
ALTER TABLE monitors ADD COLUMN current_consecutive_fails INTEGER NOT NULL DEFAULT 0;
