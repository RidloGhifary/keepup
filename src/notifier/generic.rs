use reqwest::Client;
use serde_json::Value;

use super::AlertPayload;
use crate::checker::alerting::{format_generic_default, render_custom_template};

/// Deliver an alert to a generic webhook endpoint via HTTP POST.
///
/// Uses the provider's built-in default JSON formatter unless `use_default` is false
/// and a non-empty `custom_template` is provided.
pub async fn send(
    client: &Client,
    webhook_url: &str,
    payload: &AlertPayload,
    use_default: bool,
    custom_template: Option<&str>,
) -> anyhow::Result<()> {
    let resp = if use_default || custom_template.map_or(true, |t| t.trim().is_empty()) {
        let body = format_generic_default(payload);
        client.post(webhook_url).json(&body).send().await?
    } else {
        let rendered = render_custom_template(custom_template.unwrap(), payload);
        if let Ok(json_val) = serde_json::from_str::<Value>(&rendered) {
            client.post(webhook_url).json(&json_val).send().await?
        } else {
            client
                .post(webhook_url)
                .header("Content-Type", "application/json")
                .body(rendered)
                .send()
                .await?
        }
    };

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("Generic webhook returned {status}: {text}");
    }

    Ok(())
}
