use reqwest::Client;
use serde_json::Value;

use super::AlertPayload;
use crate::checker::alerting::{format_discord_default, render_custom_template};

/// Deliver an alert to a Discord webhook.
///
/// Uses the provider's built-in default formatter (rich embed with status
/// colors #30D158 Green for UP, #FF453A Red for DOWN) unless `use_default` is false
/// and a non-empty `custom_template` is provided.
pub async fn send(
    client: &Client,
    webhook_url: &str,
    payload: &AlertPayload,
    use_default: bool,
    custom_template: Option<&str>,
) -> anyhow::Result<()> {
    let body: Value = if use_default || custom_template.map_or(true, |t| t.trim().is_empty()) {
        format_discord_default(payload)
    } else {
        let rendered = render_custom_template(custom_template.unwrap(), payload);
        if let Ok(json_val) = serde_json::from_str::<Value>(&rendered) {
            json_val
        } else {
            serde_json::json!({ "content": rendered })
        }
    };

    let resp = client.post(webhook_url).json(&body).send().await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("Discord webhook returned {status}: {text}");
    }

    Ok(())
}
