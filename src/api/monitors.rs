use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;

pub use crate::db::monitors::Monitor as MonitorRow;
use crate::error::AppError;
use crate::AppState;

// ── Request types ────────────────────────────────────────────────────────────

/// Body for creating a new monitor.
#[derive(Debug, Deserialize)]
pub struct CreateMonitor {
    pub name: String,
    pub url: String,
    #[serde(default = "default_interval")]
    pub check_interval_secs: i32,
    #[serde(default = "default_timeout")]
    pub timeout_secs: i32,
    #[serde(default = "default_retries")]
    pub max_retries: i32,
    #[serde(default = "default_true")]
    pub is_active: bool,
    #[serde(default = "default_true")]
    pub is_public: bool,
    #[serde(default)]
    pub check_domain_expiry: bool,
    #[serde(default = "default_warning_days")]
    pub domain_warning_days: i32,
    #[serde(default)]
    pub check_ssl: bool,
    #[serde(default = "default_warning_days")]
    pub ssl_warning_days: i32,
    #[serde(default)]
    pub channel_ids: Option<Vec<String>>,
}

fn default_interval() -> i32 {
    60
}
fn default_timeout() -> i32 {
    5
}
fn default_retries() -> i32 {
    3
}
fn default_true() -> bool {
    true
}
fn default_warning_days() -> i32 {
    30
}

impl CreateMonitor {
    pub fn validate(&self) -> Result<(), AppError> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err(AppError::BadRequest("Monitor name cannot be empty".to_string()));
        }
        let url = self.url.trim();
        if url.is_empty() {
            return Err(AppError::BadRequest("Monitor URL cannot be empty".to_string()));
        }
        if !url.starts_with("http://")
            && !url.starts_with("https://")
            && !url.starts_with("tcp://")
            && !url.starts_with("dns://")
            && !url.starts_with("push://")
        {
            return Err(AppError::BadRequest(
                "Unsupported URL scheme: must start with http://, https://, tcp://, dns://, or push://"
                    .to_string(),
            ));
        }
        if !(5..=86400).contains(&self.check_interval_secs) {
            return Err(AppError::BadRequest(
                "check_interval_secs must be between 5 and 86400".to_string(),
            ));
        }
        if !(1..=60).contains(&self.timeout_secs) {
            return Err(AppError::BadRequest(
                "timeout_secs must be between 1 and 60".to_string(),
            ));
        }
        if !(1..=10).contains(&self.max_retries) {
            return Err(AppError::BadRequest(
                "max_retries must be between 1 and 10".to_string(),
            ));
        }
        if !(1..=365).contains(&self.domain_warning_days) {
            return Err(AppError::BadRequest(
                "domain_warning_days must be between 1 and 365".to_string(),
            ));
        }
        if !(1..=365).contains(&self.ssl_warning_days) {
            return Err(AppError::BadRequest(
                "ssl_warning_days must be between 1 and 365".to_string(),
            ));
        }
        Ok(())
    }
}

// ── Handlers ─────────────────────────────────────────────────────────────────

/// `GET /api/v1/monitors` — list all monitors.
pub async fn list(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<MonitorRow>>, AppError> {
    let rows = sqlx::query_as::<_, MonitorRow>(
        "SELECT id, name, url, check_interval_secs, timeout_secs, max_retries, \
               is_active, is_public, last_status, down_since, created_at, \
               check_domain_expiry, domain_warning_days, domain_expires_at, \
               domain_days_remaining, last_domain_alert_days, domain_last_checked_at, \
               check_ssl, ssl_warning_days, ssl_expires_at, ssl_days_remaining, ssl_last_checked_at \
         FROM monitors ORDER BY created_at DESC",
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(rows))
}

/// `GET /api/v1/monitors/:id` — get a single monitor by ID.
pub async fn get_one(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<MonitorRow>, AppError> {
    let row = sqlx::query_as::<_, MonitorRow>(
        "SELECT id, name, url, check_interval_secs, timeout_secs, max_retries, \
               is_active, is_public, last_status, down_since, created_at, \
               check_domain_expiry, domain_warning_days, domain_expires_at, \
               domain_days_remaining, last_domain_alert_days, domain_last_checked_at, \
               check_ssl, ssl_warning_days, ssl_expires_at, ssl_days_remaining, ssl_last_checked_at \
         FROM monitors WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("Monitor {id} not found")))?;

    Ok(Json(row))
}

/// `POST /api/v1/monitors` — create a new monitor.
pub async fn create(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateMonitor>,
) -> Result<(StatusCode, Json<MonitorRow>), AppError> {
    body.validate()?;

    let id = uuid::Uuid::new_v4().to_string();

    let mut tx = state.db.begin().await?;

    sqlx::query(
        "INSERT INTO monitors (id, name, url, check_interval_secs, timeout_secs, \
                               max_retries, is_active, is_public, last_status, down_since, \
                               check_domain_expiry, domain_warning_days, check_ssl, ssl_warning_days) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'UP', NULL, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&body.name)
    .bind(&body.url)
    .bind(body.check_interval_secs)
    .bind(body.timeout_secs)
    .bind(body.max_retries)
    .bind(body.is_active as i32)
    .bind(body.is_public as i32)
    .bind(body.check_domain_expiry as i32)
    .bind(body.domain_warning_days)
    .bind(body.check_ssl as i32)
    .bind(body.ssl_warning_days)
    .execute(&mut *tx)
    .await?;

    // Bind notification channels: explicitly selected or defaults if omitted
    let channel_ids_to_assign = match body.channel_ids {
        Some(ref ids) => ids.clone(),
        None => {
            sqlx::query_as::<_, (String,)>(
                "SELECT id FROM notification_channels WHERE is_default = 1",
            )
            .fetch_all(&mut *tx)
            .await?
            .into_iter()
            .map(|r| r.0)
            .collect()
        }
    };

    let mut seen = std::collections::HashSet::new();
    for chan_id in channel_ids_to_assign {
        if seen.insert(chan_id.clone()) {
            sqlx::query(
                "INSERT INTO monitor_notification_channels (monitor_id, channel_id) VALUES (?, ?)",
            )
            .bind(&id)
            .bind(&chan_id)
            .execute(&mut *tx)
            .await?;
        }
    }

    let row = sqlx::query_as::<_, MonitorRow>(
        "SELECT id, name, url, check_interval_secs, timeout_secs, max_retries, \
               is_active, is_public, last_status, down_since, created_at, \
               check_domain_expiry, domain_warning_days, domain_expires_at, \
               domain_days_remaining, last_domain_alert_days, domain_last_checked_at, \
               check_ssl, ssl_warning_days, ssl_expires_at, ssl_days_remaining, ssl_last_checked_at \
         FROM monitors WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    state.invalidate_status_cache().await;

    Ok((StatusCode::CREATED, Json(row)))
}

/// `DELETE /api/v1/monitors/:id` — delete a monitor and its logs.
pub async fn delete(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let mut tx = state.db.begin().await?;

    let result = sqlx::query("DELETE FROM monitors WHERE id = ?")
        .bind(&id)
        .execute(&mut *tx)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("Monitor {id} not found")));
    }

    sqlx::query("DELETE FROM check_logs WHERE monitor_id = ?")
        .bind(&id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    state.invalidate_status_cache().await;

    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_create_monitor_rejects_invalid_bounds() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();

        let config = crate::config::Config::default();
        let token = crate::api::auth::create_jwt(
            &config.admin_user,
            &config.jwt_secret,
            1,
        )
        .unwrap();

        let state = crate::AppState::new_for_test(pool, config);
        let app = crate::build_http_app(state);

        // 1. check_interval_secs < 5 must return 400
        let req = Request::builder()
            .method("POST")
            .uri("/api/monitors")
            .header("authorization", format!("Bearer {token}"))
            .header("content-type", "application/json")
            .body(Body::from(r#"{"name":"test","url":"https://example.com","check_interval_secs":0}"#))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);

        // 2. timeout_secs < 1 or > 60 must return 400
        let req = Request::builder()
            .method("POST")
            .uri("/api/monitors")
            .header("authorization", format!("Bearer {token}"))
            .header("content-type", "application/json")
            .body(Body::from(r#"{"name":"test","url":"https://example.com","timeout_secs":-1}"#))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);

        // 3. invalid scheme must return 400
        let req = Request::builder()
            .method("POST")
            .uri("/api/monitors")
            .header("authorization", format!("Bearer {token}"))
            .header("content-type", "application/json")
            .body(Body::from(r#"{"name":"test","url":"ftp://example.com"}"#))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }
}
