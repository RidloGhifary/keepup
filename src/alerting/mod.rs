use std::time::Duration;

use reqwest::Client;
use serde_json::Value;
use sqlx::SqlitePool;
use tracing::{error, info};

use crate::db::notifications::NotificationChannel;
use crate::notifier::AlertPayload;

/// Formats a telegram message text from an alert payload.
pub fn format_telegram_message(payload: &AlertPayload) -> String {
    let is_recovery = payload.new_status == "UP" && payload.old_status == "DOWN";
    let is_domain_alert = payload.new_status == "DOMAIN_EXPIRING_SOON";

    if is_recovery {
        let latency_str = payload
            .response_time_ms
            .map(|ms| format!("{ms} ms"))
            .unwrap_or_else(|| "N/A".to_string());
        let duration_str = payload
            .downtime_duration
            .as_deref()
            .unwrap_or("N/A");

        format!(
            "🟢 *[RESOLVED]* *{}* is back online\n*Target:* {}\n*Latency:* {}\n*Downtime Duration:* {}",
            payload.monitor_name, payload.monitor_url, latency_str, duration_str
        )
    } else if is_domain_alert {
        let days_str = payload
            .domain_days_remaining
            .map(|d| format!("{d} days"))
            .unwrap_or_else(|| "N/A".to_string());
        let expires_str = payload
            .domain_expires_at
            .as_deref()
            .unwrap_or("N/A");

        format!(
            "⚠️ *[DOMAIN EXPIRING]* *{}*\n*Target:* {}\n*Days Remaining:* {}\n*Expiration Date:* {}",
            payload.monitor_name, payload.monitor_url, days_str, expires_str
        )
    } else if payload.new_status == "DOWN" {
        let error_str = payload
            .error_msg
            .as_deref()
            .unwrap_or("Check failed or timed out");
        let http_status_str = payload
            .http_status
            .map(|st| st.to_string())
            .unwrap_or_else(|| "N/A".to_string());

        format!(
            "🔴 *[DOWN]* *{}* is unreachable!\n*Target:* {}\n*HTTP Status:* {}\n*Error:* {}",
            payload.monitor_name, payload.monitor_url, http_status_str, error_str
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

    match channel_type.as_str() {
        "discord" => {
            let body = crate::checker::alerting::format_discord_default(payload);
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
            let body = crate::checker::alerting::format_slack_default(payload);
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
            let text = format_telegram_message(payload);
            let body = serde_json::json!({
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
            let body: Value = crate::checker::alerting::format_generic_default(payload);
            let resp = client
                .post(&channel.url)
                .timeout(timeout)
                .json(&body)
                .send()
                .await?;

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

/// Centralized alert dispatcher.
///
/// When a monitor state changes:
/// 1. Verifies if the monitor is within an active maintenance window (suppressing alerts if active).
/// 2. Queries all associated channels via `monitor_notification_channels` + any default channels (`is_default = 1`).
/// 3. Spawns asynchronous delivery tasks across Tokio workers (`tokio::spawn`) in parallel.
/// 4. Dispatches to legacy webhooks for backwards compatibility.
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

    // Query effective notification channels
    let channels = match &payload.monitor_id {
        Some(mid) => {
            crate::db::notifications::get_effective_channels_for_monitor(pool, mid)
                .await
                .unwrap_or_default()
        }
        None => {
            crate::db::notifications::list_channels(pool)
                .await
                .unwrap_or_default()
                .into_iter()
                .filter(|c| c.is_default)
                .collect()
        }
    };

    if !channels.is_empty() {
        info!(
            count = channels.len(),
            monitor = %payload.monitor_name,
            transition = %format!("{} → {}", payload.old_status, payload.new_status),
            "Dispatching alerts across parallel notification channels"
        );

        for channel in channels {
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

    // Also dispatch to legacy webhooks if any are configured
    crate::notifier::dispatch_alerts(pool, client, payload).await;
}
