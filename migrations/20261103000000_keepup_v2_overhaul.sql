-- KeepUp v2 Overhaul Migration
-- 1. Extend notification_channels with legacy webhook capabilities (enabled, template, resolve toggle)
ALTER TABLE notification_channels ADD COLUMN is_enabled INTEGER NOT NULL DEFAULT 1;
ALTER TABLE notification_channels ADD COLUMN use_default_template INTEGER NOT NULL DEFAULT 1;
ALTER TABLE notification_channels ADD COLUMN notify_on_resolve INTEGER NOT NULL DEFAULT 1;
ALTER TABLE notification_channels ADD COLUMN custom_template TEXT;

-- 2. Migrate any rows from legacy webhooks & monitor_webhooks into notification_channels
INSERT OR IGNORE INTO notification_channels (
    id, name, type, url, is_default, is_enabled, use_default_template, notify_on_resolve, custom_template, created_at
)
SELECT
    w.id,
    w.name,
    CASE
        WHEN LOWER(w.provider) IN ('generic', 'generic_webhook') THEN 'webhook'
        ELSE LOWER(w.provider)
    END,
    w.url,
    CASE
        WHEN EXISTS (SELECT 1 FROM monitor_webhooks mw WHERE mw.webhook_id = w.id) THEN 0
        ELSE 1
    END,
    w.is_enabled,
    w.use_default_template,
    w.notify_on_resolve,
    w.message_template,
    w.created_at
FROM webhooks w;

INSERT OR IGNORE INTO monitor_notification_channels (monitor_id, channel_id)
SELECT monitor_id, webhook_id FROM monitor_webhooks;

DROP TABLE IF EXISTS monitor_webhooks;
DROP TABLE IF EXISTS webhooks;

-- 3. Add v2 probe, alerting, SSL trust/milestone, push heartbeat, and DNS columns to monitors
ALTER TABLE monitors ADD COLUMN last_ssl_alert_days INTEGER;
ALTER TABLE monitors ADD COLUMN ssl_trusted INTEGER;
ALTER TABLE monitors ADD COLUMN http_method TEXT NOT NULL DEFAULT 'GET';
ALTER TABLE monitors ADD COLUMN expected_status TEXT NOT NULL DEFAULT '200-299';
ALTER TABLE monitors ADD COLUMN request_headers TEXT;
ALTER TABLE monitors ADD COLUMN body_keyword TEXT;
ALTER TABLE monitors ADD COLUMN json_pointer TEXT;
ALTER TABLE monitors ADD COLUMN json_expected TEXT;
ALTER TABLE monitors ADD COLUMN degraded_latency_ms INTEGER;
ALTER TABLE monitors ADD COLUMN push_token TEXT;
ALTER TABLE monitors ADD COLUMN grace_secs INTEGER NOT NULL DEFAULT 60;
ALTER TABLE monitors ADD COLUMN dns_record_type TEXT NOT NULL DEFAULT 'A';
ALTER TABLE monitors ADD COLUMN dns_expected TEXT;
ALTER TABLE monitors ADD COLUMN resend_interval_mins INTEGER NOT NULL DEFAULT 0;
ALTER TABLE monitors ADD COLUMN last_alerted_at DATETIME;

-- 4. Create incidents and incident_updates tables for Incident Timeline & Postmortems (N2)
CREATE TABLE IF NOT EXISTS incidents (
    id TEXT PRIMARY KEY NOT NULL,
    title TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'Investigating',
    severity TEXT NOT NULL DEFAULT 'major',
    monitor_id TEXT,
    status_page_slug TEXT,
    is_auto INTEGER NOT NULL DEFAULT 0,
    created_at DATETIME NOT NULL,
    updated_at DATETIME NOT NULL,
    resolved_at DATETIME,
    FOREIGN KEY (monitor_id) REFERENCES monitors(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS incident_updates (
    id TEXT PRIMARY KEY NOT NULL,
    incident_id TEXT NOT NULL,
    status TEXT NOT NULL,
    message TEXT NOT NULL,
    created_at DATETIME NOT NULL,
    FOREIGN KEY (incident_id) REFERENCES incidents(id) ON DELETE CASCADE
);

-- 5. Add missing secondary indexes for query scaling (#10)
CREATE INDEX IF NOT EXISTS idx_spm_slug_lower ON status_page_monitors(LOWER(status_page_slug));
CREATE INDEX IF NOT EXISTS idx_mnc_channel ON monitor_notification_channels(channel_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_monitors_push_token ON monitors(push_token) WHERE push_token IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_incidents_slug_created ON incidents(status_page_slug, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_incident_updates_incident ON incident_updates(incident_id, created_at ASC);
