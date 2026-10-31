-- Maintenance Windows & Target Monitor Junction Table
CREATE TABLE IF NOT EXISTS maintenance_windows (
    id TEXT PRIMARY KEY NOT NULL,
    title TEXT NOT NULL,
    description TEXT,
    start_time TIMESTAMP NOT NULL,
    end_time TIMESTAMP NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS maintenance_monitors (
    maintenance_id TEXT NOT NULL,
    monitor_id TEXT NOT NULL,
    PRIMARY KEY (maintenance_id, monitor_id),
    FOREIGN KEY (maintenance_id) REFERENCES maintenance_windows(id) ON DELETE CASCADE,
    FOREIGN KEY (monitor_id) REFERENCES monitors(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_mw_time ON maintenance_windows(start_time, end_time);
CREATE INDEX IF NOT EXISTS idx_mm_mon ON maintenance_monitors(monitor_id);
