use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::api::dto::{CreateWebhookDto, UpdateWebhookDto};
use crate::error::AppError;

/// Full webhook notification channel database model.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Webhook {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub url: String,
    pub is_enabled: bool,
    pub use_default_template: bool,
    pub notify_on_resolve: bool,
    pub message_template: Option<String>,
    pub created_at: String,
}

/// Fetch a single webhook record by its ID.
pub async fn get_webhook_by_id(pool: &SqlitePool, id: &str) -> Result<Webhook, AppError> {
    sqlx::query_as::<_, Webhook>(
        "SELECT id, name, provider, url, is_enabled, use_default_template, notify_on_resolve, message_template, created_at \
         FROM webhooks WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("Webhook {id} not found")))
}

/// List all registered webhook channels.
pub async fn list_webhooks(pool: &SqlitePool) -> Result<Vec<Webhook>, AppError> {
    sqlx::query_as::<_, Webhook>(
        "SELECT id, name, provider, url, is_enabled, use_default_template, notify_on_resolve, message_template, created_at \
         FROM webhooks ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

/// Insert a new webhook channel record.
pub async fn create_webhook(
    pool: &SqlitePool,
    payload: CreateWebhookDto,
) -> Result<Webhook, AppError> {
    let id = uuid::Uuid::new_v4().to_string();
    let is_enabled = if payload.is_enabled { 1 } else { 0 };
    let use_default_template = if payload.use_default_template { 1 } else { 0 };
    let notify_on_resolve = if payload.notify_on_resolve { 1 } else { 0 };

    sqlx::query(
        "INSERT INTO webhooks (id, name, provider, url, is_enabled, use_default_template, notify_on_resolve, message_template) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&payload.name)
    .bind(&payload.provider.to_lowercase())
    .bind(&payload.url)
    .bind(is_enabled)
    .bind(use_default_template)
    .bind(notify_on_resolve)
    .bind(&payload.message_template)
    .execute(pool)
    .await?;

    get_webhook_by_id(pool, &id).await
}

/// Update an existing webhook record.
pub async fn update_webhook(
    pool: &SqlitePool,
    id: &str,
    payload: UpdateWebhookDto,
) -> Result<Webhook, AppError> {
    let existing = get_webhook_by_id(pool, id).await?;

    let name = payload.name.unwrap_or(existing.name);
    let provider = payload
        .provider
        .map(|p| p.to_lowercase())
        .unwrap_or(existing.provider);
    let url = payload.url.unwrap_or(existing.url);
    let is_enabled = if payload.is_enabled.unwrap_or(existing.is_enabled) { 1 } else { 0 };
    let use_default_template = if payload.use_default_template.unwrap_or(existing.use_default_template) { 1 } else { 0 };
    let notify_on_resolve = if payload.notify_on_resolve.unwrap_or(existing.notify_on_resolve) { 1 } else { 0 };
    let message_template = match payload.message_template {
        Some(t) => Some(t),
        None => existing.message_template,
    };

    sqlx::query(
        "UPDATE webhooks \
         SET name = ?, provider = ?, url = ?, is_enabled = ?, use_default_template = ?, notify_on_resolve = ?, message_template = ? \
         WHERE id = ?",
    )
    .bind(&name)
    .bind(&provider)
    .bind(&url)
    .bind(is_enabled)
    .bind(use_default_template)
    .bind(notify_on_resolve)
    .bind(&message_template)
    .bind(id)
    .execute(pool)
    .await?;

    get_webhook_by_id(pool, id).await
}

/// Delete a webhook record by its ID.
pub async fn delete_webhook(pool: &SqlitePool, id: &str) -> Result<(), AppError> {
    let result = sqlx::query("DELETE FROM webhooks WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("Webhook {id} not found")));
    }

    Ok(())
}
