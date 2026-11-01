-- Migration 010: Add domain registration expiration monitoring using RDAP
ALTER TABLE monitors ADD COLUMN check_domain_expiry INTEGER NOT NULL DEFAULT 0;
ALTER TABLE monitors ADD COLUMN domain_warning_days INTEGER NOT NULL DEFAULT 30;
ALTER TABLE monitors ADD COLUMN domain_expires_at DATETIME;
ALTER TABLE monitors ADD COLUMN domain_days_remaining INTEGER;
ALTER TABLE monitors ADD COLUMN last_domain_alert_days INTEGER;
ALTER TABLE monitors ADD COLUMN domain_last_checked_at DATETIME;
ALTER TABLE check_logs ADD COLUMN domain_days_remaining INTEGER;
