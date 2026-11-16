-- Migration: Add SSL & Domain Expiry monitor type support
ALTER TABLE monitors ADD COLUMN expiry_threshold_days INTEGER NOT NULL DEFAULT 30;
