-- Expand notification_channels table with health and failure tracking columns
ALTER TABLE notification_channels ADD COLUMN last_error TEXT;
ALTER TABLE notification_channels ADD COLUMN last_error_at TIMESTAMP;

-- Audit log table for alert notification dispatch failures
CREATE TABLE IF NOT EXISTS notification_failure_logs (
    id TEXT PRIMARY KEY NOT NULL,
    channel_id TEXT NOT NULL,
    channel_name TEXT NOT NULL,
    channel_type TEXT NOT NULL,
    error_message TEXT NOT NULL,
    status_code INTEGER,
    attempt_count INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (channel_id) REFERENCES notification_channels(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_notification_failure_logs_channel_id ON notification_failure_logs(channel_id);
CREATE INDEX IF NOT EXISTS idx_notification_failure_logs_created_at ON notification_failure_logs(created_at);
