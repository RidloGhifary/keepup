-- Migration 011: Add TLS/SSL certificate expiration monitoring
ALTER TABLE monitors ADD COLUMN check_ssl INTEGER NOT NULL DEFAULT 0;
ALTER TABLE monitors ADD COLUMN ssl_warning_days INTEGER NOT NULL DEFAULT 30;
ALTER TABLE monitors ADD COLUMN ssl_expires_at DATETIME;
ALTER TABLE monitors ADD COLUMN ssl_days_remaining INTEGER;
ALTER TABLE monitors ADD COLUMN ssl_last_checked_at DATETIME;
ALTER TABLE check_logs ADD COLUMN ssl_days_remaining INTEGER;
