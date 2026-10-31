-- Migration 008: Add response_time_ms column to check_logs and keep latency_ms synchronized
ALTER TABLE check_logs ADD COLUMN response_time_ms INTEGER;
UPDATE check_logs SET response_time_ms = latency_ms WHERE response_time_ms IS NULL;

CREATE TRIGGER IF NOT EXISTS trg_sync_check_logs_resp_time
AFTER INSERT ON check_logs
FOR EACH ROW
WHEN NEW.response_time_ms IS NULL AND NEW.latency_ms IS NOT NULL
BEGIN
    UPDATE check_logs SET response_time_ms = NEW.latency_ms WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_sync_check_logs_latency
AFTER INSERT ON check_logs
FOR EACH ROW
WHEN NEW.latency_ms IS NULL AND NEW.response_time_ms IS NOT NULL
BEGIN
    UPDATE check_logs SET latency_ms = NEW.response_time_ms WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_sync_check_logs_created_at
AFTER INSERT ON check_logs
FOR EACH ROW
WHEN NEW.created_at IS NULL AND NEW.checked_at IS NOT NULL
BEGIN
    UPDATE check_logs SET created_at = NEW.checked_at WHERE id = NEW.id;
END;
