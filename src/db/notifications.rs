use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::error::AppError;

fn default_true() -> bool {
    true
}

/// Relational data model for a unified notification channel.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct NotificationChannel {
    pub id: String,
    pub name: String,
    #[sqlx(rename = "type")]
    #[serde(rename = "type")]
    pub channel_type: String,
    pub url: String,
    pub is_default: bool,
    #[serde(default = "default_true")]
    pub is_enabled: bool,
    #[serde(default = "default_true")]
    pub use_default_template: bool,
    #[serde(default = "default_true")]
    pub notify_on_resolve: bool,
    #[serde(default)]
    pub custom_template: Option<String>,
    pub created_at: String,
}

impl NotificationChannel {
    /// Returns a cloned view with secret token/path/query segments masked for API responses (`SEC-04`).
    pub fn masked_view(&self) -> Self {
        let mut copy = self.clone();
        copy.url = mask_webhook_url(&self.url);
        copy
    }
}

/// Returns `true` if `url` still contains the `****` redaction marker from `masked_view()`.
pub fn is_masked_url(url: &str) -> bool {
    url.contains("****")
}

/// Redacts sensitive webhook tokens, path suffixes, and query strings while preserving scheme + host.
pub fn mask_webhook_url(raw: &str) -> String {
    let trimmed = raw.trim();
    let Some((scheme, rest)) = trimmed.split_once("://") else {
        return "****".to_string();
    };

    // Strip any userinfo@ before host
    let rest_no_auth = match rest.split_once('@') {
        Some((_, after_at)) => after_at,
        None => rest,
    };

    let (host, tail) = match rest_no_auth.find(['/', '?', '#']) {
        Some(idx) => (&rest_no_auth[..idx], &rest_no_auth[idx..]),
        None => (rest_no_auth, ""),
    };

    let path_only = tail.split(['?', '#']).next().unwrap_or("");
    let segments: Vec<&str> = path_only
        .trim_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();

    if segments.len() >= 4 && segments[0] == "api" && segments[1] == "webhooks" {
        // Discord: https://discord.com/api/webhooks/<id>/****
        format!("{scheme}://{host}/api/webhooks/{}/****", segments[2])
    } else if segments.len() >= 3 && segments[0] == "services" {
        // Slack: https://hooks.slack.com/services/<workspace>/****
        format!("{scheme}://{host}/services/{}/****", segments[1])
    } else if !segments.is_empty() && segments[0].starts_with("bot") {
        // Telegram: https://api.telegram.org/bot****/sendMessage
        let action = segments.get(1).copied().unwrap_or("sendMessage");
        format!("{scheme}://{host}/bot****/{action}")
    } else if !segments.is_empty() || tail.contains('?') {
        format!("{scheme}://{host}/****")
    } else {
        format!("{scheme}://{host}")
    }
}

const SELECT_CHANNEL_COLS: &str = "\
    id, name, type, url, \
    (is_default != 0) AS is_default, \
    (is_enabled != 0) AS is_enabled, \
    (use_default_template != 0) AS use_default_template, \
    (notify_on_resolve != 0) AS notify_on_resolve, \
    custom_template, created_at";

/// Retrieve all notification channels ordered by creation date (newest first).
pub async fn list_channels(pool: &SqlitePool) -> Result<Vec<NotificationChannel>, sqlx::Error> {
    let sql = format!(
        "SELECT {SELECT_CHANNEL_COLS} FROM notification_channels ORDER BY created_at DESC"
    );
    sqlx::query_as::<_, NotificationChannel>(&sql)
        .fetch_all(pool)
        .await
}

/// Retrieve a single notification channel by ID.
pub async fn get_channel(
    pool: &SqlitePool,
    id: &str,
) -> Result<Option<NotificationChannel>, sqlx::Error> {
    let sql = format!("SELECT {SELECT_CHANNEL_COLS} FROM notification_channels WHERE id = ?");
    sqlx::query_as::<_, NotificationChannel>(&sql)
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
        "INSERT INTO notification_channels \
         (id, name, type, url, is_default, is_enabled, use_default_template, notify_on_resolve, custom_template, created_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&channel.id)
    .bind(&channel.name)
    .bind(&channel.channel_type)
    .bind(&channel.url)
    .bind(channel.is_default as i32)
    .bind(channel.is_enabled as i32)
    .bind(channel.use_default_template as i32)
    .bind(channel.notify_on_resolve as i32)
    .bind(&channel.custom_template)
    .bind(&channel.created_at)
    .execute(pool)
    .await?;

    Ok(channel)
}

/// Update an existing notification channel.
#[allow(clippy::too_many_arguments)]
pub async fn update_channel(
    pool: &SqlitePool,
    id: &str,
    name: &str,
    channel_type: &str,
    url: &str,
    is_default: bool,
    is_enabled: bool,
    use_default_template: bool,
    notify_on_resolve: bool,
    custom_template: Option<&str>,
) -> Result<NotificationChannel, AppError> {
    let result = sqlx::query(
        "UPDATE notification_channels \
         SET name = ?, type = ?, url = ?, is_default = ?, is_enabled = ?, \
             use_default_template = ?, notify_on_resolve = ?, custom_template = ? \
         WHERE id = ?",
    )
    .bind(name)
    .bind(channel_type)
    .bind(url)
    .bind(is_default as i32)
    .bind(is_enabled as i32)
    .bind(use_default_template as i32)
    .bind(notify_on_resolve as i32)
    .bind(custom_template)
    .bind(id)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!(
            "Notification channel {id} not found"
        )));
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
        "SELECT c.id, c.name, c.type, c.url, \
                (c.is_default != 0) AS is_default, \
                (c.is_enabled != 0) AS is_enabled, \
                (c.use_default_template != 0) AS use_default_template, \
                (c.notify_on_resolve != 0) AS notify_on_resolve, \
                c.custom_template, c.created_at \
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

/// Overwrite channel associations for a monitor within a transaction, enforcing foreign keys.
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

    let mut seen = HashSet::new();
    for channel_id in channel_ids {
        if seen.insert(channel_id.as_str()) {
            sqlx::query(
                "INSERT INTO monitor_notification_channels (monitor_id, channel_id) \
                 VALUES (?, ?)",
            )
            .bind(monitor_id)
            .bind(channel_id)
            .execute(&mut *tx)
            .await?;
        }
    }

    tx.commit().await?;
    Ok(())
}

/// Get effective enabled channels for a monitor: all explicitly associated channels
/// union all default channels (`is_default = 1`).
pub async fn get_effective_channels_for_monitor(
    pool: &SqlitePool,
    monitor_id: &str,
) -> Result<Vec<NotificationChannel>, sqlx::Error> {
    sqlx::query_as::<_, NotificationChannel>(
        "SELECT DISTINCT c.id, c.name, c.type, c.url, \
                (c.is_default != 0) AS is_default, \
                (c.is_enabled != 0) AS is_enabled, \
                (c.use_default_template != 0) AS use_default_template, \
                (c.notify_on_resolve != 0) AS notify_on_resolve, \
                c.custom_template, c.created_at \
         FROM notification_channels c \
         LEFT JOIN monitor_notification_channels mnc ON c.id = mnc.channel_id AND mnc.monitor_id = ? \
         WHERE c.is_enabled != 0 AND (mnc.monitor_id IS NOT NULL OR c.is_default = 1) \
         ORDER BY c.name ASC",
    )
    .bind(monitor_id)
    .fetch_all(pool)
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_v2_migration_and_pool_foreign_keys_and_permissions() {
        let tmp_path = std::env::temp_dir().join(format!(
            "keepup-task2-{}.db",
            uuid::Uuid::new_v4()
        ));
        let tmp_str = tmp_path.to_str().unwrap();

        let pool = crate::db::init_pool(tmp_str).await.unwrap();
        crate::db::run_migrations(&pool).await.unwrap();

        // Acquire two distinct connections simultaneously to prove pool-wide foreign_keys = 1
        let mut conn1 = pool.acquire().await.unwrap();
        let mut conn2 = pool.acquire().await.unwrap();
        let fk1: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
            .fetch_one(&mut *conn1)
            .await
            .unwrap();
        let fk2: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
            .fetch_one(&mut *conn2)
            .await
            .unwrap();
        assert_eq!(fk1, 1);
        assert_eq!(fk2, 1);
        drop(conn1);
        drop(conn2);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&tmp_path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "SQLite database file must have 0600 permissions");
        }

        // Legacy webhooks tables dropped, new incidents tables created
        let legacy_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('webhooks', 'monitor_webhooks')",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(legacy_count, 0);

        let incident_tables: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('incidents', 'incident_updates')",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(incident_tables, 2);

        // Verify new monitors columns exist
        sqlx::query(
            "SELECT last_ssl_alert_days, ssl_trusted, http_method, expected_status, \
             request_headers, body_keyword, json_pointer, json_expected, degraded_latency_ms, \
             push_token, grace_secs, dns_record_type, dns_expected, resend_interval_mins, \
             last_alerted_at FROM monitors LIMIT 1",
        )
        .fetch_optional(&pool)
        .await
        .unwrap();

        pool.close().await;
        let _ = std::fs::remove_file(&tmp_path);
        let _ = std::fs::remove_file(format!("{tmp_str}-wal"));
        let _ = std::fs::remove_file(format!("{tmp_str}-shm"));
    }

    #[tokio::test]
    async fn test_unified_channel_fields_masking_and_fk_enforcement() {
        let tmp_path = std::env::temp_dir().join(format!(
            "keepup-task2-chan-{}.db",
            uuid::Uuid::new_v4()
        ));
        let tmp_str = tmp_path.to_str().unwrap();
        let pool = crate::db::init_pool(tmp_str).await.unwrap();
        crate::db::run_migrations(&pool).await.unwrap();

        let chan = NotificationChannel {
            id: "chan-1".to_string(),
            name: "Ops Discord".to_string(),
            channel_type: "discord".to_string(),
            url: "https://discord.com/api/webhooks/123456789/super-secret-token-xyz".to_string(),
            is_default: true,
            is_enabled: true,
            use_default_template: false,
            notify_on_resolve: false,
            custom_template: Some("{\"content\":\"{{monitor_name}} {{status}}\"}".to_string()),
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        create_channel(&pool, chan.clone()).await.unwrap();
        let loaded = get_channel(&pool, "chan-1").await.unwrap().unwrap();
        assert!(loaded.is_enabled);
        assert!(!loaded.use_default_template);
        assert!(!loaded.notify_on_resolve);
        assert_eq!(
            loaded.custom_template.as_deref(),
            Some("{\"content\":\"{{monitor_name}} {{status}}\"}")
        );

        // Verify masked_view redacts secrets across provider URL formats
        let masked_discord = loaded.masked_view();
        assert!(!masked_discord.url.contains("super-secret-token-xyz"));
        assert!(masked_discord.url.contains("****"));

        let mut tg = loaded.clone();
        tg.channel_type = "telegram".to_string();
        tg.url = "https://api.telegram.org/bot998877:AAHsecretBotToken123/sendMessage?chat_id=42".to_string();
        assert!(!tg.masked_view().url.contains("AAHsecretBotToken123"));

        let mut generic = loaded.clone();
        generic.channel_type = "webhook".to_string();
        generic.url = "https://hooks.example.com/ingest/secretpath?token=topsecret".to_string();
        let masked_gen = generic.masked_view();
        assert!(!masked_gen.url.contains("topsecret"));
        assert!(!masked_gen.url.contains("secretpath"));

        // Seed monitor and verify FK enforcement on set_monitor_channels
        sqlx::query(
            "INSERT INTO monitors (id, name, url) VALUES ('mon-1', 'Test', 'https://example.com')",
        )
        .execute(&pool)
        .await
        .unwrap();

        let fk_err = set_monitor_channels(&pool, "mon-1", &["non-existent-channel".to_string()]).await;
        assert!(fk_err.is_err(), "Foreign key violation must return Err");

        pool.close().await;
        let _ = std::fs::remove_file(&tmp_path);
        let _ = std::fs::remove_file(format!("{tmp_str}-wal"));
        let _ = std::fs::remove_file(format!("{tmp_str}-shm"));
    }
}
