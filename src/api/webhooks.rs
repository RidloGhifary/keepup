use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde_json::{json, Value};
use tracing::info;

use crate::api::dto::{CreateWebhookRequest, TestWebhookRequest, UpdateWebhookRequest};
use crate::db::webhooks::{self, Webhook};
use crate::error::AppError;
use crate::notifier::{self, AlertPayload};
use crate::AppState;

// ── Webhook Route Handlers ───────────────────────────────────────────────────

/// `GET /api/webhooks`
///
/// Returns a JSON list of all configured notification webhooks.
pub async fn list_webhooks(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<Webhook>>, AppError> {
    let webhooks = webhooks::list_webhooks(&state.db).await?;
    Ok(Json(webhooks))
}

/// `POST /api/webhooks`
///
/// Registers a new alert webhook channel.
/// Returns HTTP 201 Created with the inserted Webhook object.
pub async fn create_webhook(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateWebhookRequest>,
) -> Result<(StatusCode, Json<Webhook>), AppError> {
    payload.validate()?;

    let created = webhooks::create_webhook(&state.db, payload)
        .await
        .map_err(AppError::from)?;
    Ok((StatusCode::CREATED, Json(created)))
}

/// `PUT /api/webhooks/:id`
///
/// Updates an existing alert webhook channel.
pub async fn update_webhook(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateWebhookRequest>,
) -> Result<Json<Webhook>, AppError> {
    payload.validate()?;

    let updated = webhooks::update_webhook(&state.db, &id, payload).await?;
    Ok(Json(updated))
}

/// `DELETE /api/webhooks/:id`
///
/// Removes an alert webhook channel by its ID.
pub async fn delete_webhook(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    webhooks::delete_webhook(&state.db, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /api/webhooks/test`
///
/// Dispatches a sample notification payload using client-provided configuration.
/// Uses a 5-second timeout to prevent request hanging.
pub async fn test_webhook_direct(
    Json(payload): Json<TestWebhookRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if let Err(err) = payload.validate() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": err.to_string() })),
        ));
    }

    let client = match reqwest::ClientBuilder::new()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": format!("Failed to create HTTP client: {e}") })),
            ));
        }
    };

    let test_payload = AlertPayload {
        monitor_id: Some("test-monitor-001".to_string()),
        monitor_name: "KeepUp Test Target".to_string(),
        monitor_url: "https://keepup.local".to_string(),
        new_status: "UP".to_string(),
        old_status: "DOWN".to_string(),
        http_status: Some(200),
        response_time_ms: Some(24),
        error_msg: None,
        downtime_duration: Some("0 seconds (Manual Test Alert)".to_string()),
        downtime_duration_secs: Some(0),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    let use_default = payload.use_default_template;
    let template = payload.message_template.as_deref();

    let dispatch_res = match payload.provider.to_lowercase().as_str() {
        "discord" => {
            notifier::discord::send(&client, &payload.url, &test_payload, use_default, template)
                .await
        }
        "slack" => {
            notifier::slack::send(&client, &payload.url, &test_payload, use_default, template)
                .await
        }
        "generic" | "generic_webhook" => {
            notifier::generic::send(&client, &payload.url, &test_payload, use_default, template)
                .await
        }
        other => Err(anyhow::anyhow!("Unsupported webhook provider '{other}'")),
    };

    match dispatch_res {
        Ok(()) => Ok(Json(json!({
            "success": true,
            "message": "Test notification delivered successfully."
        }))),
        Err(err) => Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": format!("Failed to send test payload: {err}")
            })),
        )),
    }
}

/// `POST /api/webhooks/:id/test`
///
/// Dispatches a sample notification payload to test an existing saved webhook.
pub async fn test_webhook(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let webhook = match webhooks::get_webhook_by_id(&state.db, &id).await {
        Ok(w) => w,
        Err(err) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({ "error": err.to_string() })),
            ));
        }
    };

    let client = match reqwest::ClientBuilder::new()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": format!("Failed to create HTTP client: {e}") })),
            ));
        }
    };

    let test_payload = AlertPayload {
        monitor_id: Some("test-monitor-001".to_string()),
        monitor_name: "KeepUp Test Target".to_string(),
        monitor_url: "https://keepup.local".to_string(),
        new_status: "UP".to_string(),
        old_status: "DOWN".to_string(),
        http_status: Some(200),
        response_time_ms: Some(24),
        error_msg: None,
        downtime_duration: Some("0 seconds (Manual Test Alert)".to_string()),
        downtime_duration_secs: Some(0),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    let use_default = webhook.use_default_template;
    let template = webhook.message_template.as_deref();

    let dispatch_res = match webhook.provider.to_lowercase().as_str() {
        "discord" => {
            notifier::discord::send(&client, &webhook.url, &test_payload, use_default, template)
                .await
        }
        "slack" => {
            notifier::slack::send(&client, &webhook.url, &test_payload, use_default, template)
                .await
        }
        "generic" | "generic_webhook" => {
            notifier::generic::send(&client, &webhook.url, &test_payload, use_default, template)
                .await
        }
        other => Err(anyhow::anyhow!("Unsupported webhook provider '{other}'")),
    };

    match dispatch_res {
        Ok(()) => {
            info!(
                webhook_id = %webhook.id,
                name = %webhook.name,
                provider = %webhook.provider,
                "Test webhook successfully delivered"
            );
            Ok(Json(json!({
                "success": true,
                "message": "Test notification delivered successfully.",
                "webhook_id": webhook.id,
            })))
        }
        Err(err) => Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": format!("Failed to send test payload: {err}")
            })),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_webhook_request_validation() {
        let req = CreateWebhookRequest {
            name: "Slack Ops".to_string(),
            provider: "slack".to_string(),
            url: "https://hooks.slack.com/services/xxx".to_string(),
            is_enabled: true,
            use_default_template: true,
            notify_on_resolve: true,
            message_template: None,
        };
        assert!(req.validate().is_ok());

        let invalid_provider = CreateWebhookRequest {
            provider: "unsupported".to_string(),
            ..req.clone()
        };
        assert!(invalid_provider.validate().is_err());

        let invalid_url = CreateWebhookRequest {
            url: "ftp://invalid-url.com".to_string(),
            ..req
        };
        assert!(invalid_url.validate().is_err());
    }

    #[test]
    fn test_test_webhook_request_validation() {
        let req = TestWebhookRequest {
            url: "https://discord.com/api/webhooks/xxx".to_string(),
            provider: "discord".to_string(),
            use_default_template: true,
            message_template: None,
        };
        assert!(req.validate().is_ok());

        let invalid_url = TestWebhookRequest {
            url: "invalid-url".to_string(),
            ..req
        };
        assert!(invalid_url.validate().is_err());
    }
}
