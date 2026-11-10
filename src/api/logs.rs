use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::AppState;

// ── Response types ───────────────────────────────────────────────────────────

/// A single check-log data point used by latency graphs.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct LogEntry {
    pub id: i64,
    pub monitor_id: String,
    pub status_code: Option<i32>,
    pub latency_ms: Option<i64>,
    pub is_up: i32,
    pub error_msg: Option<String>,
    pub checked_at: String,
}

// ── Query params ─────────────────────────────────────────────────────────────

/// Optional filters for log listing.
#[derive(Debug, Deserialize)]
pub struct LogQuery {
    /// Maximum number of results (default: 100, cap: 1000).
    pub limit: Option<i64>,
    /// Only return entries after this ISO-8601 timestamp.
    pub since: Option<String>,
}

// ── Handlers ─────────────────────────────────────────────────────────────────

/// `GET /api/v1/monitors/:id/logs` — fetch check logs for a monitor
/// (admin, response-time graph data).
pub async fn by_monitor(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(params): Query<LogQuery>,
) -> Result<Json<Vec<LogEntry>>, AppError> {
    let limit = params.limit.unwrap_or(100).min(1000);
    let since = params
        .since
        .unwrap_or_else(|| "1970-01-01T00:00:00".to_string());

    let rows = sqlx::query_as::<_, LogEntry>(
        "SELECT id, monitor_id, status_code, latency_ms, is_up, error_msg, checked_at \
         FROM   check_logs \
         WHERE  monitor_id = ? AND checked_at > ? \
         ORDER  BY checked_at DESC \
         LIMIT  ?",
    )
    .bind(&id)
    .bind(&since)
    .bind(limit)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(rows))
}

/// `GET /api/v1/public/monitors/:id/history` — last 24 h of latency data
/// points for a **public** active monitor (response-time graph).
pub async fn public_history(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Vec<LogEntry>>, AppError> {
    // Verify the monitor exists, is public, and is active (uniform 404 to prevent oracle).
    let monitor: Option<(i32, i32)> =
        sqlx::query_as("SELECT is_public, is_active FROM monitors WHERE id = ?")
            .bind(&id)
            .fetch_optional(&state.db)
            .await?;

    match monitor {
        Some((1, 1)) => {}
        _ => return Err(AppError::NotFound("Monitor not found".to_string())),
    }

    let mut rows = sqlx::query_as::<_, LogEntry>(
        "SELECT id, monitor_id, status_code, latency_ms, is_up, error_msg, checked_at \
         FROM   check_logs \
         WHERE  monitor_id = ? AND checked_at >= datetime('now', '-24 hours') \
         ORDER  BY checked_at DESC \
         LIMIT 1000",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await?;

    for row in &mut rows {
        if let Some(ref err) = row.error_msg {
            row.error_msg = Some(crate::api::admin::sanitize_public_error(err));
        }
    }

    Ok(Json(rows))
}
