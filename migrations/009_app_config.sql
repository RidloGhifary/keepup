-- Dynamic application configuration storage (e.g. admin password hashes)
CREATE TABLE IF NOT EXISTS app_config (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);
