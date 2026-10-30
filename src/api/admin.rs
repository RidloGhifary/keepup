use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use tracing::info;

use crate::api::dto::UpdateMonitorDto;
use crate::db::monitors::{self, Monitor};
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

// ── Webhook Admin Handlers (delegated to crate::api::webhooks) ───────────────

#[allow(unused_imports)]
pub use crate::api::webhooks::{
    create_webhook, delete_webhook, list_webhooks, test_webhook, test_webhook_direct,
    update_webhook,
};
