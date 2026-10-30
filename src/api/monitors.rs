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

// ── Handlers ─────────────────────────────────────────────────────────────────

/// `GET /api/v1/monitors` — list all monitors.
pub async fn list(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<MonitorRow>>, AppError> {
    let rows = sqlx::query_as::<_, MonitorRow>(
        "SELECT id, name, url, check_interval_secs, timeout_secs, max_retries, \
               is_active, is_public, last_status, down_since, created_at \
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
               is_active, is_public, last_status, down_since, created_at \
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
    let id = uuid::Uuid::new_v4().to_string();

    sqlx::query(
        "INSERT INTO monitors (id, name, url, check_interval_secs, timeout_secs, \
                               max_retries, is_active, is_public, last_status, down_since) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'UP', NULL)",
    )
    .bind(&id)
    .bind(&body.name)
    .bind(&body.url)
    .bind(body.check_interval_secs)
    .bind(body.timeout_secs)
    .bind(body.max_retries)
    .bind(body.is_active as i32)
    .bind(body.is_public as i32)
    .execute(&state.db)
    .await?;

    let row = sqlx::query_as::<_, MonitorRow>(
        "SELECT id, name, url, check_interval_secs, timeout_secs, max_retries, \
               is_active, is_public, last_status, down_since, created_at \
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
