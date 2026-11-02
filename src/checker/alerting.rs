use serde_json::{json, Value};
use crate::alerting::AlertPayload;

/// Status colors used for Discord rich embeds.
pub const COLOR_STATUS_GREEN: u32 = 0x30D158; // #30D158
pub const COLOR_STATUS_RED: u32 = 0xFF453A;   // #FF453A
pub const COLOR_STATUS_AMBER: u32 = 0xFFD60A; // #FFD60A

/// Formats the built-in default Discord rich embed notification.
///
/// Features status colors (#30D158 Green for UP, #FF453A Red for DOWN, #FFD60A Amber for DOMAIN_EXPIRING_SOON),
/// service name, target URL, latency in ms, HTTP status code, and timestamp.
pub fn format_discord_default(payload: &AlertPayload) -> Value {
    if payload.new_status == "UP" && payload.old_status == "DOWN" {
        return format_discord_recovery(payload);
    }

    let is_domain_alert = payload.new_status == "DOMAIN_EXPIRING_SOON";
    let is_up = payload.new_status.to_uppercase() == "UP";
    let is_maint = payload.new_status.to_uppercase() == "MAINTENANCE";
    let color = if is_domain_alert {
        COLOR_STATUS_AMBER
    } else if is_up {
        COLOR_STATUS_GREEN
    } else if is_maint {
        0xA855F7
    } else {
        COLOR_STATUS_RED
    };
    let icon = if is_domain_alert {
        "⚠️"
    } else if is_up {
        "🟢"
    } else if is_maint {
        "🔧"
    } else {
        "🔴"
    };
    let title = if is_domain_alert {
        format!("⚠️ {} Domain Registration Expiring Soon", payload.monitor_name)
    } else {
        format!("{icon} {} is {}", payload.monitor_name, payload.new_status)
    };

    let latency_str = payload
        .response_time_ms
        .map(|ms| format!("{ms} ms"))
        .unwrap_or_else(|| "N/A".to_string());

    let http_status_str = payload
        .http_status
        .map(|st| st.to_string())
        .unwrap_or_else(|| "N/A".to_string());

    let mut fields = vec![
        json!({ "name": "Service", "value": payload.monitor_name, "inline": true }),
        json!({ "name": "Status", "value": payload.new_status, "inline": true }),
        json!({ "name": "Target URL", "value": payload.monitor_url, "inline": false }),
        json!({ "name": "Latency", "value": latency_str, "inline": true }),
        json!({ "name": "HTTP Status", "value": http_status_str, "inline": true }),
    ];

    if is_domain_alert {
        if let Some(days) = payload.domain_days_remaining {
            fields.push(json!({ "name": "Domain Days Remaining", "value": format!("{days} days"), "inline": true }));
        }
        if let Some(ref exp) = payload.domain_expires_at {
            fields.push(json!({ "name": "Domain Expiration Date", "value": exp, "inline": true }));
        }
    }

    if let Some(ref duration) = payload.downtime_duration {
        fields.push(json!({ "name": "Downtime Duration", "value": duration, "inline": false }));
    }

    if let Some(ref err) = payload.error_msg {
        fields.push(json!({ "name": "Error Details", "value": err, "inline": false }));
    }

    json!({
        "embeds": [{
            "title": title,
            "color": color,
            "fields": fields,
            "timestamp": payload.timestamp,
            "footer": { "text": "KeepUp Uptime Monitor" }
        }]
    })
}

/// Formats the built-in default Discord rich embed for incident recovery (DOWN → UP).
pub fn format_discord_recovery(payload: &AlertPayload) -> Value {
    let title = format!("[RESOLVED] {} is back online", payload.monitor_name);
    let color = COLOR_STATUS_GREEN;

    let latency_str = payload
        .response_time_ms
        .map(|ms| format!("{ms} ms"))
        .unwrap_or_else(|| "N/A".to_string());

    let http_status_str = match payload.http_status {
        Some(200) => "200 OK".to_string(),
        Some(code) => code.to_string(),
        None => "200 OK".to_string(),
    };

    let duration_str = payload
        .downtime_duration
        .clone()
        .unwrap_or_else(|| "N/A".to_string());

    let fields = vec![
        json!({ "name": "Downtime Duration", "value": duration_str, "inline": true }),
        json!({ "name": "Current Latency", "value": latency_str, "inline": true }),
        json!({ "name": "HTTP Status", "value": http_status_str, "inline": true }),
        json!({ "name": "Resolved At", "value": payload.timestamp, "inline": false }),
    ];

    json!({
        "embeds": [{
            "title": title,
            "color": color,
            "fields": fields,
            "timestamp": payload.timestamp,
            "footer": { "text": "KeepUp Uptime Monitor" }
        }]
    })
}

/// Formats the built-in default Slack Block Kit layout notification.
///
/// Features status icons (:white_check_mark: for UP, :rotating_light: for DOWN, :warning: for DOMAIN_EXPIRING_SOON),
/// monitor name, response time, and outage duration.
pub fn format_slack_default(payload: &AlertPayload) -> Value {
    if payload.new_status == "UP" && payload.old_status == "DOWN" {
        return format_slack_recovery(payload);
    }

    let is_domain_alert = payload.new_status == "DOMAIN_EXPIRING_SOON";
    let is_up = payload.new_status.to_uppercase() == "UP";
    let is_maint = payload.new_status.to_uppercase() == "MAINTENANCE";
    let icon = if is_domain_alert {
        ":warning:"
    } else if is_up {
        ":white_check_mark:"
    } else if is_maint {
        ":wrench:"
    } else {
        ":rotating_light:"
    };
    let header_text = if is_domain_alert {
        format!("{icon} *{}* Domain Registration Expiring Soon", payload.monitor_name)
    } else {
        format!("{icon} *{}* is *{}*", payload.monitor_name, payload.new_status)
    };

    let latency_str = payload
        .response_time_ms
        .map(|ms| format!("{ms} ms"))
        .unwrap_or_else(|| "N/A".to_string());

    let mut fields = vec![
        json!({ "type": "mrkdwn", "text": format!("*Monitor:*\n{}", payload.monitor_name) }),
        json!({ "type": "mrkdwn", "text": format!("*Status:*\n{}", payload.new_status) }),
        json!({ "type": "mrkdwn", "text": format!("*Target URL:*\n{}", payload.monitor_url) }),
        json!({ "type": "mrkdwn", "text": format!("*Response Time:*\n{}", latency_str) }),
    ];

    if is_domain_alert {
        if let Some(days) = payload.domain_days_remaining {
            fields.push(json!({
                "type": "mrkdwn",
                "text": format!("*Domain Days Remaining:*\n{days} days")
            }));
        }
        if let Some(ref exp) = payload.domain_expires_at {
            fields.push(json!({
                "type": "mrkdwn",
                "text": format!("*Domain Expiration:*\n{exp}")
            }));
        }
    }

    if let Some(ref duration) = payload.downtime_duration {
        fields.push(json!({
            "type": "mrkdwn",
            "text": format!("*Outage Duration:*\n{}", duration)
        }));
    }

    let mut blocks = vec![
        json!({
            "type": "header",
            "text": {
                "type": "plain_text",
                "text": header_text,
                "emoji": true
            }
        }),
        json!({
            "type": "section",
            "fields": fields
        }),
    ];

    if let Some(ref err) = payload.error_msg {
        blocks.push(json!({
            "type": "context",
            "elements": [{
                "type": "mrkdwn",
                "text": format!("*Error:* {}", err)
            }]
        }));
    }

    blocks.push(json!({ "type": "divider" }));
    blocks.push(json!({
        "type": "context",
        "elements": [{
            "type": "mrkdwn",
            "text": "Sent by *KeepUp* uptime monitor"
        }]
    }));

    json!({ "blocks": blocks })
}

/// Formats the built-in default Slack Block Kit layout for incident recovery (DOWN → UP).
pub fn format_slack_recovery(payload: &AlertPayload) -> Value {
    let header_text = format!("[RESOLVED] {}", payload.monitor_name);
    let duration = payload.downtime_duration.as_deref().unwrap_or("N/A");
    let text = format!("Service is operational. Total downtime: {duration}.");

    let latency_str = payload
        .response_time_ms
        .map(|ms| format!("{ms} ms"))
        .unwrap_or_else(|| "N/A".to_string());

    let http_status_str = match payload.http_status {
        Some(200) => "200 OK".to_string(),
        Some(code) => code.to_string(),
        None => "200 OK".to_string(),
    };

    let blocks = vec![
        json!({
            "type": "header",
            "text": {
                "type": "plain_text",
                "text": format!(":white_check_mark: {header_text}"),
                "emoji": true
            }
        }),
        json!({
            "type": "section",
            "text": {
                "type": "mrkdwn",
                "text": text
            }
        }),
        json!({
            "type": "section",
            "fields": [
                { "type": "mrkdwn", "text": format!("*Target URL:*\n{}", payload.monitor_url) },
                { "type": "mrkdwn", "text": format!("*Current Latency:*\n{}", latency_str) },
                { "type": "mrkdwn", "text": format!("*HTTP Status:*\n{}", http_status_str) },
                { "type": "mrkdwn", "text": format!("*Resolved At:*\n{}", payload.timestamp) }
            ]
        }),
        json!({ "type": "divider" }),
        json!({
            "type": "context",
            "elements": [{
                "type": "mrkdwn",
                "text": "Sent by *KeepUp* uptime monitor"
            }]
        })
    ];

    json!({ "blocks": blocks })
}

/// Formats the built-in default Generic HTTP JSON notification.
pub fn format_generic_default(payload: &AlertPayload) -> Value {
    if payload.new_status == "UP" && payload.old_status == "DOWN" {
        return format_generic_recovery(payload);
    }

    let monitor_id = payload.monitor_id.clone().unwrap_or_default();
    let is_domain_alert = payload.new_status == "DOMAIN_EXPIRING_SOON";
    let event = if is_domain_alert {
        "domain.expiring_soon"
    } else if payload.monitor_id.as_deref() == Some("test-monitor-001") {
        "test_notification"
    } else {
        "status_change"
    };

    let mut obj = json!({
        "event": event,
        "monitor_id": monitor_id,
        "monitor_name": payload.monitor_name,
        "target_url": payload.monitor_url,
        "status": payload.new_status,
        "response_time_ms": payload.response_time_ms.unwrap_or(0),
        "http_status": payload.http_status.unwrap_or(0),
        "timestamp": payload.timestamp
    });

    if is_domain_alert {
        obj["domain_days_remaining"] = json!(payload.domain_days_remaining);
        obj["domain_expires_at"] = json!(payload.domain_expires_at);
    }

    obj
}

/// Formats the built-in default Generic HTTP JSON recovery notification (DOWN → UP).
pub fn format_generic_recovery(payload: &AlertPayload) -> Value {
    let monitor_id = payload.monitor_id.clone().unwrap_or_default();
    json!({
        "event": "monitor.recovered",
        "monitor_id": monitor_id,
        "monitor_name": payload.monitor_name,
        "target_url": payload.monitor_url,
        "status": "UP",
        "downtime_duration_secs": payload.downtime_duration_secs.unwrap_or(0),
        "response_time_ms": payload.response_time_ms.unwrap_or(0),
        "timestamp": payload.timestamp
    })
}

/// Interpolates standard placeholder tokens in a custom user-defined template.
pub fn render_custom_template(template: &str, payload: &AlertPayload) -> String {
    let latency_str = payload
        .response_time_ms
        .map(|ms| ms.to_string())
        .unwrap_or_else(|| "0".to_string());
    let http_status_str = payload
        .http_status
        .map(|st| st.to_string())
        .unwrap_or_else(|| "0".to_string());
    let monitor_id = payload.monitor_id.as_deref().unwrap_or("");
    let duration_secs_str = payload
        .downtime_duration_secs
        .map(|s| s.to_string())
        .unwrap_or_else(|| "0".to_string());
    let domain_days_str = payload
        .domain_days_remaining
        .map(|d| d.to_string())
        .unwrap_or_else(|| "N/A".to_string());
    let domain_exp_str = payload
        .domain_expires_at
        .as_deref()
        .unwrap_or("N/A");

    template
        .replace("{{monitor_id}}", monitor_id)
        .replace("{{monitor_name}}", &payload.monitor_name)
        .replace("{{target_url}}", &payload.monitor_url)
        .replace("{{monitor_url}}", &payload.monitor_url)
        .replace("{{status}}", &payload.new_status)
        .replace("{{new_status}}", &payload.new_status)
        .replace("{{old_status}}", &payload.old_status)
        .replace("{{previous_status}}", &payload.old_status)
        .replace("{{response_time_ms}}", &latency_str)
        .replace("{{http_status}}", &http_status_str)
        .replace("{{error}}", payload.error_msg.as_deref().unwrap_or(""))
        .replace("{{downtime_duration}}", payload.downtime_duration.as_deref().unwrap_or(""))
        .replace("{{downtime_duration_secs}}", &duration_secs_str)
        .replace("{{timestamp}}", &payload.timestamp)
        .replace("{{domain_days_remaining}}", &domain_days_str)
        .replace("{{domain_expires_at}}", domain_exp_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_payload(is_up: bool) -> AlertPayload {
        AlertPayload {
            monitor_id: Some("mon-123".to_string()),
            monitor_name: "API Service".to_string(),
            monitor_url: "https://api.example.com/health".to_string(),
            new_status: if is_up { "UP".to_string() } else { "DOWN".to_string() },
            old_status: if is_up { "DOWN".to_string() } else { "UP".to_string() },
            http_status: if is_up { Some(200) } else { Some(503) },
            response_time_ms: Some(45),
            error_msg: if is_up { None } else { Some("Service Unavailable".to_string()) },
            downtime_duration: if is_up { Some("4m 12s".to_string()) } else { None },
            downtime_duration_secs: if is_up { Some(252) } else { None },
            timestamp: "2026-10-30T18:30:00Z".to_string(),
            domain_days_remaining: None,
            domain_expires_at: None,
        }
    }

    #[test]
    fn test_format_domain_expiring_soon_alerts() {
        let mut payload = sample_payload(true);
        payload.new_status = "DOMAIN_EXPIRING_SOON".to_string();
        payload.old_status = "UP".to_string();
        payload.domain_days_remaining = Some(14);
        payload.domain_expires_at = Some("2026-11-15T00:00:00Z".to_string());

        // Discord Amber Embed
        let discord = format_discord_default(&payload);
        assert_eq!(discord["embeds"][0]["color"], COLOR_STATUS_AMBER);
        let title = discord["embeds"][0]["title"].as_str().unwrap();
        assert!(title.contains("Domain Registration Expiring Soon"));
        let fields = discord["embeds"][0]["fields"].as_array().unwrap();
        let days_field = fields.iter().find(|f| f["name"] == "Domain Days Remaining").unwrap();
        assert_eq!(days_field["value"], "14 days");

        // Slack Warning Block
        let slack = format_slack_default(&payload);
        let header = slack["blocks"][0]["text"]["text"].as_str().unwrap();
        assert!(header.contains(":warning:"));
        assert!(header.contains("Domain Registration Expiring Soon"));

        // Generic Webhook
        let generic = format_generic_default(&payload);
        assert_eq!(generic["event"], "domain.expiring_soon");
        assert_eq!(generic["status"], "DOMAIN_EXPIRING_SOON");
        assert_eq!(generic["domain_days_remaining"], 14);
        assert_eq!(generic["domain_expires_at"], "2026-11-15T00:00:00Z");

        // Custom Template
        let tpl = "Warning: {{monitor_name}} domain expires in {{domain_days_remaining}} days on {{domain_expires_at}}!";
        let rendered = render_custom_template(tpl, &payload);
        assert_eq!(rendered, "Warning: API Service domain expires in 14 days on 2026-11-15T00:00:00Z!");
    }

    #[test]
    fn test_format_discord_default_colors() {
        let up = sample_payload(true);
        let discord_up = format_discord_default(&up);
        assert_eq!(discord_up["embeds"][0]["color"], COLOR_STATUS_GREEN);
        assert_eq!(discord_up["embeds"][0]["title"], "[RESOLVED] API Service is back online");
        let fields = discord_up["embeds"][0]["fields"].as_array().unwrap();
        assert_eq!(fields[0]["name"], "Downtime Duration");
        assert_eq!(fields[0]["value"], "4m 12s");
        assert_eq!(fields[1]["name"], "Current Latency");
        assert_eq!(fields[1]["value"], "45 ms");
        assert_eq!(fields[2]["name"], "HTTP Status");
        assert_eq!(fields[2]["value"], "200 OK");
        assert_eq!(fields[3]["name"], "Resolved At");

        let down = sample_payload(false);
        let discord_down = format_discord_default(&down);
        assert_eq!(discord_down["embeds"][0]["color"], COLOR_STATUS_RED);
    }

    #[test]
    fn test_format_slack_default_icons() {
        let up = sample_payload(true);
        let slack_up = format_slack_default(&up);
        let up_header = slack_up["blocks"][0]["text"]["text"].as_str().unwrap();
        assert!(up_header.contains("[RESOLVED] API Service"));
        let section_text = slack_up["blocks"][1]["text"]["text"].as_str().unwrap();
        assert_eq!(section_text, "Service is operational. Total downtime: 4m 12s.");

        let down = sample_payload(false);
        let slack_down = format_slack_default(&down);
        let down_header = slack_down["blocks"][0]["text"]["text"].as_str().unwrap();
        assert!(down_header.contains(":rotating_light:"));
    }

    #[test]
    fn test_format_generic_default_payload() {
        let down = sample_payload(false);
        let generic_down = format_generic_default(&down);
        assert_eq!(generic_down["event"], "status_change");
        assert_eq!(generic_down["monitor_name"], "API Service");
        assert_eq!(generic_down["status"], "DOWN");

        let mut test = sample_payload(true);
        test.monitor_id = Some("test-monitor-001".to_string());
        test.old_status = "UP".to_string();
        let generic_test = format_generic_default(&test);
        assert_eq!(generic_test["event"], "test_notification");

        let up = sample_payload(true);
        let generic_up = format_generic_default(&up);
        assert_eq!(generic_up["event"], "monitor.recovered");
        assert_eq!(generic_up["monitor_name"], "API Service");
        assert_eq!(generic_up["target_url"], "https://api.example.com/health");
        assert_eq!(generic_up["status"], "UP");
        assert_eq!(generic_up["downtime_duration_secs"], 252);
        assert_eq!(generic_up["response_time_ms"], 45);
        assert_eq!(generic_up["timestamp"], "2026-10-30T18:30:00Z");
    }

    #[test]
    fn test_render_custom_template() {
        let up = sample_payload(true);
        let template = "Alert: {{monitor_name}} is {{status}} (HTTP {{http_status}} in {{response_time_ms}}ms, down for {{downtime_duration}} / {{downtime_duration_secs}}s)";
        let rendered = render_custom_template(template, &up);
        assert_eq!(rendered, "Alert: API Service is UP (HTTP 200 in 45ms, down for 4m 12s / 252s)");
    }
}
