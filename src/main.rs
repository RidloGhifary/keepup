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
    pub config: Config,
    pub http_client: Client,
    pub reload_tx: tokio::sync::broadcast::Sender<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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

    // Initialise SQLite pool (WAL mode) and run migrations.
    let pool = db::init_pool(&config.database_url).await?;
    db::run_migrations(&pool).await?;

    // Create broadcast channel for monitor reload notifications.
    let (reload_tx, reload_rx) = tokio::sync::broadcast::channel::<String>(100);

    // Build shared HTTP client for health checks.
    let http_client = Client::builder()
        .user_agent("KeepUp-Monitor/1.0")
        .build()?;

    // Build shared state.
    let state = Arc::new(AppState {
        db: pool.clone(),
        config: config.clone(),
        http_client: http_client.clone(),
        reload_tx,
    });

    // Spawn the background health-check scheduler with reload receiver.
    tokio::spawn(checker::scheduler::run_scheduler(pool, http_client, reload_rx));

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
        .route("/status", axum::routing::get(ui::status_page_handler))
        .route("/status/*path", axum::routing::get(ui::status_page_handler))
        .route("/badge/:id", axum::routing::get(api::public::badge))
        .merge(api::router(state.clone()))
        .fallback(ui::static_handler)
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
