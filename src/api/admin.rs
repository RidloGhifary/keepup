use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::Json;
use axum_extra::extract::cookie::CookieJar;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::info;

use crate::api::dto::UpdateMonitorDto;
use crate::db::checks::{self, DetailCheckLog, LatencyPercentiles, UptimeSlas};
use crate::db::monitors::{self, HourlyLatency, Monitor};
use crate::error::AppError;
use crate::AppState;

// ── Monitor Admin Handlers ───────────────────────────────────────────────────

/// `PUT /api/monitors/:id`
///
/// Updates an existing monitor in SQLite and immediately notifies the background
/// scheduler over `state.reload_tx` to re-arm the health check interval.
pub async fn update_monitor(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateMonitorDto>,
) -> Result<Json<Monitor>, AppError> {
    payload.validate()?;

    if let Some(ref channel_ids) = payload.channel_ids {
        let _ = crate::db::notifications::set_monitor_channels(&state.db, &id, channel_ids).await;
    }

    let updated = monitors::update_monitor(&state.db, &id, payload).await?;

    // Signal the background scheduler to re-arm the monitor interval immediately.
    if let Err(err) = state.reload_tx.send(id.clone()) {
        tracing::debug!(
            monitor_id = %id,
            "Reload signal not received by active subscribers: {err}"
        );
    } else {
        info!(
            monitor_id = %id,
            "Dispatched reload signal for updated monitor"
        );
    }

    Ok(Json(updated))
}

// ── Monitor Telemetry & Detail Handlers ──────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorDetailInfo {
    pub id: String,
    pub name: String,
    pub url: String,
    pub method: String,
    pub status: String,
    pub is_active: bool,
    pub is_public: bool,
    pub check_interval_secs: i32,
    pub timeout_secs: i32,
    pub check_domain_expiry: bool,
    pub domain_warning_days: i32,
    pub domain_expires_at: Option<String>,
    pub domain_days_remaining: Option<i32>,
    pub check_ssl: bool,
    pub ssl_warning_days: i32,
    pub ssl_expires_at: Option<String>,
    pub ssl_days_remaining: Option<i32>,
    #[serde(default)]
    pub channel_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetExpiryItem {
    pub monitor_id: String,
    pub name: String,
    pub url: String,
    pub ssl_enabled: bool,
    pub ssl_days_remaining: Option<i32>,
    pub ssl_expires_at: Option<String>,
    pub domain_enabled: bool,
    pub domain_days_remaining: Option<i32>,
    pub domain_expires_at: Option<String>,
    pub urgency: String, // "CRITICAL" | "WARNING" | "HEALTHY"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorDetailResponse {
    pub monitor: MonitorDetailInfo,
    pub slas: UptimeSlas,
    pub latency: LatencyPercentiles,
    pub hourly_latency: Vec<HourlyLatency>,
    pub logs: Vec<DetailCheckLog>,
}

/// `GET /api/admin/monitors/:id/details`
///
/// Fetches deep-dive diagnostic telemetry, SLA percentages, latency stats, and 50 recent check logs.
/// For public monitors, unauthenticated access is permitted. For private monitors, authentication is required.
pub async fn get_monitor_details(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<MonitorDetailResponse>, AppError> {
    let monitor = monitors::get_monitor_by_id(&state.db, &id).await?;

    // If monitor is private (is_public == 0), require valid admin authentication
    if monitor.is_public == 0 {
        let has_cookie_auth = jar
            .get(crate::api::auth::SESSION_COOKIE_NAME)
            .and_then(|c| crate::api::auth::validate_jwt(c.value(), &state.config.jwt_secret).ok())
            .is_some();

        let has_bearer_auth = headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
            .and_then(|t| crate::api::auth::validate_jwt(t, &state.config.jwt_secret).ok())
            .is_some();

        if !has_cookie_auth && !has_bearer_auth {
            return Err(AppError::Unauthorized(
                "Authentication required to view private monitor details".to_string(),
            ));
        }
    }

    let slas = checks::get_uptime_slas(&state.db, &id).await?;
    let latency = checks::get_latency_percentiles(&state.db, &id, 24).await?;
    let hourly_latency = monitors::get_24h_latency_history(&state.db, &id).await?;
    let logs = checks::get_recent_check_logs(&state.db, &id, 50).await?;

    let channel_ids = crate::db::notifications::get_channel_ids_for_monitor(&state.db, &id)
        .await
        .unwrap_or_default();

    let info = MonitorDetailInfo {
        id: monitor.id.clone(),
        name: monitor.name.clone(),
        url: monitor.url.clone(),
        method: "GET".to_string(),
        status: monitor.last_status.clone(),
        is_active: monitor.is_active == 1,
        is_public: monitor.is_public == 1,
        check_interval_secs: monitor.check_interval_secs,
        timeout_secs: monitor.timeout_secs,
        check_domain_expiry: monitor.check_domain_expiry == 1,
        domain_warning_days: monitor.domain_warning_days,
        domain_expires_at: monitor.domain_expires_at.clone(),
        domain_days_remaining: monitor.domain_days_remaining,
        check_ssl: monitor.check_ssl == 1,
        ssl_warning_days: monitor.ssl_warning_days,
        ssl_expires_at: monitor.ssl_expires_at.clone(),
        ssl_days_remaining: monitor.ssl_days_remaining,
        channel_ids,
    };

    Ok(Json(MonitorDetailResponse {
        monitor: info,
        slas,
        latency,
        hourly_latency,
        logs,
    }))
}

#[derive(Debug, Serialize)]
pub struct PingResponse {
    pub status: String,
    pub status_code: Option<i32>,
    pub latency_ms: i64,
}

/// `POST /api/admin/monitors/:id/ping`
///
/// Executes an immediate synthetic check out-of-band and persists the check log to SQLite.
pub async fn ping_monitor(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<PingResponse>, AppError> {
    let monitor = monitors::get_monitor_by_id(&state.db, &id).await?;

    let result = crate::checker::http::ping_http(
        &state.http_client,
        &monitor.url,
        monitor.timeout_secs as u64,
    )
    .await;

    let mut domain_days = monitor.domain_days_remaining;
    if monitor.check_domain_expiry == 1 {
        if let Ok(info) = crate::checker::domain::check_domain_expiration(&state.http_client, &monitor.url).await {
            domain_days = Some(info.days_remaining as i32);
            let _ = monitors::update_domain_expiry(
                &state.db,
                &monitor.id,
                Some(&info.expires_at),
                Some(info.days_remaining as i32),
            )
            .await;
        }
    }

    let mut ssl_days = monitor.ssl_days_remaining;
    if monitor.check_ssl == 1 {
        if let Ok(info) = crate::checker::tls::check_ssl_expiration(&monitor.url, monitor.timeout_secs as u64).await {
            ssl_days = Some(info.days_remaining as i32);
            let _ = monitors::update_ssl_expiry(
                &state.db,
                &monitor.id,
                Some(&info.expires_at),
                Some(info.days_remaining as i32),
            )
            .await;
        }
    }

    // Persist check log to SQLite
    sqlx::query(
        r#"
        INSERT INTO check_logs (monitor_id, status_code, latency_ms, response_time_ms, is_up, error_msg, checked_at, created_at, domain_days_remaining, ssl_days_remaining)
        VALUES (?, ?, ?, ?, ?, ?, datetime('now'), datetime('now'), ?, ?)
        "#,
    )
    .bind(&monitor.id)
    .bind(result.status_code)
    .bind(result.latency_ms)
    .bind(result.latency_ms)
    .bind(result.is_up as i32)
    .bind(&result.error_msg)
    .bind(domain_days)
    .bind(ssl_days)
    .execute(&state.db)
    .await?;

    let status_str = if result.is_up { "UP" } else { "DOWN" };
    let down_since = if result.is_up {
        None
    } else {
        Some(chrono::Utc::now().to_rfc3339())
    };

    let _ = monitors::update_monitor_status(
        &state.db,
        &monitor.id,
        status_str,
        down_since.as_deref(),
    )
    .await;

    info!(
        monitor_id = %monitor.id,
        status = %status_str,
        latency_ms = result.latency_ms,
        "Executed manual on-demand ping"
    );

    Ok(Json(PingResponse {
        status: status_str.to_string(),
        status_code: result.status_code,
        latency_ms: result.latency_ms,
    }))
}

// ── Asset Expiry Handlers ────────────────────────────────────────────────────

fn compute_urgency(
    ssl_days: Option<i32>,
    domain_days: Option<i32>,
    ssl_enabled: bool,
    domain_enabled: bool,
) -> String {
    let mut min_days: Option<i32> = None;
    if ssl_enabled {
        if let Some(d) = ssl_days {
            min_days = Some(min_days.map_or(d, |m| m.min(d)));
        }
    }
    if domain_enabled {
        if let Some(d) = domain_days {
            min_days = Some(min_days.map_or(d, |m| m.min(d)));
        }
    }

    match min_days {
        Some(days) if days <= 7 => "CRITICAL".to_string(),
        Some(days) if days <= 30 => "WARNING".to_string(),
        _ => "HEALTHY".to_string(),
    }
}

fn urgency_sort_key(item: &AssetExpiryItem) -> i32 {
    let mut min_val = i32::MAX;
    if item.ssl_enabled {
        if let Some(d) = item.ssl_days_remaining {
            min_val = min_val.min(d);
        }
    }
    if item.domain_enabled {
        if let Some(d) = item.domain_days_remaining {
            min_val = min_val.min(d);
        }
    }
    min_val
}

/// `GET /api/admin/assets/expiry`
///
/// Returns list of all active monitors with TLS or domain expiration tracking,
/// sorted by MIN(ssl_days_remaining, domain_days_remaining) ASC (expiring soonest first).
pub async fn get_asset_expiry(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<AssetExpiryItem>>, AppError> {
    let monitors = monitors::get_asset_expiry_monitors(&state.db).await?;

    let mut items = Vec::with_capacity(monitors.len());
    for m in monitors {
        let ssl_enabled = m.check_ssl == 1;
        let domain_enabled = m.check_domain_expiry == 1;
        let urgency = compute_urgency(
            m.ssl_days_remaining,
            m.domain_days_remaining,
            ssl_enabled,
            domain_enabled,
        );

        items.push(AssetExpiryItem {
            monitor_id: m.id,
            name: m.name,
            url: m.url,
            ssl_enabled,
            ssl_days_remaining: m.ssl_days_remaining,
            ssl_expires_at: m.ssl_expires_at,
            domain_enabled,
            domain_days_remaining: m.domain_days_remaining,
            domain_expires_at: m.domain_expires_at,
            urgency,
        });
    }

    items.sort_by(|a, b| {
        urgency_sort_key(a)
            .cmp(&urgency_sort_key(b))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(Json(items))
}

/// `POST /api/admin/monitors/:id/refresh-assets`
///
/// Performs on-demand TLS and RDAP checks for a monitor, updates DB, and returns updated AssetExpiryItem.
pub async fn refresh_monitor_assets(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<AssetExpiryItem>, AppError> {
    let mut monitor = monitors::get_monitor_by_id(&state.db, &id).await?;

    if monitor.check_ssl == 1 {
        if let Ok(info) = crate::checker::tls::check_ssl_expiration(&monitor.url, monitor.timeout_secs as u64).await {
            let _ = monitors::update_ssl_expiry(
                &state.db,
                &monitor.id,
                Some(&info.expires_at),
                Some(info.days_remaining as i32),
            )
            .await;
            monitor.ssl_expires_at = Some(info.expires_at);
            monitor.ssl_days_remaining = Some(info.days_remaining as i32);
        }
    }

    if monitor.check_domain_expiry == 1 {
        if let Ok(info) = crate::checker::domain::check_domain_expiration(&state.http_client, &monitor.url).await {
            let _ = monitors::update_domain_expiry(
                &state.db,
                &monitor.id,
                Some(&info.expires_at),
                Some(info.days_remaining as i32),
            )
            .await;
            monitor.domain_expires_at = Some(info.expires_at);
            monitor.domain_days_remaining = Some(info.days_remaining as i32);
        }
    }

    let ssl_enabled = monitor.check_ssl == 1;
    let domain_enabled = monitor.check_domain_expiry == 1;
    let urgency = compute_urgency(
        monitor.ssl_days_remaining,
        monitor.domain_days_remaining,
        ssl_enabled,
        domain_enabled,
    );

    let item = AssetExpiryItem {
        monitor_id: monitor.id,
        name: monitor.name,
        url: monitor.url,
        ssl_enabled,
        ssl_days_remaining: monitor.ssl_days_remaining,
        ssl_expires_at: monitor.ssl_expires_at,
        domain_enabled,
        domain_days_remaining: monitor.domain_days_remaining,
        domain_expires_at: monitor.domain_expires_at,
        urgency,
    };

    Ok(Json(item))
}

// ── Password Admin Handlers ───────────────────────────────────────────────────

/// Request payload for changing the admin password.
#[derive(Debug, Deserialize)]
pub struct ChangePasswordPayload {
    pub current_password: String,
    pub new_password: String,
}

/// `POST /api/admin/change-password`
///
/// Changes the administrator password after validating the current password.
/// Persists the new salted SHA-256 hash into SQLite `app_config`.
pub async fn change_password(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ChangePasswordPayload>,
) -> Result<Json<Value>, AppError> {
    // 1. Verify current_password matches active admin credentials.
    let active_password =
        crate::api::auth::get_active_admin_password(&state.db, &state.config.admin_password).await;

    if !crate::api::auth::verify_password(&payload.current_password, &active_password) {
        return Err(AppError::Unauthorized(
            "Invalid current password".to_string(),
        ));
    }

    // 2. Enforce minimum length on new_password (>= 8 characters).
    if payload.new_password.chars().count() < 8 {
        return Err(AppError::BadRequest(
            "New password must be at least 8 characters long".to_string(),
        ));
    }

    // 3. Persist new password hash into SQLite app_config table.
    let hashed = crate::api::auth::hash_password(&payload.new_password);
    sqlx::query(
        "INSERT INTO app_config (key, value) VALUES ('admin_password', $1) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(&hashed)
    .execute(&state.db)
    .await
    .map_err(AppError::Database)?;

    info!("Administrator password updated successfully");
    Ok(Json(json!({
        "message": "Password updated successfully"
    })))
}

// ── Notification Channel Admin Handlers ─────────────────────────────────────

fn default_true_bool() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CreateChannelRequest {
    pub name: String,
    #[serde(rename = "type", alias = "provider")]
    pub channel_type: String,
    pub url: String,
    #[serde(default)]
    pub is_default: bool,
    #[serde(default = "default_true_bool")]
    pub is_enabled: bool,
    #[serde(default = "default_true_bool")]
    pub use_default_template: bool,
    #[serde(default = "default_true_bool")]
    pub notify_on_resolve: bool,
    #[serde(default, alias = "message_template")]
    pub custom_template: Option<String>,
}

impl CreateChannelRequest {
    pub fn validate(&self) -> Result<(), AppError> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err(AppError::BadRequest("Channel name cannot be empty".to_string()));
        }
        let url = self.url.trim();
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            return Err(AppError::BadRequest("Channel URL must begin with http:// or https://".to_string()));
        }
        let ctype = self.channel_type.trim().to_lowercase();
        if !["discord", "slack", "webhook", "telegram", "generic", "generic_webhook"].contains(&ctype.as_str()) {
            return Err(AppError::BadRequest(
                "Invalid channel type. Supported types: discord, slack, webhook, telegram".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UpdateChannelRequest {
    pub name: Option<String>,
    #[serde(rename = "type", alias = "provider")]
    pub channel_type: Option<String>,
    pub url: Option<String>,
    pub is_default: Option<bool>,
    pub is_enabled: Option<bool>,
    pub use_default_template: Option<bool>,
    pub notify_on_resolve: Option<bool>,
    #[serde(default, alias = "message_template")]
    pub custom_template: Option<String>,
}

impl UpdateChannelRequest {
    pub fn validate(&self) -> Result<(), AppError> {
        if let Some(ref name) = self.name {
            if name.trim().is_empty() {
                return Err(AppError::BadRequest("Channel name cannot be empty".to_string()));
            }
        }
        if let Some(ref url) = self.url {
            let u = url.trim();
            if !u.is_empty()
                && !crate::db::notifications::is_masked_url(u)
                && !(u.starts_with("http://") || u.starts_with("https://"))
            {
                return Err(AppError::BadRequest("Channel URL must begin with http:// or https://".to_string()));
            }
        }
        if let Some(ref ctype) = self.channel_type {
            let c = ctype.trim().to_lowercase();
            if !["discord", "slack", "webhook", "telegram", "generic", "generic_webhook"].contains(&c.as_str()) {
                return Err(AppError::BadRequest(
                    "Invalid channel type. Supported types: discord, slack, webhook, telegram".to_string(),
                ));
            }
        }
        Ok(())
    }
}

fn normalize_channel_type(raw: &str) -> String {
    match raw.trim().to_lowercase().as_str() {
        "generic" | "generic_webhook" => "webhook".to_string(),
        other => other.to_string(),
    }
}

/// `GET /api/admin/channels` — List all saved notification channels (with masked URLs).
pub async fn list_channels(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<crate::db::notifications::NotificationChannel>>, AppError> {
    let channels = crate::db::notifications::list_channels(&state.db).await?;
    Ok(Json(channels.into_iter().map(|c| c.masked_view()).collect()))
}

/// `POST /api/admin/channels` — Create a reusable notification channel.
pub async fn create_channel(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateChannelRequest>,
) -> Result<(axum::http::StatusCode, Json<crate::db::notifications::NotificationChannel>), AppError> {
    payload.validate()?;

    let channel = crate::db::notifications::NotificationChannel {
        id: uuid::Uuid::new_v4().to_string(),
        name: payload.name.trim().to_string(),
        channel_type: normalize_channel_type(&payload.channel_type),
        url: payload.url.trim().to_string(),
        is_default: payload.is_default,
        is_enabled: payload.is_enabled,
        use_default_template: payload.use_default_template,
        notify_on_resolve: payload.notify_on_resolve,
        custom_template: payload.custom_template.filter(|s| !s.trim().is_empty()),
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    let created = crate::db::notifications::create_channel(&state.db, channel).await?;
    Ok((axum::http::StatusCode::CREATED, Json(created.masked_view())))
}

/// `PUT /api/admin/channels/:id` — Update channel details (preserving existing secret URL when omitted or masked).
pub async fn update_channel(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateChannelRequest>,
) -> Result<Json<crate::db::notifications::NotificationChannel>, AppError> {
    payload.validate()?;

    let existing = crate::db::notifications::get_channel(&state.db, &id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Notification channel {id} not found")))?;

    let name = payload.name.as_deref().map(str::trim).unwrap_or(&existing.name);
    let channel_type = payload
        .channel_type
        .as_deref()
        .map(normalize_channel_type)
        .unwrap_or(existing.channel_type);
    let url = match payload.url.as_deref().map(str::trim) {
        Some(u) if !u.is_empty() && !crate::db::notifications::is_masked_url(u) => u,
        _ => &existing.url,
    };
    let is_default = payload.is_default.unwrap_or(existing.is_default);
    let is_enabled = payload.is_enabled.unwrap_or(existing.is_enabled);
    let use_default_template = payload
        .use_default_template
        .unwrap_or(existing.use_default_template);
    let notify_on_resolve = payload
        .notify_on_resolve
        .unwrap_or(existing.notify_on_resolve);
    let custom_template = match &payload.custom_template {
        Some(tpl) if tpl.trim().is_empty() => None,
        Some(tpl) => Some(tpl.as_str()),
        None => existing.custom_template.as_deref(),
    };

    let updated = crate::db::notifications::update_channel(
        &state.db,
        &id,
        name,
        &channel_type,
        url,
        is_default,
        is_enabled,
        use_default_template,
        notify_on_resolve,
        custom_template,
    )
    .await?;

    Ok(Json(updated.masked_view()))
}

/// `DELETE /api/admin/channels/:id` — Remove a channel and unbind from monitors.
pub async fn delete_channel(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<axum::http::StatusCode, AppError> {
    let deleted = crate::db::notifications::delete_channel(&state.db, &id).await?;
    if !deleted {
        return Err(AppError::NotFound(format!("Notification channel {id} not found")));
    }
    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// `POST /api/admin/channels/:id/test` — Dispatch an instant test payload to verify webhook functionality.
pub async fn test_channel(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (axum::http::StatusCode, Json<Value>)> {
    let channel = match crate::db::notifications::get_channel(&state.db, &id).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            return Err((
                axum::http::StatusCode::NOT_FOUND,
                Json(json!({ "error": format!("Notification channel {id} not found") })),
            ));
        }
        Err(err) => {
            return Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": err.to_string() })),
            ));
        }
    };

    match crate::alerting::test_channel(&state.http_client, &channel).await {
        Ok(()) => Ok(Json(json!({
            "success": true,
            "message": format!("Test payload successfully delivered to {} ({})", channel.name, channel.channel_type)
        }))),
        Err(err) => Err((
            axum::http::StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "error": format!("Channel test failed: {err}")
            })),
        )),
    }
}

/// `GET /api/admin/monitors/:id/channels` — Get channels bound to a monitor.
pub async fn get_monitor_channels(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Vec<String>>, AppError> {
    let channel_ids = crate::db::notifications::get_channel_ids_for_monitor(&state.db, &id).await?;
    Ok(Json(channel_ids))
}

// ── Status Page Admin Handlers (delegated to crate::api::status_pages) ───────

#[allow(unused_imports)]
pub use crate::api::status_pages::{
    create_status_page, delete_status_page, get_status_page, list_status_pages,
    update_status_page,
};

// ── Maintenance Window Admin Handlers (delegated to crate::api::maintenance) ──

#[allow(unused_imports)]
pub use crate::api::maintenance::{
    create_maintenance, delete_maintenance, get_maintenance, list_maintenance,
    update_maintenance,
};

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use sqlx::sqlite::SqlitePoolOptions;
    use tower::ServiceExt;

    async fn setup_test_app() -> (axum::Router, Arc<AppState>) {
        let _ = tokio_rustls::rustls::crypto::ring::default_provider().install_default();

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("Failed to run migrations");

        let config = crate::config::Config::from_env();
        let http_client = reqwest::Client::new();
        let (reload_tx, _) = tokio::sync::broadcast::channel(16);

        let state = Arc::new(AppState {
            db: pool,
            config,
            http_client,
            reload_tx,
        });

        let app = crate::api::router(state.clone()).with_state(state.clone());
        (app, state)
    }

    #[tokio::test]
    async fn test_change_password_flow() {
        let (app, state) = setup_test_app().await;

        let token = crate::api::auth::create_jwt(
            &state.config.admin_user,
            &state.config.jwt_secret,
        )
        .unwrap();

        // 1. Unauthenticated request should fail (401)
        let req = Request::builder()
            .method("POST")
            .uri("/api/admin/change-password")
            .header("Content-Type", "application/json")
            .body(Body::from(
                json!({
                    "current_password": "admin",
                    "new_password": "newpassword123"
                })
                .to_string(),
            ))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

        // 2. Authenticated request with invalid current password should return 401 with { "error": "Invalid current password" }
        let req = Request::builder()
            .method("POST")
            .uri("/api/admin/change-password")
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .body(Body::from(
                json!({
                    "current_password": "wrongpassword",
                    "new_password": "newpassword123"
                })
                .to_string(),
            ))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
        let body = axum::body::to_bytes(res.into_body(), 1024 * 16).await.unwrap();
        let json_body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json_body["error"], "Invalid current password");

        // 3. New password too short (< 8 chars) should return 400 Bad Request
        let req = Request::builder()
            .method("POST")
            .uri("/api/admin/change-password")
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .body(Body::from(
                json!({
                    "current_password": "admin",
                    "new_password": "short"
                })
                .to_string(),
            ))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);

        // 4. Valid change password should return 200 OK with { "message": "Password updated successfully" }
        let req = Request::builder()
            .method("POST")
            .uri("/api/admin/change-password")
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .body(Body::from(
                json!({
                    "current_password": "admin",
                    "new_password": "supersecretpassword123"
                })
                .to_string(),
            ))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = axum::body::to_bytes(res.into_body(), 1024 * 16).await.unwrap();
        let json_body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json_body["message"], "Password updated successfully");

        // 5. Verify old password no longer works for login
        let req = Request::builder()
            .method("POST")
            .uri("/api/auth/login")
            .header("X-Forwarded-For", "10.0.0.42")
            .header("Content-Type", "application/json")
            .body(Body::from(
                json!({
                    "username": "admin",
                    "password": "admin"
                })
                .to_string(),
            ))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

        // 6. Verify new password successfully logs in
        let req = Request::builder()
            .method("POST")
            .uri("/api/auth/login")
            .header("X-Forwarded-For", "10.0.0.42")
            .header("Content-Type", "application/json")
            .body(Body::from(
                json!({
                    "username": "admin",
                    "password": "supersecretpassword123"
                })
                .to_string(),
            ))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_monitor_details_and_ping_endpoints() {
        let (app, state) = setup_test_app().await;

        let token = crate::api::auth::create_jwt(
            &state.config.admin_user,
            &state.config.jwt_secret,
        )
        .unwrap();

        // Seed a public monitor and a private monitor
        sqlx::query(
            "INSERT INTO monitors (id, name, url, is_active, is_public) VALUES ('mon-pub', 'Public Test', 'https://example.com', 1, 1)"
        )
        .execute(&state.db)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO monitors (id, name, url, is_active, is_public) VALUES ('mon-priv', 'Private Test', 'https://example.com', 1, 0)"
        )
        .execute(&state.db)
        .await
        .unwrap();

        // Seed 3 check logs for mon-pub
        for i in 1..=3 {
            sqlx::query(
                "INSERT INTO check_logs (monitor_id, status_code, latency_ms, is_up, error_msg, checked_at, created_at)
                 VALUES ('mon-pub', 200, ?, 1, NULL, datetime('now'), datetime('now'))"
            )
            .bind(20 + i * 5)
            .execute(&state.db)
            .await
            .unwrap();
        }

        // 1. Unauthenticated request to private monitor details should return 401
        let req = Request::builder()
            .method("GET")
            .uri("/api/admin/monitors/mon-priv/details")
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

        // 2. Unauthenticated request to public monitor details should succeed (200 OK)
        let req = Request::builder()
            .method("GET")
            .uri("/api/admin/monitors/mon-pub/details")
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = axum::body::to_bytes(res.into_body(), 1024 * 64).await.unwrap();
        let details: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(details["monitor"]["id"], "mon-pub");
        assert_eq!(details["monitor"]["name"], "Public Test");
        assert_eq!(details["monitor"]["method"], "GET");
        assert_eq!(details["slas"]["uptime_24h"], 100.0);
        assert_eq!(details["logs"].as_array().unwrap().len(), 3);
        assert_eq!(details["hourly_latency"].as_array().unwrap().len(), 24);

        // 3. Authenticated request to private monitor details should succeed
        let req = Request::builder()
            .method("GET")
            .uri("/api/admin/monitors/mon-priv/details")
            .header("Authorization", format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        // 4. Unauthenticated ping should fail with 401
        let req = Request::builder()
            .method("POST")
            .uri("/api/admin/monitors/mon-pub/ping")
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

        // 5. Authenticated ping to a valid monitor executes out-of-band and persists log
        let req = Request::builder()
            .method("POST")
            .uri("/api/admin/monitors/mon-pub/ping")
            .header("Authorization", format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = axum::body::to_bytes(res.into_body(), 1024 * 16).await.unwrap();
        let ping_res: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(ping_res["status"] == "UP" || ping_res["status"] == "DOWN");
        assert!(ping_res.get("latency_ms").is_some());

        // Check that check_logs has a new row (now 4 logs)
        let log_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM check_logs WHERE monitor_id = 'mon-pub'")
            .fetch_one(&state.db)
            .await
            .unwrap();
        assert_eq!(log_count, 4);
    }

    #[tokio::test]
    async fn test_asset_expiry_endpoints() {
        let (app, state) = setup_test_app().await;

        let token = crate::api::auth::create_jwt(
            &state.config.admin_user,
            &state.config.jwt_secret,
        )
        .unwrap();

        // 1. Unauthenticated request should return 401
        let req = Request::builder()
            .method("GET")
            .uri("/api/admin/assets/expiry")
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

        // Seed monitors:
        // mon-crit: SSL expiring in 3 days -> CRITICAL
        sqlx::query(
            "INSERT INTO monitors (id, name, url, is_active, check_ssl, ssl_days_remaining, ssl_expires_at)
             VALUES ('mon-crit', 'Critical SSL', 'https://crit.example.com', 1, 1, 3, '2026-04-01T00:00:00Z')"
        )
        .execute(&state.db)
        .await
        .unwrap();

        // mon-warn: Domain expiring in 25 days -> WARNING
        sqlx::query(
            "INSERT INTO monitors (id, name, url, is_active, check_domain_expiry, domain_days_remaining, domain_expires_at)
             VALUES ('mon-warn', 'Warning Domain', 'https://warn.example.com', 1, 1, 25, '2026-04-20T00:00:00Z')"
        )
        .execute(&state.db)
        .await
        .unwrap();

        // mon-healthy: SSL expiring in 60 days, Domain expiring in 180 days -> HEALTHY
        sqlx::query(
            "INSERT INTO monitors (id, name, url, is_active, check_ssl, ssl_days_remaining, check_domain_expiry, domain_days_remaining)
             VALUES ('mon-healthy', 'Healthy Asset', 'https://healthy.example.com', 1, 1, 60, 1, 180)"
        )
        .execute(&state.db)
        .await
        .unwrap();

        // mon-ignored: No SSL or domain monitoring enabled
        sqlx::query(
            "INSERT INTO monitors (id, name, url, is_active, check_ssl, check_domain_expiry)
             VALUES ('mon-ignored', 'Plain HTTP', 'http://plain.example.com', 1, 0, 0)"
        )
        .execute(&state.db)
        .await
        .unwrap();

        // 2. Authenticated request should return the 3 tracked monitors sorted by MIN days remaining ASC
        let req = Request::builder()
            .method("GET")
            .uri("/api/admin/assets/expiry")
            .header("Authorization", format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let body = axum::body::to_bytes(res.into_body(), 1024 * 64).await.unwrap();
        let items: Vec<AssetExpiryItem> = serde_json::from_slice(&body).unwrap();
        assert_eq!(items.len(), 3);

        // First item must be mon-crit (3 days remaining <= 7 -> CRITICAL)
        assert_eq!(items[0].monitor_id, "mon-crit");
        assert_eq!(items[0].urgency, "CRITICAL");
        assert_eq!(items[0].ssl_days_remaining, Some(3));

        // Second item must be mon-warn (25 days remaining <= 30 -> WARNING)
        assert_eq!(items[1].monitor_id, "mon-warn");
        assert_eq!(items[1].urgency, "WARNING");
        assert_eq!(items[1].domain_days_remaining, Some(25));

        // Third item must be mon-healthy (60 days remaining -> HEALTHY)
        assert_eq!(items[2].monitor_id, "mon-healthy");
        assert_eq!(items[2].urgency, "HEALTHY");

        // 3. Test on-demand refresh endpoint
        // Unauthenticated request should fail (401)
        let req = Request::builder()
            .method("POST")
            .uri("/api/admin/monitors/mon-crit/refresh-assets")
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

        // Authenticated request
        let req = Request::builder()
            .method("POST")
            .uri("/api/admin/monitors/mon-crit/refresh-assets")
            .header("Authorization", format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = axum::body::to_bytes(res.into_body(), 1024 * 16).await.unwrap();
        let refreshed: AssetExpiryItem = serde_json::from_slice(&body).unwrap();
        assert_eq!(refreshed.monitor_id, "mon-crit");
        assert!(refreshed.ssl_enabled);
    }

    #[tokio::test]
    async fn test_notification_channels_crud_and_monitor_attachment() {
        let (app, state) = setup_test_app().await;

        let token = crate::api::auth::create_jwt(
            &state.config.admin_user,
            &state.config.jwt_secret,
        )
        .unwrap();

        // 1. Unauthenticated requests to /api/admin/channels should return 401
        let req = Request::builder()
            .method("GET")
            .uri("/api/admin/channels")
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

        // 2. Create "Engineering Discord" channel
        let req = Request::builder()
            .method("POST")
            .uri("/api/admin/channels")
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .body(Body::from(
                json!({
                    "name": "Engineering Discord",
                    "type": "discord",
                    "url": "https://discord.com/api/webhooks/12345/abcdef",
                    "is_default": false
                })
                .to_string(),
            ))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);
        let body = axum::body::to_bytes(res.into_body(), 1024 * 16).await.unwrap();
        let chan_discord: crate::db::notifications::NotificationChannel =
            serde_json::from_slice(&body).unwrap();
        assert_eq!(chan_discord.name, "Engineering Discord");
        assert_eq!(chan_discord.channel_type, "discord");
        assert!(!chan_discord.is_default);

        // 3. Create "Ops Slack" channel (default = true)
        let req = Request::builder()
            .method("POST")
            .uri("/api/admin/channels")
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .body(Body::from(
                json!({
                    "name": "Ops Slack",
                    "type": "slack",
                    "url": "https://hooks.slack.com/services/T00/B00/X00",
                    "is_default": true
                })
                .to_string(),
            ))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);
        let body = axum::body::to_bytes(res.into_body(), 1024 * 16).await.unwrap();
        let chan_slack: crate::db::notifications::NotificationChannel =
            serde_json::from_slice(&body).unwrap();
        assert_eq!(chan_slack.name, "Ops Slack");
        assert!(chan_slack.is_default);

        // 4. List channels
        let req = Request::builder()
            .method("GET")
            .uri("/api/admin/channels")
            .header("Authorization", format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = axum::body::to_bytes(res.into_body(), 1024 * 16).await.unwrap();
        let channels: Vec<crate::db::notifications::NotificationChannel> =
            serde_json::from_slice(&body).unwrap();
        assert_eq!(channels.len(), 2);

        // 5. Update "Engineering Discord" channel
        let req = Request::builder()
            .method("PUT")
            .uri(format!("/api/admin/channels/{}", chan_discord.id))
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .body(Body::from(
                json!({
                    "name": "Engineering Discord Updated"
                })
                .to_string(),
            ))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = axum::body::to_bytes(res.into_body(), 1024 * 16).await.unwrap();
        let updated_chan: crate::db::notifications::NotificationChannel =
            serde_json::from_slice(&body).unwrap();
        assert_eq!(updated_chan.name, "Engineering Discord Updated");

        // 6. Create a monitor attaching Engineering Discord explicitly
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/monitors")
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .body(Body::from(
                json!({
                    "name": "Backend API Service",
                    "url": "https://api.example.com/healthz",
                    "channel_ids": [chan_discord.id]
                })
                .to_string(),
            ))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);
        let body = axum::body::to_bytes(res.into_body(), 1024 * 16).await.unwrap();
        let created_mon: Monitor = serde_json::from_slice(&body).unwrap();

        // 7. Verify monitor details returns the attached channel ID
        let req = Request::builder()
            .method("GET")
            .uri(format!("/api/admin/monitors/{}/details", created_mon.id))
            .header("Authorization", format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = axum::body::to_bytes(res.into_body(), 1024 * 32).await.unwrap();
        let detail_resp: MonitorDetailResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(detail_resp.monitor.channel_ids, vec![chan_discord.id.clone()]);

        // 8. Verify effective channels returns BOTH explicitly attached + default channel
        let effective = crate::db::notifications::get_effective_channels_for_monitor(
            &state.db,
            &created_mon.id,
        )
        .await
        .unwrap();
        let effective_names: Vec<String> = effective.into_iter().map(|c| c.name).collect();
        assert!(effective_names.contains(&"Engineering Discord Updated".to_string()));
        assert!(effective_names.contains(&"Ops Slack".to_string()));

        // 9. Update monitor to attach both channels
        let req = Request::builder()
            .method("PUT")
            .uri(format!("/api/monitors/{}", created_mon.id))
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .body(Body::from(
                json!({
                    "channel_ids": [chan_discord.id, chan_slack.id]
                })
                .to_string(),
            ))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let attached_ids = crate::db::notifications::get_channel_ids_for_monitor(
            &state.db,
            &created_mon.id,
        )
        .await
        .unwrap();
        assert_eq!(attached_ids.len(), 2);

        // 10. Delete channel
        let req = Request::builder()
            .method("DELETE")
            .uri(format!("/api/admin/channels/{}", chan_discord.id))
            .header("Authorization", format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::NO_CONTENT);

        // Verify monitor association was cascaded
        let attached_ids_after = crate::db::notifications::get_channel_ids_for_monitor(
            &state.db,
            &created_mon.id,
        )
        .await
        .unwrap();
        assert_eq!(attached_ids_after, vec![chan_slack.id]);
    }

    #[tokio::test]
    async fn test_channel_test_and_alert_dispatch() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let (app, state) = setup_test_app().await;

        let token = crate::api::auth::create_jwt(
            &state.config.admin_user,
            &state.config.jwt_secret,
        )
        .unwrap();

        // Spin up a mock HTTP server on 127.0.0.1
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let hit_counter = Arc::new(AtomicUsize::new(0));
        let hit_counter_clone = hit_counter.clone();

        tokio::spawn(async move {
            loop {
                if let Ok((mut socket, _)) = listener.accept().await {
                    let mut buf = [0u8; 2048];
                    let _ = socket.read(&mut buf).await;
                    hit_counter_clone.fetch_add(1, Ordering::SeqCst);
                    let response = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok";
                    let _ = socket.write_all(response.as_bytes()).await;
                }
            }
        });

        // Create channel pointing to mock server
        let mock_url = format!("http://{addr}/test-webhook");
        let req = Request::builder()
            .method("POST")
            .uri("/api/admin/channels")
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .body(Body::from(
                json!({
                    "name": "Mock Test Channel",
                    "type": "webhook",
                    "url": mock_url,
                    "is_default": true
                })
                .to_string(),
            ))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);
        let body = axum::body::to_bytes(res.into_body(), 1024 * 16).await.unwrap();
        let chan: crate::db::notifications::NotificationChannel =
            serde_json::from_slice(&body).unwrap();

        // Test POST /api/admin/channels/:id/test
        let req = Request::builder()
            .method("POST")
            .uri(format!("/api/admin/channels/{}/test", chan.id))
            .header("Authorization", format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert!(hit_counter.load(Ordering::SeqCst) >= 1);

        // Test parallel alert dispatcher decoupling
        let payload = crate::alerting::AlertPayload {
            monitor_id: None,
            monitor_name: "Mock Service".to_string(),
            monitor_url: "https://mock.service".to_string(),
            new_status: "DOWN".to_string(),
            old_status: "UP".to_string(),
            http_status: Some(503),
            response_time_ms: Some(150),
            error_msg: Some("HTTP 503 Service Unavailable".to_string()),
            downtime_duration: None,
            downtime_duration_secs: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            domain_days_remaining: None,
            domain_expires_at: None,
        };

        let initial_hits = hit_counter.load(Ordering::SeqCst);
        crate::alerting::dispatch_alerts(&state.db, &state.http_client, &payload).await;

        // Allow parallel Tokio worker task to execute
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        assert!(hit_counter.load(Ordering::SeqCst) > initial_hits);
    }
}
