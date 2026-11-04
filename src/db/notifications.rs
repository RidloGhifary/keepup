use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::error::AppError;

/// Relational data model for a notification channel.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct NotificationChannel {
    pub id: String,
    pub name: String,
    #[sqlx(rename = "type")]
    #[serde(rename = "type")]
    pub channel_type: String,
    pub url: String,
    pub is_default: bool,
    pub created_at: String,
    pub bot_name: Option<String>,
    pub avatar_url: Option<String>,
    pub telegram_bot_token: Option<String>,
    pub telegram_chat_id: Option<String>,
    pub telegram_thread_id: Option<String>,
    pub last_error: Option<String>,
    pub last_error_at: Option<String>,
}

/// Relational data model for an alert notification failure log entry.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[allow(dead_code)]
pub struct NotificationFailureLog {
    pub id: String,
    pub channel_id: String,
    pub channel_name: String,
    pub channel_type: String,
    pub error_message: String,
    pub status_code: Option<i64>,
    pub attempt_count: i64,
    pub created_at: String,
}

/// Retrieve all notification channels ordered by creation date (newest first).
pub async fn list_channels(pool: &SqlitePool) -> Result<Vec<NotificationChannel>, sqlx::Error> {
    sqlx::query_as::<_, NotificationChannel>(
        "SELECT id, name, type, url, (is_default != 0) AS is_default, created_at, \
         bot_name, avatar_url, telegram_bot_token, telegram_chat_id, telegram_thread_id, \
         last_error, last_error_at \
         FROM notification_channels \
         ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await
}

/// Retrieve a single notification channel by ID.
pub async fn get_channel(
    pool: &SqlitePool,
    id: &str,
) -> Result<Option<NotificationChannel>, sqlx::Error> {
    sqlx::query_as::<_, NotificationChannel>(
        "SELECT id, name, type, url, (is_default != 0) AS is_default, created_at, \
         bot_name, avatar_url, telegram_bot_token, telegram_chat_id, telegram_thread_id, \
         last_error, last_error_at \
         FROM notification_channels \
         WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// Insert a new notification channel into the database.
pub async fn create_channel(
    pool: &SqlitePool,
    channel: NotificationChannel,
) -> Result<NotificationChannel, sqlx::Error> {
    sqlx::query(
        "INSERT INTO notification_channels (\
         id, name, type, url, is_default, created_at, \
         bot_name, avatar_url, telegram_bot_token, telegram_chat_id, telegram_thread_id\
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&channel.id)
    .bind(&channel.name)
    .bind(&channel.channel_type)
    .bind(&channel.url)
    .bind(channel.is_default as i32)
    .bind(&channel.created_at)
    .bind(&channel.bot_name)
    .bind(&channel.avatar_url)
    .bind(&channel.telegram_bot_token)
    .bind(&channel.telegram_chat_id)
    .bind(&channel.telegram_thread_id)
    .execute(pool)
    .await?;

    Ok(channel)
}

/// Update an existing notification channel.
pub async fn update_channel(
    pool: &SqlitePool,
    id: &str,
    name: &str,
    channel_type: &str,
    url: &str,
    is_default: bool,
    bot_name: Option<&str>,
    avatar_url: Option<&str>,
    telegram_bot_token: Option<&str>,
    telegram_chat_id: Option<&str>,
    telegram_thread_id: Option<&str>,
) -> Result<NotificationChannel, AppError> {
    let result = sqlx::query(
        "UPDATE notification_channels \
         SET name = ?, type = ?, url = ?, is_default = ?, bot_name = ?, avatar_url = ?, \
             telegram_bot_token = ?, telegram_chat_id = ?, telegram_thread_id = ?, \
             last_error = NULL, last_error_at = NULL \
         WHERE id = ?",
    )
    .bind(name)
    .bind(channel_type)
    .bind(url)
    .bind(is_default as i32)
    .bind(bot_name)
    .bind(avatar_url)
    .bind(telegram_bot_token)
    .bind(telegram_chat_id)
    .bind(telegram_thread_id)
    .bind(id)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("Notification channel {id} not found")));
    }

    get_channel(pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Notification channel {id} not found")))
}

/// Delete a notification channel by ID.
/// Monitor associations are automatically removed via ON DELETE CASCADE.
pub async fn delete_channel(pool: &SqlitePool, id: &str) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM notification_channels WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}

/// Get all channels explicitly bound to a monitor.
#[allow(dead_code)]
pub async fn get_channels_for_monitor(
    pool: &SqlitePool,
    monitor_id: &str,
) -> Result<Vec<NotificationChannel>, sqlx::Error> {
    sqlx::query_as::<_, NotificationChannel>(
        "SELECT c.id, c.name, c.type, c.url, (c.is_default != 0) AS is_default, c.created_at, \
         c.bot_name, c.avatar_url, c.telegram_bot_token, c.telegram_chat_id, c.telegram_thread_id, \
         c.last_error, c.last_error_at \
         FROM notification_channels c \
         INNER JOIN monitor_notification_channels mnc ON c.id = mnc.channel_id \
         WHERE mnc.monitor_id = ? \
         ORDER BY c.name ASC",
    )
    .bind(monitor_id)
    .fetch_all(pool)
    .await
}

/// Get IDs of channels explicitly attached to a monitor.
pub async fn get_channel_ids_for_monitor(
    pool: &SqlitePool,
    monitor_id: &str,
) -> Result<Vec<String>, sqlx::Error> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT channel_id FROM monitor_notification_channels WHERE monitor_id = ?",
    )
    .bind(monitor_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|(id,)| id).collect())
}

/// Overwrite channel associations for a monitor.
pub async fn set_monitor_channels(
    pool: &SqlitePool,
    monitor_id: &str,
    channel_ids: &[String],
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    sqlx::query("DELETE FROM monitor_notification_channels WHERE monitor_id = ?")
        .bind(monitor_id)
        .execute(&mut *tx)
        .await?;

    for channel_id in channel_ids {
        sqlx::query(
            "INSERT OR IGNORE INTO monitor_notification_channels (monitor_id, channel_id) \
             VALUES (?, ?)",
        )
        .bind(monitor_id)
        .bind(channel_id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

/// Get effective channels for a monitor: all explicitly associated channels
/// union all default channels (`is_default = 1`).
pub async fn get_effective_channels_for_monitor(
    pool: &SqlitePool,
    monitor_id: &str,
) -> Result<Vec<NotificationChannel>, sqlx::Error> {
    sqlx::query_as::<_, NotificationChannel>(
        "SELECT DISTINCT c.id, c.name, c.type, c.url, (c.is_default != 0) AS is_default, c.created_at, \
         c.bot_name, c.avatar_url, c.telegram_bot_token, c.telegram_chat_id, c.telegram_thread_id, \
         c.last_error, c.last_error_at \
         FROM notification_channels c \
         LEFT JOIN monitor_notification_channels mnc ON c.id = mnc.channel_id AND mnc.monitor_id = ? \
         WHERE mnc.monitor_id IS NOT NULL OR c.is_default = 1 \
         ORDER BY c.name ASC",
    )
    .bind(monitor_id)
    .fetch_all(pool)
    .await
}

/// Record an alert notification delivery failure log entry and set warning state on the channel.
pub async fn record_channel_failure(
    pool: &SqlitePool,
    channel_id: &str,
    channel_name: &str,
    channel_type: &str,
    error_message: &str,
    status_code: Option<u16>,
    attempt_count: u32,
) -> Result<(), sqlx::Error> {
    let log_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let sc = status_code.map(|s| s as i64);

    sqlx::query(
        "INSERT INTO notification_failure_logs (\
         id, channel_id, channel_name, channel_type, error_message, status_code, attempt_count, created_at\
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&log_id)
    .bind(channel_id)
    .bind(channel_name)
    .bind(channel_type)
    .bind(error_message)
    .bind(sc)
    .bind(attempt_count as i64)
    .bind(&now)
    .execute(pool)
    .await?;

    sqlx::query(
        "UPDATE notification_channels \
         SET last_error = ?, last_error_at = ? \
         WHERE id = ?",
    )
    .bind(error_message)
    .bind(&now)
    .bind(channel_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// Clear any warning state on the channel after a successful delivery.
pub async fn record_channel_success(
    pool: &SqlitePool,
    channel_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE notification_channels \
         SET last_error = NULL, last_error_at = NULL \
         WHERE id = ?",
    )
    .bind(channel_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// Retrieve recent failure logs for a given channel (or all channels).
#[allow(dead_code)]
pub async fn list_failure_logs(
    pool: &SqlitePool,
    channel_id: Option<&str>,
    limit: i64,
) -> Result<Vec<NotificationFailureLog>, sqlx::Error> {
    if let Some(cid) = channel_id {
        sqlx::query_as::<_, NotificationFailureLog>(
            "SELECT id, channel_id, channel_name, channel_type, error_message, status_code, attempt_count, created_at \
             FROM notification_failure_logs \
             WHERE channel_id = ? \
             ORDER BY created_at DESC LIMIT ?",
        )
        .bind(cid)
        .bind(limit)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query_as::<_, NotificationFailureLog>(
            "SELECT id, channel_id, channel_name, channel_type, error_message, status_code, attempt_count, created_at \
             FROM notification_failure_logs \
             ORDER BY created_at DESC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(pool)
        .await
    }
}
