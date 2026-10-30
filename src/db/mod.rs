pub mod monitors;
pub mod webhooks;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{ConnectOptions, SqlitePool};
use std::str::FromStr;
use tracing::info;

/// Initialise the SQLite connection pool with WAL mode enabled
/// and run all pending migrations.
pub async fn init_pool(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    // Ensure the parent directory exists so SQLite can create the file.
    if let Some(parent) = std::path::Path::new(database_url).parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let connect_options = SqliteConnectOptions::from_str(&format!("sqlite://{database_url}"))?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
        .log_statements(tracing::log::LevelFilter::Debug);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(connect_options)
        .await?;

    info!("SQLite pool initialised (WAL mode)");
    Ok(pool)
}

/// Run embedded sqlx migrations against the given pool.
pub async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!().run(pool).await?;
    info!("Database migrations applied");
    Ok(())
}
