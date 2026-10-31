CREATE TABLE IF NOT EXISTS status_pages (
    slug TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    description TEXT,
    is_public BOOLEAN DEFAULT 1,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS status_page_monitors (
    status_page_slug TEXT NOT NULL,
    monitor_id TEXT NOT NULL,
    group_name TEXT NOT NULL DEFAULT 'Services',
    display_order INTEGER DEFAULT 0,
    PRIMARY KEY (status_page_slug, monitor_id),
    FOREIGN KEY (status_page_slug) REFERENCES status_pages(slug) ON DELETE CASCADE,
    FOREIGN KEY (monitor_id) REFERENCES monitors(id) ON DELETE CASCADE
);

INSERT OR IGNORE INTO status_pages (slug, title, description, is_public) 
VALUES ('default', 'System Status', 'Current status and real-time performance of public services', 1);
