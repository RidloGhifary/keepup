pub mod discord;
pub mod generic;
pub mod slack;

use reqwest::Client;
use sqlx::SqlitePool;
use tracing::{error, info};

/// Channel type matching the `provider` column in the `webhooks` table.
#[derive(Debug)]
pub enum Provider {
    Discord,
    Slack,
    Generic,
}

/// Row projection from the `webhooks` table.
#[derive(sqlx::FromRow)]
struct Webhook {
    #[allow(dead_code)]
    id: String,
    name: String,
    provider: String,
    url: String,
    use_default_template: bool,
    notify_on_resolve: bool,
    message_template: Option<String>,
}

/// Alert context passed to each webhook dispatcher.
#[derive(Debug, Clone)]
pub struct AlertPayload {
    /// Optional monitor ID.
    pub monitor_id: Option<String>,
    /// Human-readable monitor name.
    pub monitor_name: String,
    /// URL being monitored.
    pub monitor_url: String,
    /// New status after the transition (`"DOWN"` or `"UP"`).
    pub new_status: String,
    /// Previous status before the transition.
    pub old_status: String,
    /// HTTP status code from check (e.g. 200, 500).
    pub http_status: Option<u16>,
    /// Response time / latency in milliseconds.
    pub response_time_ms: Option<u64>,
    /// Error message (populated when transitioning to DOWN).
    pub error_msg: Option<String>,
    /// Human-readable downtime duration (populated when recovering to UP).
    pub downtime_duration: Option<String>,
    /// Downtime duration in seconds (populated when recovering to UP).
    pub downtime_duration_secs: Option<u64>,
    /// ISO 8601 timestamp string.
    pub timestamp: String,
}

/// Query all enabled webhooks and dispatch `payload` to each one.
///
/// Errors are logged but never propagated—alerting failures must not crash
/// the scheduler.
pub async fn dispatch_alerts(pool: &SqlitePool, client: &Client, payload: &AlertPayload) {
    let webhooks = match fetch_enabled_webhooks(pool).await {
        Ok(w) => w,
        Err(e) => {
            error!("Failed to query webhooks: {e}");
            return;
        }
    };

    if webhooks.is_empty() {
        return;
    }

    if let Some(ref mid) = payload.monitor_id {
        if crate::db::maintenance::is_monitor_in_maintenance(pool, mid).await.unwrap_or(false) {
            info!(
                monitor = %payload.monitor_name,
                monitor_id = %mid,
                "Bypassing webhook alert dispatch: monitor is currently inside an active maintenance window"
            );
            return;
        }
    }

    info!(
        count = webhooks.len(),
        monitor = %payload.monitor_name,
        transition = %format!("{} → {}", payload.old_status, payload.new_status),
        "Dispatching alerts"
    );

    let is_recovery = payload.new_status == "UP" && payload.old_status == "DOWN";

    for wh in &webhooks {
        if is_recovery && !wh.notify_on_resolve {
            info!(webhook = %wh.name, "Skipping recovery notification because notify_on_resolve is disabled");
            continue;
        }

        let provider = match wh.provider.to_lowercase().as_str() {
            "discord" => Provider::Discord,
            "slack" => Provider::Slack,
            "generic" | "generic_webhook" => Provider::Generic,
            other => {
                error!(webhook = %wh.name, provider = other, "Unknown webhook provider, skipping");
                continue;
            }
        };

        let use_default = wh.use_default_template;
        let template = wh.message_template.as_deref();

        let result = match provider {
            Provider::Discord => discord::send(client, &wh.url, payload, use_default, template).await,
            Provider::Slack => slack::send(client, &wh.url, payload, use_default, template).await,
            Provider::Generic => generic::send(client, &wh.url, payload, use_default, template).await,
        };

        match result {
            Ok(()) => info!(webhook = %wh.name, ?provider, "Alert delivered"),
            Err(e) => error!(webhook = %wh.name, ?provider, "Alert delivery failed: {e}"),
        }
    }
}

/// Return all webhooks that are currently enabled.
async fn fetch_enabled_webhooks(pool: &SqlitePool) -> Result<Vec<Webhook>, sqlx::Error> {
    sqlx::query_as::<_, Webhook>(
        "SELECT id, name, provider, url, use_default_template, notify_on_resolve, message_template \
         FROM webhooks WHERE is_enabled = 1",
    )
    .fetch_all(pool)
    .await
}
