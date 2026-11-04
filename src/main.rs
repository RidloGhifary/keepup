mod alerting;
mod api;
mod banner;
mod checker;
mod config;
mod db;
mod error;
mod middleware;
mod notifier;
mod ui;

use std::sync::Arc;

use axum::http::{header, Method};
use reqwest::Client;
use sqlx::SqlitePool;
use tokio::net::TcpListener;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::config::Config;

/// Shared application state threaded through Axum handlers.
#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub write_db: SqlitePool,
    pub config: Config,
    pub http_client: Client,
    pub reload_tx: tokio::sync::broadcast::Sender<String>,
    pub status_cache: crate::api::public::PublicStatusCache,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Install ring crypto provider for rustls
    let _ = tokio_rustls::rustls::crypto::ring::default_provider().install_default();

    // Load .env file (optional – not an error if missing).
    dotenvy::dotenv().ok();

    // Initialise structured logging.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,keepup=debug")),
        )
        .init();

    // Load configuration from environment variables.
    let config = Config::from_env();
    banner::print_banner(env!("CARGO_PKG_VERSION"), config.port, &config.database_url);
    info!(port = config.port, db = %config.database_url, "Starting KeepUp");

    // Initialise SQLite dual pools (read_pool: 10 conns, write_pool: 1 conn) and run migrations.
    let (read_pool, write_pool) = db::init_pools(&config.database_url).await?;
    db::run_migrations(&write_pool).await?;

    // Create broadcast channel for monitor reload notifications.
    let (reload_tx, reload_rx) = tokio::sync::broadcast::channel::<String>(100);

    // Build shared HTTP client for health checks.
    let http_client = Client::builder()
        .user_agent("KeepUp-Monitor/1.0")
        .build()?;

    let status_cache = crate::api::public::PublicStatusCache::default();

    // Build shared state.
    let state = Arc::new(AppState {
        db: read_pool.clone(),
        write_db: write_pool.clone(),
        config: config.clone(),
        http_client: http_client.clone(),
        reload_tx,
        status_cache: status_cache.clone(),
    });

    // Spawn the background health-check scheduler with reload receiver.
    tokio::spawn(checker::scheduler::run_scheduler(
        read_pool,
        write_pool,
        http_client,
        reload_rx,
        config.data_retention_days,
        status_cache,
    ));

    // Configure CORS with credentials support for localhost and local origins.
    let cors = CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::PATCH,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::CONTENT_TYPE,
            header::AUTHORIZATION,
            header::ACCEPT,
            header::COOKIE,
        ])
        .allow_origin(AllowOrigin::predicate(|origin, _| {
            let origin_bytes = origin.as_bytes();
            origin_bytes.starts_with(b"http://localhost")
                || origin_bytes.starts_with(b"http://127.0.0.1")
                || origin_bytes.starts_with(b"https://localhost")
                || origin_bytes.starts_with(b"https://127.0.0.1")
        }))
        .allow_credentials(true);

    // Build Axum router with explicit status routes, API routes and embedded UI fallback.
    let app = axum::Router::new()
        .route("/robots.txt", axum::routing::get(api::public::robots_txt))
        .route("/status", axum::routing::get(ui::status_page_handler))
        .route("/status/*path", axum::routing::get(ui::status_page_handler))
        .route("/badge/:id", axum::routing::get(api::public::badge))
        .merge(api::router(state.clone()))
        .fallback(ui::static_handler)
        .layer(tower_http::compression::CompressionLayer::new())
        .layer(cors)
        .with_state(state);

    // Bind and serve with connection info for client IP rate limiting.
    let addr = format!("0.0.0.0:{}", config.port);
    let listener = TcpListener::bind(&addr).await?;
    info!("Listening on {}", addr);
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;

    Ok(())
}
