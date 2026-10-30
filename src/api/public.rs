use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
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
             WHERE monitor_id = ? ORDER BY checked_at DESC, id DESC LIMIT 1",
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

// ── SVG Status Badge ─────────────────────────────────────────────────────────

/// Optional query parameters for dynamic badge styling or overrides.
#[derive(Debug, Deserialize, Default)]
pub struct BadgeQuery {
    /// Explicit status override (e.g. "operational", "degraded", "downtime", "not_found").
    pub status: Option<String>,
    /// Whether to include latency in operational badge text (e.g. "operational | 18ms").
    pub latency: Option<bool>,
}

/// Internal monitor status enum for badge rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorBadgeStatus {
    Up,
    Down,
    Degraded,
    NotFound,
}

impl MonitorBadgeStatus {
    pub fn default_label(&self) -> &'static str {
        match self {
            Self::Up => "operational",
            Self::Down => "downtime",
            Self::Degraded => "degraded",
            Self::NotFound => "not found",
        }
    }

    pub fn bg_color(&self) -> &'static str {
        match self {
            Self::Up => "#30D158",
            Self::Down => "#FF453A",
            Self::Degraded => "#FFD60A",
            Self::NotFound => "#71717A",
        }
    }

    pub fn text_color(&self) -> &'static str {
        match self {
            Self::Up | Self::Degraded => "#09090b",
            Self::Down | Self::NotFound => "#ffffff",
        }
    }
}

fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

fn estimate_text_width(text: &str) -> f32 {
    let mut width = 0.0;
    for ch in text.chars() {
        width += match ch {
            'm' | 'M' | 'W' | 'w' | '@' => 9.2,
            'A'..='Z' => 7.5,
            'i' | 'l' | '!' | '|' | '.' | ':' | ';' | '\'' | ' ' => 3.6,
            'f' | 'r' | 't' | 'j' => 4.8,
            'a'..='z' | '0'..='9' | '-' | '_' => 6.2,
            _ => 6.5,
        };
    }
    width
}

/// Renders clean vector badge markup (2-part rounded rectangle).
///
/// Left side: Dark background label ("KeepUp | <Monitor Name>" or "KeepUp").
/// Right side: Status indicator color and label.
pub fn render_badge_svg(
    left_label: &str,
    right_label: &str,
    status_bg: &str,
    status_text_color: &str,
    title_text: &str,
) -> String {
    let left_escaped = escape_xml(left_label);
    let right_escaped = escape_xml(right_label);
    let title_escaped = escape_xml(title_text);

    let left_text_w = estimate_text_width(left_label);
    let right_text_w = estimate_text_width(right_label);

    let padding = 16.0;
    let left_w = (left_text_w + padding).max(40.0).round();
    let right_w = (right_text_w + padding).max(40.0).round();
    let total_w = left_w + right_w;

    let left_center_x = (left_w / 2.0).round();
    let right_center_x = (left_w + right_w / 2.0).round();
    let clip_id = format!("k-clip-{}", total_w as u32);

    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{total_w}" height="20" role="img" aria-label="{left_escaped}: {right_escaped}">
  <title>{title_escaped}</title>
  <clipPath id="{clip_id}">
    <rect width="{total_w}" height="20" rx="4" fill="#fff"/>
  </clipPath>
  <g clip-path="url(#{clip_id})">
    <rect width="{left_w}" height="20" fill="#18181b"/>
    <rect x="{left_w}" width="{right_w}" height="20" fill="{status_bg}"/>
  </g>
  <g fill="#fff" text-anchor="middle" font-family="system-ui,BlinkMacSystemFont,'SF Pro Text','Segoe UI',Helvetica,Arial,sans-serif" text-rendering="geometricPrecision" font-size="11">
    <text x="{left_center_x}" y="14" fill="#a1a1aa">{left_escaped}</text>
    <text x="{right_center_x}" y="14" fill="{status_text_color}" font-weight="600">{right_escaped}</text>
  </g>
</svg>"##
    )
}

#[derive(sqlx::FromRow)]
struct MonitorBadgeRow {
    id: String,
    name: String,
    is_active: i32,
    is_public: i32,
    last_status: String,
}

/// `GET /api/public/badge/:id` or `GET /api/v1/public/badge/:id.svg`
///
/// Returns a 2-part vector SVG status badge for embedding in READMEs and documentation.
/// Response Headers: `Content-Type: image/svg+xml; charset=utf-8`, `Cache-Control: no-cache, max-age=30`.
/// Always returns HTTP 200 OK with vector content (including NOT_FOUND fallback).
pub async fn badge(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(params): Query<BadgeQuery>,
) -> Result<impl IntoResponse, AppError> {
    let monitor_id = id.strip_suffix(".svg").unwrap_or(&id);

    // Look up monitor by primary ID or matching name
    let monitor = sqlx::query_as::<_, MonitorBadgeRow>(
        "SELECT id, name, is_active, is_public, last_status \
         FROM monitors WHERE id = ? OR name = ? LIMIT 1",
    )
    .bind(monitor_id)
    .bind(monitor_id)
    .fetch_optional(&state.db)
    .await?;

    let (left_label, right_label, status_bg, status_text_color, title_text) = match monitor {
        Some(m) if m.is_public == 1 && m.is_active == 1 => {
            if let Some(ref override_status) = params.status {
                let status_lower = override_status.to_lowercase();
                let status_enum = match status_lower.as_str() {
                    "down" | "downtime" => MonitorBadgeStatus::Down,
                    "degraded" => MonitorBadgeStatus::Degraded,
                    "not_found" | "not found" => MonitorBadgeStatus::NotFound,
                    _ => MonitorBadgeStatus::Up,
                };
                let left = format!("KeepUp | {}", m.name);
                let right = status_enum.default_label().to_string();
                let title = format!("{left}: {right}");
                (left, right, status_enum.bg_color(), status_enum.text_color(), title)
            } else {
                // Fetch latest check
                let latest = sqlx::query_as::<_, LatestCheck>(
                    "SELECT is_up, latency_ms FROM check_logs \
                     WHERE monitor_id = ? ORDER BY checked_at DESC, id DESC LIMIT 1",
                )
                .bind(&m.id)
                .fetch_optional(&state.db)
                .await?;

                // Fetch 24-hour uptime percentage
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
                    100.0
                };

                let latest_latency = latest.as_ref().and_then(|c| c.latency_ms);

                // Determine operational state
                let (status_enum, right) = match &latest {
                    Some(c) if c.is_up == 0 => {
                        (MonitorBadgeStatus::Down, "downtime".to_string())
                    }
                    Some(_) if uptime_24h < 99.0 => {
                        (MonitorBadgeStatus::Degraded, "degraded".to_string())
                    }
                    Some(c) => {
                        if params.latency.unwrap_or(false) {
                            if let Some(ms) = c.latency_ms {
                                (MonitorBadgeStatus::Up, format!("operational | {}ms", ms))
                            } else {
                                (MonitorBadgeStatus::Up, "operational".to_string())
                            }
                        } else {
                            (MonitorBadgeStatus::Up, "operational".to_string())
                        }
                    }
                    None => {
                        if m.last_status == "DOWN" {
                            (MonitorBadgeStatus::Down, "downtime".to_string())
                        } else {
                            (MonitorBadgeStatus::Up, "operational".to_string())
                        }
                    }
                };

                let left = format!("KeepUp | {}", m.name);
                let title = match (&status_enum, latest_latency) {
                    (MonitorBadgeStatus::Up, Some(ms)) => {
                        format!("{left}: operational ({ms}ms, {uptime_24h:.1}% 24h)")
                    }
                    (MonitorBadgeStatus::Up, None) => {
                        format!("{left}: operational ({uptime_24h:.1}% 24h)")
                    }
                    (MonitorBadgeStatus::Degraded, Some(ms)) => {
                        format!("{left}: degraded ({ms}ms, {uptime_24h:.1}% 24h)")
                    }
                    (MonitorBadgeStatus::Degraded, None) => {
                        format!("{left}: degraded ({uptime_24h:.1}% 24h)")
                    }
                    (MonitorBadgeStatus::Down, _) => {
                        format!("{left}: downtime")
                    }
                    (MonitorBadgeStatus::NotFound, _) => {
                        "KeepUp: not found".to_string()
                    }
                };

                (left, right, status_enum.bg_color(), status_enum.text_color(), title)
            }
        }
        _ => {
            let status_enum = MonitorBadgeStatus::NotFound;
            let left = "KeepUp".to_string();
            let right = status_enum.default_label().to_string();
            let title = "KeepUp: monitor not found".to_string();
            (left, right, status_enum.bg_color(), status_enum.text_color(), title)
        }
    };

    let svg_body = render_badge_svg(
        &left_label,
        &right_label,
        status_bg,
        status_text_color,
        &title_text,
    );

    let headers = [
        (header::CONTENT_TYPE, "image/svg+xml; charset=utf-8"),
        (header::CACHE_CONTROL, "no-cache, max-age=30"),
    ];

    Ok((StatusCode::OK, headers, svg_body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_badge_svg_colors_and_text() {
        // UP / Operational: #30D158
        let svg_up = render_badge_svg("KeepUp | API Gateway", "operational", "#30D158", "#09090b", "API Gateway: operational");
        assert!(svg_up.contains("#30D158"));
        assert!(svg_up.contains("KeepUp | API Gateway"));
        assert!(svg_up.contains("operational"));
        assert!(svg_up.contains("<svg"));
        assert!(svg_up.contains("</svg>"));

        // DOWN / Downtime: #FF453A
        let svg_down = render_badge_svg("KeepUp | Database", "downtime", "#FF453A", "#ffffff", "Database: downtime");
        assert!(svg_down.contains("#FF453A"));
        assert!(svg_down.contains("downtime"));

        // DEGRADED: #FFD60A
        let svg_deg = render_badge_svg("KeepUp | Payment", "degraded", "#FFD60A", "#09090b", "Payment: degraded");
        assert!(svg_deg.contains("#FFD60A"));
        assert!(svg_deg.contains("degraded"));

        // NOT_FOUND: #71717A
        let svg_nf = render_badge_svg("KeepUp", "not found", "#71717A", "#ffffff", "KeepUp: monitor not found");
        assert!(svg_nf.contains("#71717A"));
        assert!(svg_nf.contains("not found"));
    }

    #[test]
    fn test_render_badge_svg_xml_escaping() {
        let svg = render_badge_svg("Auth & <Billing> \"v1\"", "status", "#30D158", "#09090b", "Title & 'Quotes'");
        assert!(svg.contains("Auth &amp; &lt;Billing&gt; &quot;v1&quot;"));
        assert!(svg.contains("Title &amp; &apos;Quotes&apos;"));
    }

    #[test]
    fn test_monitor_badge_status_properties() {
        assert_eq!(MonitorBadgeStatus::Up.bg_color(), "#30D158");
        assert_eq!(MonitorBadgeStatus::Down.bg_color(), "#FF453A");
        assert_eq!(MonitorBadgeStatus::Degraded.bg_color(), "#FFD60A");
        assert_eq!(MonitorBadgeStatus::NotFound.bg_color(), "#71717A");

        assert_eq!(MonitorBadgeStatus::Up.default_label(), "operational");
        assert_eq!(MonitorBadgeStatus::Down.default_label(), "downtime");
        assert_eq!(MonitorBadgeStatus::Degraded.default_label(), "degraded");
        assert_eq!(MonitorBadgeStatus::NotFound.default_label(), "not found");
    }

    #[tokio::test]
    async fn test_badge_handler_with_db() {
        use axum::response::IntoResponse;

        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS monitors (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                url TEXT NOT NULL,
                check_interval_secs INTEGER NOT NULL DEFAULT 60,
                timeout_secs INTEGER NOT NULL DEFAULT 5,
                max_retries INTEGER NOT NULL DEFAULT 3,
                is_active INTEGER NOT NULL DEFAULT 1,
                is_public INTEGER NOT NULL DEFAULT 1,
                last_status TEXT NOT NULL DEFAULT 'UP',
                down_since DATETIME,
                created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE IF NOT EXISTS check_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                monitor_id TEXT NOT NULL,
                status_code INTEGER,
                latency_ms INTEGER,
                is_up INTEGER NOT NULL,
                error_msg TEXT,
                checked_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
            );"
        )
        .execute(&pool)
        .await
        .unwrap();

        let (reload_tx, _) = tokio::sync::broadcast::channel::<String>(10);
        let state = Arc::new(AppState {
            db: pool.clone(),
            config: crate::config::Config::from_env(),
            http_client: reqwest::Client::new(),
            reload_tx,
        });

        // 1. Insert active public monitor
        sqlx::query(
            "INSERT INTO monitors (id, name, url, is_active, is_public) VALUES ('api-srv', 'API Gateway', 'https://api.test', 1, 1)"
        )
        .execute(&pool)
        .await
        .unwrap();

        // Check log: UP, 18ms
        sqlx::query(
            "INSERT INTO check_logs (monitor_id, is_up, latency_ms) VALUES ('api-srv', 1, 18)"
        )
        .execute(&pool)
        .await
        .unwrap();

        // Test GET /badge/api-srv.svg
        let res = badge(
            State(state.clone()),
            Path("api-srv.svg".to_string()),
            Query(BadgeQuery::default()),
        )
        .await
        .unwrap()
        .into_response();

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers().get("content-type").unwrap(),
            "image/svg+xml; charset=utf-8"
        );
        assert_eq!(
            res.headers().get("cache-control").unwrap(),
            "no-cache, max-age=30"
        );

        let body_bytes = axum::body::to_bytes(res.into_body(), 1024 * 64).await.unwrap();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
        assert!(body_str.contains("#30D158"));
        assert!(body_str.contains("KeepUp | API Gateway"));
        assert!(body_str.contains("operational"));

        // 2. Test nonexistent monitor
        let res_nf = badge(
            State(state.clone()),
            Path("unknown-service.svg".to_string()),
            Query(BadgeQuery::default()),
        )
        .await
        .unwrap()
        .into_response();

        assert_eq!(res_nf.status(), StatusCode::OK);
        let nf_bytes = axum::body::to_bytes(res_nf.into_body(), 1024 * 64).await.unwrap();
        let nf_str = String::from_utf8(nf_bytes.to_vec()).unwrap();
        assert!(nf_str.contains("#71717A"));
        assert!(nf_str.contains("not found"));

        // 3. Test DOWN check log
        sqlx::query(
            "INSERT INTO check_logs (monitor_id, is_up, latency_ms, checked_at) VALUES ('api-srv', 0, 0, datetime('now', '+1 second'))"
        )
        .execute(&pool)
        .await
        .unwrap();

        let res_down = badge(
            State(state.clone()),
            Path("api-srv.svg".to_string()),
            Query(BadgeQuery::default()),
        )
        .await
        .unwrap()
        .into_response();

        let down_bytes = axum::body::to_bytes(res_down.into_body(), 1024 * 64).await.unwrap();
        let down_str = String::from_utf8(down_bytes.to_vec()).unwrap();
        assert!(down_str.contains("#FF453A"));
        assert!(down_str.contains("downtime"));

        // 4. Test DEGRADED state (latest check UP, but 24h uptime is 66.7% < 99.0%)
        sqlx::query(
            "INSERT INTO check_logs (monitor_id, is_up, latency_ms, checked_at) VALUES ('api-srv', 1, 24, datetime('now', '+2 seconds'))"
        )
        .execute(&pool)
        .await
        .unwrap();

        let res_deg = badge(
            State(state.clone()),
            Path("api-srv.svg".to_string()),
            Query(BadgeQuery::default()),
        )
        .await
        .unwrap()
        .into_response();

        let deg_bytes = axum::body::to_bytes(res_deg.into_body(), 1024 * 64).await.unwrap();
        let deg_str = String::from_utf8(deg_bytes.to_vec()).unwrap();
        assert!(deg_str.contains("#FFD60A"));
        assert!(deg_str.contains("degraded"));

        // 5. Test latency query param (?latency=true) with 100% operational monitor
        sqlx::query(
            "INSERT INTO monitors (id, name, url, is_active, is_public) VALUES ('lat-srv', 'Metrics Service', 'https://metrics.test', 1, 1)"
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO check_logs (monitor_id, is_up, latency_ms, checked_at) VALUES ('lat-srv', 1, 24, datetime('now'))"
        )
        .execute(&pool)
        .await
        .unwrap();

        let res_lat = badge(
            State(state.clone()),
            Path("lat-srv.svg".to_string()),
            Query(BadgeQuery { status: None, latency: Some(true) }),
        )
        .await
        .unwrap()
        .into_response();

        let lat_bytes = axum::body::to_bytes(res_lat.into_body(), 1024 * 64).await.unwrap();
        let lat_str = String::from_utf8(lat_bytes.to_vec()).unwrap();
        assert!(lat_str.contains("operational | 24ms"));
        assert!(lat_str.contains("#30D158"));

        // 6. Test private monitor is returned as NOT_FOUND (gray #71717A)
        sqlx::query(
            "INSERT INTO monitors (id, name, url, is_active, is_public) VALUES ('priv-mon', 'Private Intranet', 'https://internal.test', 1, 0)"
        )
        .execute(&pool)
        .await
        .unwrap();

        let res_priv = badge(
            State(state.clone()),
            Path("priv-mon.svg".to_string()),
            Query(BadgeQuery::default()),
        )
        .await
        .unwrap()
        .into_response();

        assert_eq!(res_priv.status(), StatusCode::OK);
        let priv_bytes = axum::body::to_bytes(res_priv.into_body(), 1024 * 64).await.unwrap();
        let priv_str = String::from_utf8(priv_bytes.to_vec()).unwrap();
        assert!(priv_str.contains("#71717A"));
        assert!(priv_str.contains("not found"));

        // 7. Test status query override (e.g. ?status=degraded)
        let res_override = badge(
            State(state.clone()),
            Path("api-srv.svg".to_string()),
            Query(BadgeQuery { status: Some("degraded".to_string()), latency: None }),
        )
        .await
        .unwrap()
        .into_response();

        let override_bytes = axum::body::to_bytes(res_override.into_body(), 1024 * 64).await.unwrap();
        let override_str = String::from_utf8(override_bytes.to_vec()).unwrap();
        assert!(override_str.contains("#FFD60A"));
        assert!(override_str.contains("degraded"));

        // 8. Full Router integration test: GET /api/public/badge/api-srv.svg
        use tower::ServiceExt;
        let app = crate::api::router(state.clone()).with_state(state.clone());
        let req = axum::http::Request::builder()
            .method("GET")
            .uri("/api/public/badge/api-srv.svg")
            .body(axum::body::Body::empty())
            .unwrap();

        let router_res = app.oneshot(req).await.unwrap();
        assert_eq!(router_res.status(), StatusCode::OK);
        assert_eq!(
            router_res.headers().get("content-type").unwrap(),
            "image/svg+xml; charset=utf-8"
        );
        assert_eq!(
            router_res.headers().get("cache-control").unwrap(),
            "no-cache, max-age=30"
        );
        let router_bytes = axum::body::to_bytes(router_res.into_body(), 1024 * 64).await.unwrap();
        let router_str = String::from_utf8(router_bytes.to_vec()).unwrap();
        assert!(router_str.contains("KeepUp | API Gateway"));
        assert!(router_str.contains("<svg"));
        assert!(router_str.contains("</svg>"));
    }
}
