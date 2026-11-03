-- Migration: Add multi-protocol monitoring support (HTTP, TCP, DNS, PING)
ALTER TABLE monitors ADD COLUMN check_type TEXT NOT NULL DEFAULT 'http';
ALTER TABLE monitors ADD COLUMN port INTEGER;
ALTER TABLE monitors ADD COLUMN dns_record_type TEXT;
ALTER TABLE monitors ADD COLUMN dns_expected_value TEXT;
