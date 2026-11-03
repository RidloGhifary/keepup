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
    #[serde(default = "default_check_type")]
    pub check_type: String,
    #[serde(default)]
    pub port: Option<i32>,
    #[serde(default)]
    pub dns_record_type: Option<String>,
    #[serde(default)]
    pub dns_expected_value: Option<String>,
}

fn default_check_type() -> String {
    "http".to_string()
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
               check_ssl, ssl_warning_days, ssl_expires_at, ssl_days_remaining, ssl_last_checked_at, \
               check_type, port, dns_record_type, dns_expected_value \
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
               check_ssl, ssl_warning_days, ssl_expires_at, ssl_days_remaining, ssl_last_checked_at, \
               check_type, port, dns_record_type, dns_expected_value \
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
    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest("Monitor name cannot be empty".to_string()));
    }
    let check_type = body.check_type.trim().to_lowercase();
    if check_type != "http" && check_type != "tcp" && check_type != "dns" && check_type != "ping" {
        return Err(AppError::BadRequest(format!(
            "Invalid check_type '{check_type}'. Allowed: http, tcp, dns, ping"
        )));
    }
    let u = body.url.trim();
    if u.is_empty() {
        return Err(AppError::BadRequest("Target URL/host cannot be empty".to_string()));
    }
    if check_type == "http" && !u.starts_with("http://") && !u.starts_with("https://") {
        return Err(AppError::BadRequest(
            "HTTP Monitor URL must start with http:// or https://".to_string(),
        ));
    }
    if let Some(port) = body.port {
        if !(1..=65535).contains(&port) {
            return Err(AppError::BadRequest("TCP port must be between 1 and 65535".to_string()));
        }
    }
    if let Some(ref rtype) = body.dns_record_type {
        let rt = rtype.trim().to_uppercase();
        if rt != "A" && rt != "AAAA" && rt != "CNAME" && rt != "TXT" && rt != "MX" {
            return Err(AppError::BadRequest(format!(
                "Invalid dns_record_type '{rtype}'. Allowed: A, AAAA, CNAME, TXT, MX"
            )));
        }
    }

    let id = uuid::Uuid::new_v4().to_string();

    sqlx::query(
        "INSERT INTO monitors (id, name, url, check_interval_secs, timeout_secs, \
                               max_retries, is_active, is_public, last_status, down_since, \
                               check_domain_expiry, domain_warning_days, check_ssl, ssl_warning_days, \
                               check_type, port, dns_record_type, dns_expected_value) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'UP', NULL, ?, ?, ?, ?, ?, ?, ?, ?)",
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
    .bind(&check_type)
    .bind(body.port)
    .bind(&body.dns_record_type)
    .bind(&body.dns_expected_value)
    .execute(&state.db)
    .await?;

    // Bind notification channels: explicitly selected or defaults if omitted
    let channel_ids_to_assign = match body.channel_ids {
        Some(ref ids) => ids.clone(),
        None => {
            crate::db::notifications::list_channels(&state.db)
                .await
                .unwrap_or_default()
                .into_iter()
                .filter(|c| c.is_default)
                .map(|c| c.id)
                .collect()
        }
    };
    if !channel_ids_to_assign.is_empty() {
        let _ = crate::db::notifications::set_monitor_channels(&state.db, &id, &channel_ids_to_assign).await;
    }

    let row = sqlx::query_as::<_, MonitorRow>(
        "SELECT id, name, url, check_interval_secs, timeout_secs, max_retries, \
               is_active, is_public, last_status, down_since, created_at, \
               check_domain_expiry, domain_warning_days, domain_expires_at, \
               domain_days_remaining, last_domain_alert_days, domain_last_checked_at, \
               check_ssl, ssl_warning_days, ssl_expires_at, ssl_days_remaining, ssl_last_checked_at, \
               check_type, port, dns_record_type, dns_expected_value \
         FROM monitors WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(&state.db)
    .await?;

    Ok((StatusCode::CREATED, Json(row)))
}

/// `DELETE /api/v1/monitors/:id` — delete a monitor and its logs.
pub async fn delete(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let result = sqlx::query("DELETE FROM monitors WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("Monitor {id} not found")));
    }

    sqlx::query("DELETE FROM check_logs WHERE monitor_id = ?")
        .bind(&id)
        .execute(&state.db)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}
