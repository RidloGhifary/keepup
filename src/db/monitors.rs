use chrono::Timelike;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::api::dto::UpdateMonitorDto;
use crate::error::AppError;

/// Full monitor database row model.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Monitor {
    pub id: String,
    pub name: String,
    pub url: String,
    pub check_interval_secs: i32,
    pub timeout_secs: i32,
    pub max_retries: i32,
    pub is_active: i32,
    pub is_public: i32,
    pub last_status: String,
    pub down_since: Option<String>,
    pub created_at: String,
    #[sqlx(default)]
    pub check_domain_expiry: i32,
    #[sqlx(default)]
    pub domain_warning_days: i32,
    pub domain_expires_at: Option<String>,
    pub domain_days_remaining: Option<i32>,
    pub last_domain_alert_days: Option<i32>,
    pub domain_last_checked_at: Option<String>,
    #[sqlx(default)]
    pub check_ssl: i32,
    #[sqlx(default)]
    pub ssl_warning_days: i32,
    pub ssl_expires_at: Option<String>,
    pub ssl_days_remaining: Option<i32>,
    pub ssl_last_checked_at: Option<String>,
    #[sqlx(default)]
    pub last_ssl_alert_days: Option<i32>,
    #[sqlx(default)]
    pub ssl_trusted: Option<i32>,
    #[sqlx(default)]
    pub http_method: String,
    #[sqlx(default)]
    pub expected_status: String,
    #[sqlx(default)]
    pub request_headers: Option<String>,
    #[sqlx(default)]
    pub body_keyword: Option<String>,
    #[sqlx(default)]
    pub json_pointer: Option<String>,
    #[sqlx(default)]
    pub json_expected: Option<String>,
    #[sqlx(default)]
    pub degraded_latency_ms: Option<i64>,
    #[sqlx(default)]
    pub push_token: Option<String>,
    #[sqlx(default)]
    pub grace_secs: i32,
    #[sqlx(default)]
    pub dns_record_type: String,
    #[sqlx(default)]
    pub dns_expected: Option<String>,
    #[sqlx(default)]
    pub resend_interval_mins: i32,
    #[sqlx(default)]
    pub last_alerted_at: Option<String>,
}

/// Fetch a single monitor by its primary ID.
pub async fn get_monitor_by_id(pool: &SqlitePool, id: &str) -> Result<Monitor, AppError> {
    sqlx::query_as::<_, Monitor>("SELECT * FROM monitors WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Monitor {id} not found")))
}

/// Update an existing monitor target with the supplied DTO fields.
pub async fn update_monitor(
    pool: &SqlitePool,
    id: &str,
    payload: UpdateMonitorDto,
) -> Result<Monitor, AppError> {
    let mut tx = pool.begin().await?;

    let existing = sqlx::query_as::<_, Monitor>("SELECT * FROM monitors WHERE id = ?")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Monitor {id} not found")))?;

    let name = payload.name.unwrap_or(existing.name);
    let url = payload.url.unwrap_or(existing.url);
    let check_interval_secs = payload
        .check_interval_secs
        .unwrap_or(existing.check_interval_secs);
    let timeout_secs = payload.timeout_secs.unwrap_or(existing.timeout_secs);
    let max_retries = payload.max_retries.unwrap_or(existing.max_retries);
    let is_active = payload
        .is_active
        .map(|b| if b { 1 } else { 0 })
        .unwrap_or(existing.is_active);
    let is_public = payload
        .is_public
        .map(|b| if b { 1 } else { 0 })
        .unwrap_or(existing.is_public);
    let check_domain_expiry = payload
        .check_domain_expiry
        .map(|b| if b { 1 } else { 0 })
        .unwrap_or(existing.check_domain_expiry);
    let domain_warning_days = payload
        .domain_warning_days
        .unwrap_or(existing.domain_warning_days);
    let check_ssl = payload
        .check_ssl
        .map(|b| if b { 1 } else { 0 })
        .unwrap_or(existing.check_ssl);
    let ssl_warning_days = payload
        .ssl_warning_days
        .unwrap_or(existing.ssl_warning_days);

    let http_method = payload.http_method.unwrap_or(existing.http_method);
    let expected_status = payload.expected_status.unwrap_or(existing.expected_status);
    let request_headers = payload.request_headers.or(existing.request_headers);
    let body_keyword = payload.body_keyword.or(existing.body_keyword);
    let json_pointer = payload.json_pointer.or(existing.json_pointer);
    let json_expected = payload.json_expected.or(existing.json_expected);
    let degraded_latency_ms = payload.degraded_latency_ms.or(existing.degraded_latency_ms);
    let dns_record_type = payload.dns_record_type.unwrap_or(existing.dns_record_type);
    let dns_expected = payload.dns_expected.or(existing.dns_expected);
    let resend_interval_mins = payload.resend_interval_mins.unwrap_or(existing.resend_interval_mins);
    let grace_secs = payload.grace_secs.unwrap_or(existing.grace_secs);

    sqlx::query(
        "UPDATE monitors \
         SET name = ?, url = ?, check_interval_secs = ?, timeout_secs = ?, \
             max_retries = ?, is_active = ?, is_public = ?, \
             check_domain_expiry = ?, domain_warning_days = ?, \
             check_ssl = ?, ssl_warning_days = ?, \
             http_method = ?, expected_status = ?, request_headers = ?, \
             body_keyword = ?, json_pointer = ?, json_expected = ?, \
             degraded_latency_ms = ?, dns_record_type = ?, dns_expected = ?, \
             resend_interval_mins = ?, grace_secs = ? \
         WHERE id = ?",
    )
    .bind(&name)
    .bind(&url)
    .bind(check_interval_secs)
    .bind(timeout_secs)
    .bind(max_retries)
    .bind(is_active)
    .bind(is_public)
    .bind(check_domain_expiry)
    .bind(domain_warning_days)
    .bind(check_ssl)
    .bind(ssl_warning_days)
    .bind(&http_method)
    .bind(&expected_status)
    .bind(&request_headers)
    .bind(&body_keyword)
    .bind(&json_pointer)
    .bind(&json_expected)
    .bind(degraded_latency_ms)
    .bind(&dns_record_type)
    .bind(&dns_expected)
    .bind(resend_interval_mins)
    .bind(grace_secs)
    .bind(id)
    .execute(&mut *tx)
    .await?;

    if let Some(ref channel_ids) = payload.channel_ids {
        sqlx::query("DELETE FROM monitor_notification_channels WHERE monitor_id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;

        let mut seen = std::collections::HashSet::new();
        for cid in channel_ids {
            if seen.insert(cid.as_str()) {
                sqlx::query(
                    "INSERT INTO monitor_notification_channels (monitor_id, channel_id) \
                     VALUES (?, ?)",
                )
                .bind(id)
                .bind(cid)
                .execute(&mut *tx)
                .await?;
            }
        }
    }

    let updated = sqlx::query_as::<_, Monitor>("SELECT * FROM monitors WHERE id = ?")
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;

    tx.commit().await?;

    Ok(updated)
}

/// Update domain expiration telemetry for a monitor.
pub async fn update_domain_expiry(
    pool: &SqlitePool,
    id: &str,
    expires_at: Option<&str>,
    days_remaining: Option<i32>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE monitors \
         SET domain_expires_at = ?, domain_days_remaining = ?, domain_last_checked_at = ? \
         WHERE id = ?",
    )
    .bind(expires_at)
    .bind(days_remaining)
    .bind(chrono::Utc::now().to_rfc3339())
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}

/// Update SSL certificate expiration telemetry for a monitor.
pub async fn update_ssl_expiry(
    pool: &SqlitePool,
    id: &str,
    expires_at: Option<&str>,
    days_remaining: Option<i32>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE monitors \
         SET ssl_expires_at = ?, ssl_days_remaining = ?, ssl_last_checked_at = ? \
         WHERE id = ?",
    )
    .bind(expires_at)
    .bind(days_remaining)
    .bind(chrono::Utc::now().to_rfc3339())
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}

/// Query all active monitors where either check_ssl or check_domain_expiry is enabled.
pub async fn get_asset_expiry_monitors(pool: &SqlitePool) -> Result<Vec<Monitor>, AppError> {
    let rows = sqlx::query_as::<_, Monitor>(
        "SELECT * FROM monitors WHERE is_active = 1 AND (check_ssl = 1 OR check_domain_expiry = 1)",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// Update milestone alert suppression tracker for domain expiration.
pub async fn update_domain_alert_milestone(
    pool: &SqlitePool,
    id: &str,
    milestone_days: Option<i32>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE monitors \
         SET last_domain_alert_days = ? \
         WHERE id = ?",
    )
    .bind(milestone_days)
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}

/// Update milestone alert suppression tracker for SSL expiration.
pub async fn update_ssl_alert_milestone(
    pool: &SqlitePool,
    id: &str,
    milestone_days: Option<i32>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE monitors \
         SET last_ssl_alert_days = ? \
         WHERE id = ?",
    )
    .bind(milestone_days)
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}

/// Update the operational status and down_since timestamp for a monitor.
pub async fn update_monitor_status(
    pool: &SqlitePool,
    id: &str,
    status: &str,
    down_since: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE monitors \
         SET last_status = ?, down_since = ? \
         WHERE id = ?",
    )
    .bind(status)
    .bind(down_since)
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}

/// Daily uptime bucket representation for 7-day timeline.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DailyUptime {
    pub date: String,
    pub uptime_pct: f64,
    pub status: String,
}

#[derive(Debug, sqlx::FromRow)]
#[allow(dead_code)]
struct DailyLogAgg {
    day: String,
    total: i64,
    up_count: i64,
}

/// Aggregates check logs for a monitor over the past 7 days into 7 distinct daily buckets (Day -6 to Today).
#[allow(dead_code)]
pub async fn get_7day_uptime_history(
    pool: &SqlitePool,
    monitor_id: &str,
) -> Result<Vec<DailyUptime>, AppError> {
    // Query check_logs where created_at >= datetime('now', '-7 days')
    // We also fallback to checked_at if created_at is not populated
    let rows = sqlx::query_as::<_, DailyLogAgg>(
        r#"
        SELECT 
            date(COALESCE(created_at, checked_at)) AS day,
            COUNT(*) AS total,
            COALESCE(SUM(CASE WHEN is_up = 1 THEN 1 ELSE 0 END), 0) AS up_count
        FROM check_logs
        WHERE monitor_id = ? 
          AND (
            created_at >= datetime('now', '-7 days')
            OR checked_at >= datetime('now', '-7 days')
          )
        GROUP BY date(COALESCE(created_at, checked_at))
        ORDER BY day ASC
        "#,
    )
    .bind(monitor_id)
    .fetch_all(pool)
    .await?;

    let mut day_map: std::collections::HashMap<String, (i64, i64)> = std::collections::HashMap::new();
    for r in rows {
        day_map.insert(r.day, (r.total, r.up_count));
    }

    // Group check data into 7 distinct daily buckets (Day -6 to Day 0 / Today)
    let today = chrono::Utc::now().naive_utc().date();
    let mut history = Vec::with_capacity(7);

    for i in (0..7).rev() {
        let date_obj = today - chrono::Duration::days(i);
        let date_str = date_obj.format("%Y-%m-%d").to_string();

        let (uptime_pct, status) = match day_map.get(&date_str) {
            Some(&(total, up_count)) if total > 0 => {
                let pct = ((up_count as f64 / total as f64) * 100.0 * 100.0).round() / 100.0;
                let status_str = if pct < 90.0 {
                    "DOWN"
                } else if pct < 99.0 {
                    "DEGRADED"
                } else {
                    "UP"
                };
                (pct, status_str.to_string())
            }
            _ => (100.0, "NO_DATA".to_string()),
        };

        history.push(DailyUptime {
            date: date_str,
            uptime_pct,
            status,
        });
    }

    Ok(history)
}

/// Hourly latency data point for 24-hour latency breakdown.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct HourlyLatency {
    pub timestamp: String,
    pub avg_latency_ms: i64,
    pub status: String,
}

#[derive(Debug, sqlx::FromRow)]
struct HourlyLogAgg {
    hour_slot: String,
    total: i64,
    up_count: i64,
    avg_latency: f64,
}

/// Aggregates check logs for a monitor over the past 24 hours into 24 discrete hourly slots (23h ago to current hour).
pub async fn get_24h_latency_history(
    pool: &SqlitePool,
    monitor_id: &str,
) -> Result<Vec<HourlyLatency>, AppError> {
    let rows = sqlx::query_as::<_, HourlyLogAgg>(
        r#"
        SELECT 
            strftime('%Y-%m-%d %H:00:00', COALESCE(created_at, checked_at)) AS hour_slot,
            COUNT(*) AS total,
            COALESCE(SUM(CASE WHEN is_up = 1 THEN 1 ELSE 0 END), 0) AS up_count,
            COALESCE(AVG(CASE WHEN latency_ms IS NOT NULL THEN latency_ms ELSE NULL END), 0.0) AS avg_latency
        FROM check_logs
        WHERE monitor_id = ? 
          AND (
            created_at >= datetime('now', '-24 hours')
            OR checked_at >= datetime('now', '-24 hours')
          )
        GROUP BY strftime('%Y-%m-%d %H:00:00', COALESCE(created_at, checked_at))
        ORDER BY hour_slot ASC
        "#,
    )
    .bind(monitor_id)
    .fetch_all(pool)
    .await?;

    let mut slot_map: std::collections::HashMap<String, (i64, i64, f64)> = std::collections::HashMap::new();
    for r in rows {
        slot_map.insert(r.hour_slot, (r.total, r.up_count, r.avg_latency));
    }

    let now = chrono::Utc::now();
    let current_hour = now
        .date_naive()
        .and_hms_opt(now.hour(), 0, 0)
        .unwrap_or_else(|| now.naive_utc());

    let mut latency_24h = Vec::with_capacity(24);

    for i in (0..24).rev() {
        let slot_time = current_hour - chrono::Duration::hours(i);
        let sql_key = slot_time.format("%Y-%m-%d %H:00:00").to_string();
        let timestamp = slot_time.format("%Y-%m-%dT%H:00:00Z").to_string();

        let (avg_latency_ms, status) = match slot_map.get(&sql_key) {
            Some(&(total, up_count, avg_lat)) if total > 0 => {
                let pct = (up_count as f64 / total as f64) * 100.0;
                let status_str = if pct < 90.0 {
                    "DOWN"
                } else if pct < 99.0 {
                    "DEGRADED"
                } else {
                    "UP"
                };
                (avg_lat.round() as i64, status_str.to_string())
            }
            _ => (0, "NO_DATA".to_string()),
        };

        latency_24h.push(HourlyLatency {
            timestamp,
            avg_latency_ms,
            status,
        });
    }

    Ok(latency_24h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_daily_uptime_status_thresholds() {
        // >= 99.0 -> UP
        let up_pct = 99.5;
        let status = if up_pct < 90.0 {
            "DOWN"
        } else if up_pct < 99.0 {
            "DEGRADED"
        } else {
            "UP"
        };
        assert_eq!(status, "UP");

        // 90.0..99.0 -> DEGRADED
        let deg_pct = 95.0;
        let status = if deg_pct < 90.0 {
            "DOWN"
        } else if deg_pct < 99.0 {
            "DEGRADED"
        } else {
            "UP"
        };
        assert_eq!(status, "DEGRADED");

        // < 90.0 -> DOWN
        let down_pct = 85.0;
        let status = if down_pct < 90.0 {
            "DOWN"
        } else if down_pct < 99.0 {
            "DEGRADED"
        } else {
            "UP"
        };
        assert_eq!(status, "DOWN");
    }

    #[test]
    fn test_7day_bucket_dates_order_and_count() {
        let today = chrono::Utc::now().naive_utc().date();
        let mut days = Vec::with_capacity(7);
        for i in (0..7).rev() {
            let date_obj = today - chrono::Duration::days(i);
            days.push(date_obj.format("%Y-%m-%d").to_string());
        }
        assert_eq!(days.len(), 7);
        assert_eq!(days[6], today.format("%Y-%m-%d").to_string());
        let expected_first = (today - chrono::Duration::days(6)).format("%Y-%m-%d").to_string();
        assert_eq!(days[0], expected_first);
    }

    #[tokio::test]
    async fn test_get_7day_uptime_history_db() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS monitors (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                url TEXT NOT NULL,
                check_interval_secs INTEGER NOT NULL DEFAULT 60,
                timeout_secs INTEGER NOT NULL DEFAULT 5,
                max_retries INTEGER NOT NULL DEFAULT 3,
                is_active INTEGER NOT NULL DEFAULT 1,
                is_public INTEGER NOT NULL DEFAULT 1,
                last_status TEXT NOT NULL DEFAULT 'UP',
                down_since DATETIME,
                created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE IF NOT EXISTS check_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                monitor_id TEXT NOT NULL,
                status_code INTEGER,
                latency_ms INTEGER,
                is_up INTEGER NOT NULL,
                error_msg TEXT,
                checked_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
            );"
        )
        .execute(&pool)
        .await
        .unwrap();

        let monitor_id = "test-mon-7day";
        sqlx::query(
            "INSERT INTO monitors (id, name, url, is_active, is_public) VALUES (?, ?, ?, 1, 1)"
        )
        .bind(monitor_id)
        .bind("Test Monitor")
        .bind("https://example.com")
        .execute(&pool)
        .await
        .unwrap();

        // Today: 2 checks, both UP
        sqlx::query(
            "INSERT INTO check_logs (monitor_id, is_up, checked_at, created_at) VALUES (?, 1, datetime('now'), datetime('now'))"
        )
        .bind(monitor_id)
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO check_logs (monitor_id, is_up, checked_at, created_at) VALUES (?, 1, datetime('now'), datetime('now'))"
        )
        .bind(monitor_id)
        .execute(&pool)
        .await
        .unwrap();

        // 1 day ago: 10 checks, 8 up (80% -> DOWN)
        for i in 0..10 {
            let is_up = if i < 8 { 1 } else { 0 };
            sqlx::query(
                "INSERT INTO check_logs (monitor_id, is_up, checked_at, created_at) VALUES (?, ?, datetime('now', '-1 day'), datetime('now', '-1 day'))"
            )
            .bind(monitor_id)
            .bind(is_up)
            .execute(&pool)
            .await
            .unwrap();
        }

        let history = get_7day_uptime_history(&pool, monitor_id).await.unwrap();
        assert_eq!(history.len(), 7);

        // Day -1 (index 5) should be DOWN (80.0%)
        assert_eq!(history[5].status, "DOWN");
        assert_eq!(history[5].uptime_pct, 80.0);

        // Day 0 (index 6, today) should be UP (100.0%)
        assert_eq!(history[6].status, "UP");
        assert_eq!(history[6].uptime_pct, 100.0);

        // Older days with no logs should be NO_DATA
        assert_eq!(history[0].status, "NO_DATA");
    }

    #[tokio::test]
    async fn test_get_24h_latency_history_db() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS monitors (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                url TEXT NOT NULL,
                check_interval_secs INTEGER NOT NULL DEFAULT 60,
                timeout_secs INTEGER NOT NULL DEFAULT 5,
                max_retries INTEGER NOT NULL DEFAULT 3,
                is_active INTEGER NOT NULL DEFAULT 1,
                is_public INTEGER NOT NULL DEFAULT 1,
                last_status TEXT NOT NULL DEFAULT 'UP',
                down_since DATETIME,
                created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE IF NOT EXISTS check_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                monitor_id TEXT NOT NULL,
                status_code INTEGER,
                latency_ms INTEGER,
                is_up INTEGER NOT NULL,
                error_msg TEXT,
                checked_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
            );"
        )
        .execute(&pool)
        .await
        .unwrap();

        let monitor_id = "test-mon-24h";
        sqlx::query(
            "INSERT INTO monitors (id, name, url, is_active, is_public) VALUES (?, ?, ?, 1, 1)"
        )
        .bind(monitor_id)
        .bind("Test Monitor 24h")
        .bind("https://example.com")
        .execute(&pool)
        .await
        .unwrap();

        // Insert check in the current hour: UP, 45ms latency
        sqlx::query(
            "INSERT INTO check_logs (monitor_id, status_code, latency_ms, is_up, checked_at, created_at)
             VALUES (?, 200, 45, 1, datetime('now'), datetime('now'))"
        )
        .bind(monitor_id)
        .execute(&pool)
        .await
        .unwrap();

        // Insert another check in the current hour: UP, 55ms latency (avg should be 50ms)
        sqlx::query(
            "INSERT INTO check_logs (monitor_id, status_code, latency_ms, is_up, checked_at, created_at)
             VALUES (?, 200, 55, 1, datetime('now'), datetime('now'))"
        )
        .bind(monitor_id)
        .execute(&pool)
        .await
        .unwrap();

        // Insert check 2 hours ago: DOWN, 120ms latency
        sqlx::query(
            "INSERT INTO check_logs (monitor_id, status_code, latency_ms, is_up, checked_at, created_at)
             VALUES (?, 500, 120, 0, datetime('now', '-2 hours'), datetime('now', '-2 hours'))"
        )
        .bind(monitor_id)
        .execute(&pool)
        .await
        .unwrap();

        let latency_24h = get_24h_latency_history(&pool, monitor_id).await.unwrap();

        // Must always return 24 discrete hourly slots
        assert_eq!(latency_24h.len(), 24);

        // Index 23 is the current hour
        assert_eq!(latency_24h[23].status, "UP");
        assert_eq!(latency_24h[23].avg_latency_ms, 50);

        // Index 21 is 2 hours ago (23 - 2 = 21)
        assert_eq!(latency_24h[21].status, "DOWN");
        assert_eq!(latency_24h[21].avg_latency_ms, 120);

        // Index 0 (23 hours ago) has no logs
        assert_eq!(latency_24h[0].status, "NO_DATA");
        assert_eq!(latency_24h[0].avg_latency_ms, 0);

        // Timestamps end with "Z" and are chronological
        for i in 0..24 {
            assert!(latency_24h[i].timestamp.ends_with('Z'));
            if i > 0 {
                assert!(latency_24h[i].timestamp > latency_24h[i - 1].timestamp);
            }
        }
    }

    #[tokio::test]
    async fn test_domain_expiry_db_persistence() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .unwrap();

        let monitor_id = "test-mon-domain-persist";
        sqlx::query(
            "INSERT INTO monitors (id, name, url, check_interval_secs, timeout_secs, max_retries, is_active, is_public, last_status, check_domain_expiry, domain_warning_days) \
             VALUES (?, 'Domain Test', 'https://example.com', 60, 5, 3, 1, 1, 'UP', 1, 30)"
        )
        .bind(monitor_id)
        .execute(&pool)
        .await
        .unwrap();

        // Initially domain fields are None
        let mon = get_monitor_by_id(&pool, monitor_id).await.unwrap();
        assert_eq!(mon.check_domain_expiry, 1);
        assert_eq!(mon.domain_warning_days, 30);
        assert_eq!(mon.domain_expires_at, None);
        assert_eq!(mon.domain_days_remaining, None);

        // Update domain expiry
        update_domain_expiry(&pool, monitor_id, Some("2028-01-01T00:00:00Z"), Some(450))
            .await
            .unwrap();

        let updated = get_monitor_by_id(&pool, monitor_id).await.unwrap();
        assert_eq!(updated.domain_expires_at.as_deref(), Some("2028-01-01T00:00:00Z"));
        assert_eq!(updated.domain_days_remaining, Some(450));
        assert!(updated.domain_last_checked_at.is_some());

        // Update alert milestone
        update_domain_alert_milestone(&pool, monitor_id, Some(14))
            .await
            .unwrap();
        let mon_with_milestone = get_monitor_by_id(&pool, monitor_id).await.unwrap();
        assert_eq!(mon_with_milestone.last_domain_alert_days, Some(14));
    }
}
