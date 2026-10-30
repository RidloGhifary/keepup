use reqwest::Client;
use serde_json::Value;

use super::AlertPayload;
use crate::checker::alerting::{format_slack_default, render_custom_template};

/// Deliver an alert to a Slack incoming webhook using Block Kit formatting.
///
/// Uses the provider's built-in default formatter (Block Kit with :white_check_mark:
/// and :rotating_light: icons, monitor name, response time, and outage duration)
/// unless `use_default` is false and a non-empty `custom_template` is provided.
pub async fn send(
    client: &Client,
    webhook_url: &str,
    payload: &AlertPayload,
    use_default: bool,
    custom_template: Option<&str>,
) -> anyhow::Result<()> {
    let body: Value = if use_default || custom_template.map_or(true, |t| t.trim().is_empty()) {
        format_slack_default(payload)
    } else {
        let rendered = render_custom_template(custom_template.unwrap(), payload);
        if let Ok(json_val) = serde_json::from_str::<Value>(&rendered) {
            json_val
        } else {
            serde_json::json!({ "text": rendered })
        }
    };

    let resp = client.post(webhook_url).json(&body).send().await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("Slack webhook returned {status}: {text}");
    }

    Ok(())
}
