-- Migration: Add SSL Certificate Authority details and CAA validation metadata
ALTER TABLE monitors ADD COLUMN ssl_issuer TEXT;
ALTER TABLE monitors ADD COLUMN ssl_serial_number TEXT;
ALTER TABLE monitors ADD COLUMN ssl_signature_algorithm TEXT;
ALTER TABLE monitors ADD COLUMN ssl_sans TEXT;
ALTER TABLE monitors ADD COLUMN ssl_issued_at DATETIME;
ALTER TABLE monitors ADD COLUMN caa_status TEXT;
ALTER TABLE monitors ADD COLUMN caa_details TEXT;

ALTER TABLE check_logs ADD COLUMN ssl_issuer TEXT;
ALTER TABLE check_logs ADD COLUMN ssl_serial_number TEXT;
ALTER TABLE check_logs ADD COLUMN caa_status TEXT;
ALTER TABLE check_logs ADD COLUMN caa_details TEXT;
