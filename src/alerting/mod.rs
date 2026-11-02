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

fn escape_telegram_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Formats a Telegram HTML message text from an alert payload.
pub fn format_telegram_message(payload: &AlertPayload) -> String {
    let is_recovery = payload.new_status == "UP"
        && (payload.old_status == "DOWN" || payload.old_status == "DEGRADED");
    let is_domain_alert = payload.new_status == "DOMAIN_EXPIRING_SOON";
    let is_ssl_alert = payload.new_status == "SSL_EXPIRING_SOON";

    let safe_name = escape_telegram_html(&payload.monitor_name);
    let safe_url = escape_telegram_html(&payload.monitor_url);
    let safe_status = escape_telegram_html(&payload.new_status);

    if is_recovery {
        let latency_str = payload
            .response_time_ms
            .map(|ms| format!("{ms} ms"))
            .unwrap_or_else(|| "N/A".to_string());
        let duration_str =
            escape_telegram_html(payload.downtime_duration.as_deref().unwrap_or("N/A"));

        format!(
            "🟢 <b>[RESOLVED]</b> <b>{safe_name}</b> is back online\n<b>Target:</b> {safe_url}\n<b>Latency:</b> {latency_str}\n<b>Downtime Duration:</b> {duration_str}"
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
        let expires_str =
            escape_telegram_html(payload.domain_expires_at.as_deref().unwrap_or("N/A"));

        format!(
            "⚠️ <b>[{label}]</b> <b>{safe_name}</b>\n<b>Target:</b> {safe_url}\n<b>Days Remaining:</b> {days_str}\n<b>Expiration Date:</b> {expires_str}"
        )
    } else if payload.new_status == "DOWN" || payload.new_status == "DEGRADED" {
        let error_str = escape_telegram_html(
            payload
                .error_msg
                .as_deref()
                .unwrap_or("Check failed or timed out"),
        );
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
            "{icon} <b>[{safe_status}]</b> <b>{safe_name}</b> alert!\n<b>Target:</b> {safe_url}\n<b>HTTP Status:</b> {http_status_str}\n<b>Details:</b> {error_str}"
        )
    } else {
        format!("ℹ️ <b>[{safe_status}]</b> <b>{safe_name}</b>\n<b>Target:</b> {safe_url}")
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
                anyhow::bail!("Discord returned HTTP {status}");
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
                anyhow::bail!("Slack returned HTTP {status}");
            }
        }
        "telegram" => {
            let text = if let Some(tpl) = custom_tpl {
                escape_telegram_html(&crate::checker::alerting::render_custom_template(
                    tpl, payload,
                ))
            } else {
                format_telegram_message(payload)
            };
            let body = json!({
                "text": text,
                "parse_mode": "HTML"
            });
            let resp = client
                .post(&channel.url)
                .timeout(timeout)
                .json(&body)
                .send()
                .await?;

            if !resp.status().is_success() {
                let status = resp.status();
                anyhow::bail!("Telegram returned HTTP {status}");
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
                anyhow::bail!("Webhook returned HTTP {status}");
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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[test]
    fn test_telegram_message_uses_safe_html_escaping() {
        let payload = AlertPayload {
            monitor_id: Some("m1".to_string()),
            monitor_name: "api_v2 <prod> & co".to_string(),
            monitor_url: "https://example.com/health_check?a=1&b=2".to_string(),
            new_status: "DOWN".to_string(),
            old_status: "UP".to_string(),
            http_status: Some(502),
            response_time_ms: Some(120),
            error_msg: Some("upstream <bad_gateway> & timeout".to_string()),
            downtime_duration: None,
            downtime_duration_secs: None,
            timestamp: "2026-11-02T00:00:00Z".to_string(),
            domain_days_remaining: None,
            domain_expires_at: None,
        };

        let msg = format_telegram_message(&payload);
        assert!(msg.contains("<b>[DOWN]</b>"), "Telegram message must use HTML bold tags: {msg}");
        assert!(msg.contains("api_v2 &lt;prod&gt; &amp; co"));
        assert!(msg.contains("upstream &lt;bad_gateway&gt; &amp; timeout"));
    }

    #[tokio::test]
    async fn test_dispatch_single_channel_redacts_upstream_error_body() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            if let Ok((mut sock, _)) = listener.accept().await {
                let mut buf = [0u8; 1024];
                let _ = sock.read(&mut buf).await;
                let resp = "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 25\r\n\r\nSECRET_INTERNAL_BODY_LEAK";
                let _ = sock.write_all(resp.as_bytes()).await;
            }
        });

        let channel = NotificationChannel {
            id: "c1".to_string(),
            name: "Test Hook".to_string(),
            channel_type: "webhook".to_string(),
            url: format!("http://{addr}/hook"),
            is_default: false,
            is_enabled: true,
            use_default_template: true,
            notify_on_resolve: true,
            custom_template: None,
            created_at: "2026-11-02T00:00:00Z".to_string(),
        };

        let err = test_channel(&reqwest::Client::new(), &channel)
            .await
            .expect_err("500 response must fail");
        let err_msg = err.to_string();
        assert!(err_msg.contains("500"));
        assert!(
            !err_msg.contains("SECRET_INTERNAL_BODY_LEAK"),
            "upstream response body must not be echoed in channel error: {err_msg}"
        );
    }
}
