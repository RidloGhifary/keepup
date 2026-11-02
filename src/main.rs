mod alerting;
mod api;
mod banner;
mod checker;
mod config;
mod db;
mod error;
mod middleware;
mod net_guard;
mod ui;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use axum::extract::DefaultBodyLimit;
use axum::http::{header, HeaderName, HeaderValue, Method};
use reqwest::Client;
use sqlx::SqlitePool;
use tokio::net::TcpListener;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::set_header::SetResponseHeaderLayer;
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
    pub session_version: Arc<AtomicU64>,
    pub status_cache: Arc<
        tokio::sync::RwLock<
            std::collections::HashMap<String, (std::time::Instant, crate::api::public::PublicStatus)>,
        >,
    >,
}

impl AppState {
    pub fn current_session_version(&self) -> u64 {
        self.session_version.load(Ordering::SeqCst)
    }

    pub async fn invalidate_status_cache(&self) {
        let mut cache = self.status_cache.write().await;
        cache.clear();
    }

    #[cfg(test)]
    pub fn new_for_test(db: SqlitePool, config: Config) -> Arc<Self> {
        let (reload_tx, _) = tokio::sync::broadcast::channel(16);
        let http_client = net_guard::build_http_client(
            config.allow_private_targets,
            std::time::Duration::from_secs(5),
        )
        .expect("valid test http client");
        Arc::new(Self {
            db,
            config,
            http_client,
            reload_tx,
            session_version: Arc::new(AtomicU64::new(1)),
            status_cache: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        })
    }
}

/// Assemble the complete Axum HTTP application router with body size limits,
/// explicit-only CORS, gzip compression, and defensive HTTP security headers.
pub fn build_http_app(state: Arc<AppState>) -> axum::Router {
    let mut app = axum::Router::new()
        .route("/status", axum::routing::get(ui::status_page_handler))
        .route("/status/*path", axum::routing::get(ui::status_page_handler))
        .route("/badge/:id", axum::routing::get(api::public::badge))
        .merge(api::router(state.clone()))
        .fallback(ui::static_handler)
        .layer(DefaultBodyLimit::max(64 * 1024))
        .layer(CompressionLayer::new())
        .layer(SetResponseHeaderLayer::if_not_present(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::X_FRAME_OPTIONS,
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("referrer-policy"),
            HeaderValue::from_static("same-origin"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(
                "default-src 'self'; img-src 'self' https: data:; style-src 'self' 'unsafe-inline'; script-src 'self'; connect-src 'self'",
            ),
        ))
        .with_state(state.clone());

    if !state.config.cors_allowed_origins.is_empty() {
        let parsed_origins: Vec<HeaderValue> = state
            .config
            .cors_allowed_origins
            .iter()
            .filter_map(|o| HeaderValue::from_str(o).ok())
            .collect();
        if !parsed_origins.is_empty() {
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
                .allow_origin(parsed_origins)
                .allow_credentials(true);
            app = app.layer(cors);
        }
    }

    app
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
    let mut config = Config::from_env();
    banner::print_banner(env!("CARGO_PKG_VERSION"), config.port, &config.database_url);
    info!(port = config.port, db = %config.database_url, "Starting KeepUp");

    // Initialise SQLite pool (WAL mode) and run migrations.
    let pool = db::init_pool(&config.database_url).await?;
    db::run_migrations(&pool).await?;

    // Bootstrap persistent JWT secret, initial Argon2id admin password, and session_version.
    let initial_session_ver = api::auth::bootstrap_security_state(&pool, &mut config).await?;

    // Create broadcast channel for monitor reload notifications.
    let (reload_tx, reload_rx) = tokio::sync::broadcast::channel::<String>(100);

    // Build shared HTTP client with outbound SSRF guard for health checks & webhooks.
    let http_client = net_guard::build_http_client(
        config.allow_private_targets,
        std::time::Duration::from_secs(10),
    )?;

    // Build shared state.
    let state = Arc::new(AppState {
        db: pool.clone(),
        config: config.clone(),
        http_client: http_client.clone(),
        reload_tx,
        session_version: Arc::new(AtomicU64::new(initial_session_ver)),
        status_cache: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
    });

    // Spawn the background health-check scheduler with reload receiver.
    tokio::spawn(checker::scheduler::run_scheduler(
        pool,
        http_client,
        reload_rx,
        config.allow_private_targets,
    ));

    let app = build_http_app(state);

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
