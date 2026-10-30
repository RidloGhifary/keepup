-- Webhook / Notification Channels Table
CREATE TABLE IF NOT EXISTS webhooks (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    provider TEXT NOT NULL, -- 'discord', 'slack', or 'webhook'
    url TEXT NOT NULL,
    is_enabled INTEGER NOT NULL DEFAULT 1,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Optional: Link specific webhooks to specific monitors (Many-to-Many)
-- If left out, all enabled webhooks trigger for all monitor failures.
CREATE TABLE IF NOT EXISTS monitor_webhooks (
    monitor_id TEXT NOT NULL,
    webhook_id TEXT NOT NULL,
    PRIMARY KEY (monitor_id, webhook_id),
    FOREIGN KEY(monitor_id) REFERENCES monitors(id) ON DELETE CASCADE,
    FOREIGN KEY(webhook_id) REFERENCES webhooks(id) ON DELETE CASCADE
);