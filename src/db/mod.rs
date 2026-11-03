pub mod checks;
pub mod incidents;
pub mod maintenance;
pub mod monitors;
pub mod notifications;
pub mod status_pages;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{ConnectOptions, SqlitePool};
use std::str::FromStr;
use std::time::Duration;
use tracing::info;

/// Initialise the SQLite connection pool with WAL mode, foreign keys, busy timeout,
/// bounded page cache, and `0600` file permissions on Unix.
pub async fn init_pool(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    let is_memory = database_url == ":memory:" || database_url == "sqlite::memory:";

    if !is_memory {
        // Ensure the parent directory exists so SQLite can create the file.
        if let Some(parent) = std::path::Path::new(database_url).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).ok();
            }
        }
    }

    let uri = if database_url.starts_with("sqlite:") {
        database_url.to_string()
    } else {
        format!("sqlite://{database_url}")
    };

    let mut connect_options = SqliteConnectOptions::from_str(&uri)?
        .create_if_missing(true)
        .busy_timeout(Duration::from_secs(5))
        .foreign_keys(true)
        .pragma("cache_size", "-16000")
        .log_statements(tracing::log::LevelFilter::Debug);

    if !is_memory {
        connect_options = connect_options
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .synchronous(sqlx::sqlite::SqliteSynchronous::Normal);
    }

    let max_conns = if is_memory { 1 } else { 8 };
    let pool = SqlitePoolOptions::new()
        .max_connections(max_conns)
        .connect_with(connect_options)
        .await?;

    #[cfg(unix)]
    if !is_memory {
        use std::os::unix::fs::PermissionsExt;
        let db_path = database_url.trim_start_matches("sqlite://");
        for suffix in ["", "-wal", "-shm"] {
            let p = format!("{db_path}{suffix}");
            if let Ok(meta) = std::fs::metadata(&p) {
                let mut perms = meta.permissions();
                perms.set_mode(0o600);
                let _ = std::fs::set_permissions(&p, perms);
            }
        }
    }

    info!("SQLite pool initialised (WAL + foreign_keys=ON)");
    Ok(pool)
}

/// Run embedded sqlx migrations against the given pool.
pub async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!().run(pool).await?;
    info!("Database migrations applied");
    Ok(())
}
