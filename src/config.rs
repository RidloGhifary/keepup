use std::env;

pub const INSECURE_PLACEHOLDER_JWT_SECRET: &str = "change-this-in-production-keepup-jwt-secret";

fn parse_bool_env(key: &str, default: bool) -> bool {
    match env::var(key) {
        Ok(val) => matches!(val.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"),
        Err(_) => default,
    }
}

/// Application-level configuration loaded from environment variables.
#[derive(Debug, Clone)]
pub struct Config {
    /// Port the HTTP server binds to (default: 8080).
    pub port: u16,
    /// File path for the SQLite database (default: ./data/keepup.db).
    pub database_url: String,
    /// Administrative username for dashboard access (default: admin).
    pub admin_user: String,
    /// Administrative password for dashboard access.
    pub admin_password: String,
    /// Whether `ADMIN_PASSWORD` was explicitly supplied with a non-default value.
    pub admin_password_from_env: bool,
    /// Secret key used to sign and verify session JWTs.
    pub jwt_secret: String,
    /// Whether `JWT_SECRET` was explicitly supplied with a non-placeholder value.
    pub jwt_secret_from_env: bool,
    /// Allow outbound probes/webhooks to private/loopback IPs (`ALLOW_PRIVATE_TARGETS`, default: false).
    pub allow_private_targets: bool,
    /// Trust `X-Forwarded-For` / `X-Real-IP` headers for rate limiting (`TRUST_PROXY_HEADERS`, default: false).
    pub trust_proxy_headers: bool,
    /// Explicit CORS allowed origins (`CORS_ALLOWED_ORIGINS`, comma-separated, default: empty).
    pub cors_allowed_origins: Vec<String>,
    /// Days to retain `check_logs` history before automatic pruning (`KEEPUP_RETENTION_DAYS`, default: 90).
    pub retention_days: u32,
    /// Optional custom logo URL for public status pages (`CUSTOM_LOGO_URL`).
    pub custom_logo_url: Option<String>,
    /// Optional bearer token protecting `GET /metrics` (`METRICS_BEARER_TOKEN`).
    pub metrics_bearer_token: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port: 8080,
            database_url: ":memory:".to_string(),
            admin_user: "admin".to_string(),
            admin_password: "admin".to_string(),
            admin_password_from_env: false,
            jwt_secret: "test-secret-at-least-32-bytes-long-for-security".to_string(),
            jwt_secret_from_env: false,
            allow_private_targets: false,
            trust_proxy_headers: false,
            cors_allowed_origins: vec![],
            retention_days: 90,
            custom_logo_url: None,
            metrics_bearer_token: None,
        }
    }
}

impl Config {
    /// Build a `Config` from environment variables, falling back to safe defaults.
    pub fn from_env() -> Self {
        let port = env::var("PORT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(8080);

        let database_url = env::var("KEEPUP_DB_PATH")
            .or_else(|_| env::var("RUSTPULSE_DB_PATH"))
            .or_else(|_| {
                env::var("DATABASE_URL").map(|url| {
                    url.strip_prefix("sqlite://")
                        .unwrap_or(&url)
                        .split('?')
                        .next()
                        .unwrap_or(&url)
                        .to_string()
                })
            })
            .unwrap_or_else(|_| "./data/keepup.db".to_string());

        let admin_user = env::var("ADMIN_USER")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "admin".to_string());

        let (admin_password, admin_password_from_env) = match env::var("ADMIN_PASSWORD") {
            Ok(val) if !val.is_empty() && val != "admin" => (val, true),
            Ok(val) if !val.is_empty() => (val, false),
            _ => ("admin".to_string(), false),
        };

        let (jwt_secret, jwt_secret_from_env) = match env::var("JWT_SECRET") {
            Ok(val) if !val.trim().is_empty() && val.trim() != INSECURE_PLACEHOLDER_JWT_SECRET => {
                (val, true)
            }
            _ => (
                format!(
                    "{}{}",
                    uuid::Uuid::new_v4().simple(),
                    uuid::Uuid::new_v4().simple()
                ),
                false,
            ),
        };

        let allow_private_targets = parse_bool_env("ALLOW_PRIVATE_TARGETS", false);
        let trust_proxy_headers = parse_bool_env("TRUST_PROXY_HEADERS", false);

        let cors_allowed_origins = env::var("CORS_ALLOWED_ORIGINS")
            .ok()
            .map(|raw| {
                raw.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        let retention_days = env::var("KEEPUP_RETENTION_DAYS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|&d| d > 0)
            .unwrap_or(90);

        let custom_logo_url = env::var("CUSTOM_LOGO_URL")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let metrics_bearer_token = env::var("METRICS_BEARER_TOKEN")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        Self {
            port,
            database_url,
            admin_user,
            admin_password,
            admin_password_from_env,
            jwt_secret,
            jwt_secret_from_env,
            allow_private_targets,
            trust_proxy_headers,
            cors_allowed_origins,
            retention_days,
            custom_logo_url,
            metrics_bearer_token,
        }
    }
}
