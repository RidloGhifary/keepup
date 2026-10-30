use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::db::monitors::{
    get_24h_latency_history, get_7day_uptime_history, DailyUptime, HourlyLatency,
};
use crate::error::AppError;
use crate::AppState;

// ── Response types ───────────────────────────────────────────────────────────

/// Top-level public status response.
#[derive(Debug, Serialize, Deserialize)]
pub struct PublicStatus {
    /// Overall operational banner text.
    pub status: String,
    /// Optional status page theme configuration ("default", "oled", "midnight", "light").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_page_theme: Option<String>,
    /// Per-monitor summaries (only public monitors).
    pub monitors: Vec<PublicMonitor>,
}

/// Summary of a single public monitor.
#[derive(Debug, Serialize, Deserialize)]
pub struct PublicMonitor {
    pub id: String,
    pub name: String,
    pub url: String,
    /// Current status derived from the most recent check.
    pub current_status: String,
    /// Most recent response latency (ms), if available.
    pub last_latency_ms: Option<i64>,
    /// 24-hour uptime percentage (0.0 – 100.0).
    pub uptime_24h: f64,
    /// 7-day daily uptime history buckets (Day -6 to Today).
    pub uptime_history: Vec<DailyUptime>,
    /// 24-hour hourly latency breakdown buckets (23 hours ago to current hour).
    pub latency_24h: Vec<HourlyLatency>,
}

// ── Row helpers for sqlx ─────────────────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct MonitorIdName {
    id: String,
    name: String,
    url: String,
}

#[derive(sqlx::FromRow)]
struct LatestCheck {
    is_up: i32,
    latency_ms: Option<i64>,
}

#[derive(sqlx::FromRow)]
struct UptimeAgg {
    total: i64,
    up_count: i64,
}

// ── Handler ──────────────────────────────────────────────────────────────────

/// `GET /api/v1/public/status` — aggregated health for all public monitors.
///
/// Returns an overall operational banner plus per-monitor current status,
/// 24-hour uptime percentage, and 7-day uptime timeline history.
pub async fn public_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<PublicStatus>, AppError> {
    let monitors = sqlx::query_as::<_, MonitorIdName>(
        "SELECT id, name, url FROM monitors WHERE is_public = 1 AND is_active = 1 ORDER BY name",
    )
    .fetch_all(&state.db)
    .await?;

    let mut public_monitors: Vec<PublicMonitor> = Vec::with_capacity(monitors.len());
    let mut any_down = false;
    let mut any_degraded = false;

    for m in &monitors {
        // Latest check result.
        let latest = sqlx::query_as::<_, LatestCheck>(
            "SELECT is_up, latency_ms FROM check_logs \
             WHERE monitor_id = ? ORDER BY checked_at DESC LIMIT 1",
        )
        .bind(&m.id)
        .fetch_optional(&state.db)
        .await?;

        let (current_status, last_latency_ms) = match &latest {
            Some(c) if c.is_up == 1 => ("UP".to_string(), c.latency_ms),
            Some(c) => {
                any_down = true;
                ("DOWN".to_string(), c.latency_ms)
            }
            None => ("UNKNOWN".to_string(), None),
        };

        // 24-hour uptime percentage.
        let agg = sqlx::query_as::<_, UptimeAgg>(
            "SELECT COUNT(*) AS total, \
                    COALESCE(SUM(CASE WHEN is_up = 1 THEN 1 ELSE 0 END), 0) AS up_count \
             FROM check_logs \
             WHERE monitor_id = ? AND checked_at > datetime('now', '-24 hours')",
        )
        .bind(&m.id)
        .fetch_one(&state.db)
        .await?;

        let uptime_24h = if agg.total > 0 {
            (agg.up_count as f64 / agg.total as f64) * 100.0
        } else {
            100.0 // No data yet → assume operational.
        };

        if uptime_24h < 100.0 && uptime_24h >= 90.0 {
            any_degraded = true;
        }

        // Fetch 7-day daily uptime history.
        let uptime_history = get_7day_uptime_history(&state.db, &m.id).await?;

        // Fetch 24-hour hourly latency breakdown.
        let latency_24h = get_24h_latency_history(&state.db, &m.id).await?;

        public_monitors.push(PublicMonitor {
            id: m.id.clone(),
            name: m.name.clone(),
            url: m.url.clone(),
            current_status,
            last_latency_ms,
            uptime_24h: (uptime_24h * 100.0).round() / 100.0,
            uptime_history,
            latency_24h,
        });
    }

    // ── Overall banner ───────────────────────────────────────────────
    let status = if any_down {
        "Major Outage".to_string()
    } else if any_degraded {
        "Partial System Outage".to_string()
    } else {
        "All Systems Operational".to_string()
    };

    let status_page_theme = std::env::var("STATUS_PAGE_THEME")
        .or_else(|_| std::env::var("KEEPUP_THEME"))
        .ok();

    Ok(Json(PublicStatus {
        status,
        status_page_theme,
        monitors: public_monitors,
    }))
}
