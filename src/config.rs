use std::env;

/// Application-level configuration loaded from environment variables.
#[derive(Debug, Clone)]
pub struct Config {
    /// Port the HTTP server binds to (default: 8080).
    pub port: u16,
    /// File path for the SQLite database (default: ./data/keepup.db).
    pub database_url: String,
    /// Administrative username for dashboard access (default: admin).
    pub admin_user: String,
    /// Administrative password for dashboard access (default: admin).
    pub admin_password: String,
    /// Secret key used to sign and verify session JWTs.
    pub jwt_secret: String,
    /// Days to retain check telemetry logs before automated background purge (default: 90).
    pub data_retention_days: u32,
}

impl Config {
    /// Build a `Config` from environment variables, falling back to sensible
    /// defaults when a variable is absent.
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

        let admin_user = env::var("ADMIN_USER").unwrap_or_else(|_| "admin".to_string());
        let admin_password = env::var("ADMIN_PASSWORD").unwrap_or_else(|_| "admin".to_string());
        let jwt_secret = env::var("JWT_SECRET").unwrap_or_else(|_| {
            format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple())
        });
        let data_retention_days = env::var("DATA_RETENTION_DAYS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(90);

        Self {
            port,
            database_url,
            admin_user,
            admin_password,
            jwt_secret,
            data_retention_days,
        }
    }
}
