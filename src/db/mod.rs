pub mod checks;
pub mod maintenance;
pub mod monitors;
pub mod notifications;
pub mod status_pages;
pub mod webhooks;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{ConnectOptions, SqlitePool};
use std::str::FromStr;
use std::time::Duration;
use tracing::info;

/// Build optimized SQLite connection options with WAL, NORMAL sync, generous cache,
/// memory-mapped I/O, memory temp store, and busy timeouts.
pub fn build_connect_options(database_url: &str) -> Result<SqliteConnectOptions, sqlx::Error> {
    if let Some(parent) = std::path::Path::new(database_url).parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let opts = SqliteConnectOptions::from_str(&format!("sqlite://{database_url}"))?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
        .busy_timeout(Duration::from_millis(5000))
        .foreign_keys(true)
        .pragma("cache_size", "-64000") // 64 MB page cache
        .pragma("mmap_size", "268435456") // 256 MB memory-mapped I/O
        .pragma("temp_store", "2") // Memory temp store
        .log_statements(tracing::log::LevelFilter::Debug);

    Ok(opts)
}

/// Initialise dual connection pools:
/// - `read_pool`: 10 connections for concurrent public status queries & dashboard reads.
/// - `write_pool`: 1 dedicated connection for probe telemetry writes and mutations (guaranteeing zero write-write contention).
pub async fn init_pools(database_url: &str) -> Result<(SqlitePool, SqlitePool), sqlx::Error> {
    let connect_options = build_connect_options(database_url)?;

    let read_pool = SqlitePoolOptions::new()
        .max_connections(10)
        .connect_with(connect_options.clone())
        .await?;

    let write_pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(connect_options)
        .await?;

    info!("SQLite dual pools initialised (WAL mode, read: 10 conns, write: 1 conn)");
    Ok((read_pool, write_pool))
}

/// Initialise the SQLite connection pool with optimized PRAGMAs.
#[allow(dead_code)]
pub async fn init_pool(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    let (read_pool, _) = init_pools(database_url).await?;
    Ok(read_pool)
}

/// Run embedded sqlx migrations against the given pool.
pub async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!().run(pool).await?;
    info!("Database migrations applied");
    Ok(())
}
