use std::time::Duration;

use reqwest::Client;
use serde_json::{json, Value};
use sqlx::SqlitePool;
use tracing::{error, info};

use crate::db::notifications::NotificationChannel;

/// Alert context passed to each notification channel dispatcher.
#[derive(Debug, Clone)]
pub struct AlertPayload {
    /// Optional monitor ID.
    pub monitor_id: Option<String>,
    /// Human-readable monitor name.
    pub monitor_name: String,
    /// URL being monitored.
    pub monitor_url: String,
    /// New status after the transition (`"DOWN"`, `"UP"`, `"DEGRADED"`, `"DOMAIN_EXPIRING_SOON"`, `"SSL_EXPIRING_SOON"`).
    pub new_status: String,
    /// Previous status before the transition.
    pub old_status: String,
    /// HTTP status code from check (e.g. 200, 500).
    pub http_status: Option<u16>,
    /// Response time / latency in milliseconds.
    pub response_time_ms: Option<u64>,
    /// Error message (populated when transitioning to DOWN or DEGRADED).
    pub error_msg: Option<String>,
    /// Human-readable downtime duration (populated when recovering to UP).
    pub downtime_duration: Option<String>,
    /// Downtime duration in seconds (populated when recovering to UP).
    pub downtime_duration_secs: Option<u64>,
    /// ISO 8601 timestamp string.
    pub timestamp: String,
    /// Days remaining until domain registration or certificate expires.
    pub domain_days_remaining: Option<i64>,
    /// Formatted expiration timestamp of domain registration or certificate.
    pub domain_expires_at: Option<String>,
}

/// Formats a telegram message text from an alert payload.
pub fn format_telegram_message(payload: &AlertPayload) -> String {
    let is_recovery = payload.new_status == "UP"
        && (payload.old_status == "DOWN" || payload.old_status == "DEGRADED");
    let is_domain_alert = payload.new_status == "DOMAIN_EXPIRING_SOON";
    let is_ssl_alert = payload.new_status == "SSL_EXPIRING_SOON";

    if is_recovery {
        let latency_str = payload
            .response_time_ms
            .map(|ms| format!("{ms} ms"))
            .unwrap_or_else(|| "N/A".to_string());
        let duration_str = payload.downtime_duration.as_deref().unwrap_or("N/A");

        format!(
            "🟢 *[RESOLVED]* *{}* is back online\n*Target:* {}\n*Latency:* {}\n*Downtime Duration:* {}",
            payload.monitor_name, payload.monitor_url, latency_str, duration_str
        )
    } else if is_domain_alert || is_ssl_alert {
        let label = if is_ssl_alert {
            "SSL EXPIRING"
        } else {
            "DOMAIN EXPIRING"
        };
        let days_str = payload
            .domain_days_remaining
            .map(|d| format!("{d} days"))
            .unwrap_or_else(|| "N/A".to_string());
        let expires_str = payload.domain_expires_at.as_deref().unwrap_or("N/A");

        format!(
            "⚠️ *[{label}]* *{}*\n*Target:* {}\n*Days Remaining:* {}\n*Expiration Date:* {}",
            payload.monitor_name, payload.monitor_url, days_str, expires_str
        )
    } else if payload.new_status == "DOWN" || payload.new_status == "DEGRADED" {
        let error_str = payload
            .error_msg
            .as_deref()
            .unwrap_or("Check failed or timed out");
        let http_status_str = payload
            .http_status
            .map(|st| st.to_string())
            .unwrap_or_else(|| "N/A".to_string());
        let icon = if payload.new_status == "DEGRADED" {
            "🟠"
        } else {
            "🔴"
        };

        format!(
            "{icon} *[{}]* *{}* alert!\n*Target:* {}\n*HTTP Status:* {}\n*Details:* {}",
            payload.new_status, payload.monitor_name, payload.monitor_url, http_status_str, error_str
        )
    } else {
        format!(
            "ℹ️ *[{}]* *{}*\n*Target:* {}",
            payload.new_status, payload.monitor_name, payload.monitor_url
        )
    }
}

/// Send an alert payload to a single notification channel.
/// Enforces a strict 5-second request timeout to avoid hanging the Tokio worker.
pub async fn dispatch_single_channel(
    client: &Client,
    channel: &NotificationChannel,
    payload: &AlertPayload,
) -> anyhow::Result<()> {
    let channel_type = channel.channel_type.to_lowercase();
    let timeout = Duration::from_secs(5);
    let custom_tpl = if !channel.use_default_template {
        channel
            .custom_template
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
    } else {
        None
    };

    match channel_type.as_str() {
        "discord" => {
            let body = if let Some(tpl) = custom_tpl {
                let rendered = crate::checker::alerting::render_custom_template(tpl, payload);
                match serde_json::from_str::<Value>(&rendered) {
                    Ok(v) if v.is_object() => v,
                    _ => json!({ "content": rendered }),
                }
            } else {
                crate::checker::alerting::format_discord_default(payload)
            };
            let resp = client
                .post(&channel.url)
                .timeout(timeout)
                .json(&body)
                .send()
                .await?;

            if !resp.status().is_success() {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                anyhow::bail!("Discord returned {status}: {text}");
            }
        }
        "slack" => {
            let body = if let Some(tpl) = custom_tpl {
                let rendered = crate::checker::alerting::render_custom_template(tpl, payload);
                match serde_json::from_str::<Value>(&rendered) {
                    Ok(v) if v.is_object() => v,
                    _ => json!({ "text": rendered }),
                }
            } else {
                crate::checker::alerting::format_slack_default(payload)
            };
            let resp = client
                .post(&channel.url)
                .timeout(timeout)
                .json(&body)
                .send()
                .await?;

            if !resp.status().is_success() {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                anyhow::bail!("Slack returned {status}: {text}");
            }
        }
        "telegram" => {
            let text = if let Some(tpl) = custom_tpl {
                crate::checker::alerting::render_custom_template(tpl, payload)
            } else {
                format_telegram_message(payload)
            };
            let body = json!({
                "text": text,
                "parse_mode": "Markdown"
            });
            let resp = client
                .post(&channel.url)
                .timeout(timeout)
                .json(&body)
                .send()
                .await?;

            if !resp.status().is_success() {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                anyhow::bail!("Telegram returned {status}: {text}");
            }
        }
        "webhook" | "generic" | _ => {
            let resp = if let Some(tpl) = custom_tpl {
                let rendered = crate::checker::alerting::render_custom_template(tpl, payload);
                if let Ok(json_val) = serde_json::from_str::<Value>(&rendered) {
                    client
                        .post(&channel.url)
                        .timeout(timeout)
                        .json(&json_val)
                        .send()
                        .await?
                } else {
                    client
                        .post(&channel.url)
                        .timeout(timeout)
                        .header("Content-Type", "text/plain; charset=utf-8")
                        .body(rendered)
                        .send()
                        .await?
                }
            } else {
                let body: Value = crate::checker::alerting::format_generic_default(payload);
                client
                    .post(&channel.url)
                    .timeout(timeout)
                    .json(&body)
                    .send()
                    .await?
            };

            if !resp.status().is_success() {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                anyhow::bail!("Webhook returned {status}: {text}");
            }
        }
    }

    Ok(())
}

/// Dispatches an instant test alert to verify a notification channel's configuration.
pub async fn test_channel(client: &Client, channel: &NotificationChannel) -> anyhow::Result<()> {
    let test_payload = AlertPayload {
        monitor_id: Some("test-channel-probe".to_string()),
        monitor_name: "KeepUp Health Monitor".to_string(),
        monitor_url: "https://keepup.local".to_string(),
        new_status: "UP".to_string(),
        old_status: "DOWN".to_string(),
        http_status: Some(200),
        response_time_ms: Some(42),
        error_msg: None,
        downtime_duration: Some("1m 15s".to_string()),
        downtime_duration_secs: Some(75),
        timestamp: chrono::Utc::now().to_rfc3339(),
        domain_days_remaining: None,
        domain_expires_at: None,
    };

    dispatch_single_channel(client, channel, &test_payload).await
}

/// Centralized alert dispatcher across unified `notification_channels`.
pub async fn dispatch_alerts(pool: &SqlitePool, client: &Client, payload: &AlertPayload) {
    if let Some(ref mid) = payload.monitor_id {
        if crate::db::maintenance::is_monitor_in_maintenance(pool, mid)
            .await
            .unwrap_or(false)
        {
            info!(
                monitor = %payload.monitor_name,
                monitor_id = %mid,
                "Bypassing notification channels dispatch: monitor is currently inside an active maintenance window"
            );
            return;
        }
    }

    let channels = match &payload.monitor_id {
        Some(mid) => crate::db::notifications::get_effective_channels_for_monitor(pool, mid)
            .await
            .unwrap_or_default(),
        None => crate::db::notifications::list_channels(pool)
            .await
            .unwrap_or_default()
            .into_iter()
            .filter(|c| c.is_default && c.is_enabled)
            .collect(),
    };

    let is_recovery = payload.new_status == "UP"
        && (payload.old_status == "DOWN" || payload.old_status == "DEGRADED");

    let eligible: Vec<NotificationChannel> = channels
        .into_iter()
        .filter(|c| c.is_enabled && (!is_recovery || c.notify_on_resolve))
        .collect();

    if !eligible.is_empty() {
        info!(
            count = eligible.len(),
            monitor = %payload.monitor_name,
            transition = %format!("{} → {}", payload.old_status, payload.new_status),
            "Dispatching alerts across parallel notification channels"
        );

        for channel in eligible {
            let client = client.clone();
            let payload = payload.clone();
            let channel_name = channel.name.clone();
            let channel_id = channel.id.clone();
            let channel_type = channel.channel_type.clone();

            tokio::spawn(async move {
                match dispatch_single_channel(&client, &channel, &payload).await {
                    Ok(()) => {
                        info!(
                            channel_id = %channel_id,
                            channel_name = %channel_name,
                            channel_type = %channel_type,
                            "Alert delivered successfully"
                        );
                    }
                    Err(err) => {
                        error!(
                            channel_id = %channel_id,
                            channel_name = %channel_name,
                            channel_type = %channel_type,
                            "Alert delivery failed: {err}"
                        );
                    }
                }
            });
        }
    }
}
