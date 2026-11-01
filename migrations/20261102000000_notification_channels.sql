-- Centralized Notification Channels & Monitor Mappings
CREATE TABLE IF NOT EXISTS notification_channels (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    type TEXT NOT NULL, -- 'discord', 'slack', 'webhook', 'telegram'
    url TEXT NOT NULL,
    is_default BOOLEAN NOT NULL DEFAULT 0, -- Auto-assign to new monitors
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS monitor_notification_channels (
    monitor_id TEXT NOT NULL,
    channel_id TEXT NOT NULL,
    PRIMARY KEY (monitor_id, channel_id),
    FOREIGN KEY (monitor_id) REFERENCES monitors(id) ON DELETE CASCADE,
    FOREIGN KEY (channel_id) REFERENCES notification_channels(id) ON DELETE CASCADE
);
