use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

/// Response time percentiles and average for a monitor over a given time window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LatencyPercentiles {
    pub p50: u32,
    pub p95: u32,
    pub p99: u32,
    pub avg: u32,
}

/// Compute response time percentiles (P50, P95, P99) and the average response time
/// for a monitor over the last `hours` hours using SQLite check history.
pub async fn get_latency_percentiles(
    pool: &SqlitePool,
    monitor_id: &str,
    hours: u32,
) -> Result<LatencyPercentiles, sqlx::Error> {
    let rows: Vec<i32> = sqlx::query_scalar::<_, i32>(
        r#"
        SELECT COALESCE(response_time_ms, latency_ms)
        FROM check_logs
        WHERE monitor_id = ? 
          AND COALESCE(created_at, checked_at) >= datetime('now', '-' || ? || ' hours')
          AND COALESCE(response_time_ms, latency_ms) IS NOT NULL
        ORDER BY COALESCE(response_time_ms, latency_ms) ASC
        "#,
    )
    .bind(monitor_id)
    .bind(hours)
    .fetch_all(pool)
    .await?;

    if rows.is_empty() {
        return Ok(LatencyPercentiles {
            p50: 0,
            p95: 0,
            p99: 0,
            avg: 0,
        });
    }

    let len = rows.len();
    let sum: i64 = rows.iter().map(|&v| v as i64).sum();
    let avg = (sum / len as i64) as u32;

    let calc_p = |pct: f64| -> u32 {
        let idx = ((len as f64 - 1.0) * pct).round() as usize;
        rows[idx.min(len - 1)].max(0) as u32
    };

    Ok(LatencyPercentiles {
        p50: calc_p(0.50),
        p95: calc_p(0.95),
        p99: calc_p(0.99),
        avg,
    })
}

/// Uptime SLA percentages over 24h, 7d, and 30d windows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UptimeSlas {
    pub uptime_24h: f64,
    pub uptime_7d: f64,
    pub uptime_30d: f64,
}

#[derive(Debug, sqlx::FromRow)]
struct SlaAggRow {
    total_24h: i64,
    up_24h: i64,
    total_7d: i64,
    up_7d: i64,
    total_30d: i64,
    up_30d: i64,
}

/// Compute uptime percentages over 24h, 7d, and 30d windows ((successful_checks / total_checks) * 100).
pub async fn get_uptime_slas(
    pool: &SqlitePool,
    monitor_id: &str,
) -> Result<UptimeSlas, sqlx::Error> {
    let row = sqlx::query_as::<_, SlaAggRow>(
        r#"
        SELECT 
            COUNT(CASE WHEN COALESCE(created_at, checked_at) >= datetime('now', '-24 hours') THEN 1 END) AS total_24h,
            COUNT(CASE WHEN COALESCE(created_at, checked_at) >= datetime('now', '-24 hours') AND is_up = 1 THEN 1 END) AS up_24h,
            COUNT(CASE WHEN COALESCE(created_at, checked_at) >= datetime('now', '-7 days') THEN 1 END) AS total_7d,
            COUNT(CASE WHEN COALESCE(created_at, checked_at) >= datetime('now', '-7 days') AND is_up = 1 THEN 1 END) AS up_7d,
            COUNT(CASE WHEN COALESCE(created_at, checked_at) >= datetime('now', '-30 days') THEN 1 END) AS total_30d,
            COUNT(CASE WHEN COALESCE(created_at, checked_at) >= datetime('now', '-30 days') AND is_up = 1 THEN 1 END) AS up_30d
        FROM check_logs
        WHERE monitor_id = ?
        "#,
    )
    .bind(monitor_id)
    .fetch_one(pool)
    .await?;

    let calc_pct = |total: i64, up: i64| -> f64 {
        if total == 0 {
            100.0
        } else {
            ((up as f64 / total as f64) * 10000.0).round() / 100.0
        }
    };

    Ok(UptimeSlas {
        uptime_24h: calc_pct(row.total_24h, row.up_24h),
        uptime_7d: calc_pct(row.total_7d, row.up_7d),
        uptime_30d: calc_pct(row.total_30d, row.up_30d),
    })
}

/// Detailed check-log record for deep-dive monitor telemetry.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DetailCheckLog {
    pub id: i64,
    pub created_at: String,
    pub status_code: Option<i32>,
    pub response_time_ms: Option<i64>,
    pub error_message: Option<String>,
    pub tls_days_remaining: Option<i64>,
    pub domain_days_remaining: Option<i64>,
    pub is_up: bool,
}

#[derive(Debug, sqlx::FromRow)]
struct RawCheckLogRow {
    id: i64,
    created_at: String,
    status_code: Option<i32>,
    response_time_ms: Option<i64>,
    error_msg: Option<String>,
    is_up: i32,
    domain_days_remaining: Option<i64>,
}

/// Fetch the most recent check logs for a monitor (up to limit entries).
pub async fn get_recent_check_logs(
    pool: &SqlitePool,
    monitor_id: &str,
    limit: i64,
) -> Result<Vec<DetailCheckLog>, sqlx::Error> {
    let rows = sqlx::query_as::<_, RawCheckLogRow>(
        r#"
        SELECT 
            id,
            COALESCE(created_at, checked_at) AS created_at,
            status_code,
            COALESCE(response_time_ms, latency_ms) AS response_time_ms,
            error_msg,
            is_up,
            domain_days_remaining
        FROM check_logs
        WHERE monitor_id = ?
        ORDER BY COALESCE(created_at, checked_at) DESC, id DESC
        LIMIT ?
        "#,
    )
    .bind(monitor_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let logs = rows
        .into_iter()
        .map(|r| DetailCheckLog {
            id: r.id,
            created_at: r.created_at,
            status_code: r.status_code,
            response_time_ms: r.response_time_ms,
            error_message: r.error_msg,
            tls_days_remaining: None,
            domain_days_remaining: r.domain_days_remaining,
            is_up: r.is_up == 1,
        })
        .collect();

    Ok(logs)
}

/// Purge historical check logs older than `retention_days` in chunks of `batch_size`.
/// Chunked batch deletion prevents holding an exclusive write lock for extended periods,
/// eliminating query bottlenecks while preventing database bloat over time.
pub async fn purge_historical_check_logs(
    pool: &SqlitePool,
    retention_days: u32,
    batch_size: u32,
) -> Result<u64, sqlx::Error> {
    let mut total_deleted = 0u64;
    let bsize = batch_size.max(100) as i64;

    loop {
        let result = sqlx::query(
            r#"
            DELETE FROM check_logs
            WHERE id IN (
                SELECT id FROM check_logs
                WHERE COALESCE(created_at, checked_at) < datetime('now', '-' || ? || ' days')
                LIMIT ?
            )
            "#,
        )
        .bind(retention_days)
        .bind(bsize)
        .execute(pool)
        .await?;

        let affected = result.rows_affected();
        total_deleted += affected;

        if affected == 0 {
            break;
        }

        // Yield slightly between chunks to allow other concurrent queries
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }

    Ok(total_deleted)
}

#[cfg(test)]
pub mod tests {
    use super::*;

    async fn setup_test_db() -> SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("Failed to connect to in-memory SQLite");

        crate::db::run_migrations(&pool)
            .await
            .expect("Failed to run migrations");

        pool
    }

    #[tokio::test]
    async fn test_empty_check_logs_returns_zeros() {
        let pool = setup_test_db().await;
        let res = get_latency_percentiles(&pool, "m-empty", 24).await.unwrap();
        assert_eq!(
            res,
            LatencyPercentiles {
                p50: 0,
                p95: 0,
                p99: 0,
                avg: 0
            }
        );
    }

    #[tokio::test]
    async fn test_single_check_log_percentiles() {
        let pool = setup_test_db().await;

        sqlx::query(
            "INSERT INTO monitors (id, name, url, is_active, is_public) VALUES ('m-single', 'Single Test', 'https://test.local', 1, 1)"
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO check_logs (monitor_id, status_code, latency_ms, is_up, checked_at, created_at)
             VALUES ('m-single', 200, 42, 1, datetime('now'), datetime('now'))"
        )
        .execute(&pool)
        .await
        .unwrap();

        let res = get_latency_percentiles(&pool, "m-single", 24).await.unwrap();
        assert_eq!(
            res,
            LatencyPercentiles {
                p50: 42,
                p95: 42,
                p99: 42,
                avg: 42
            }
        );
    }

    #[tokio::test]
    async fn test_latency_percentiles_calculation_with_100_samples() {
        let pool = setup_test_db().await;

        sqlx::query(
            "INSERT INTO monitors (id, name, url, is_active, is_public) VALUES ('m-100', '100 Samples', 'https://test.local', 1, 1)"
        )
        .execute(&pool)
        .await
        .unwrap();

        // Seed 100 sample response times from 10ms to 406ms (10 + i * 4)
        for i in 0..100 {
            let lat: i32 = 10 + i * 4;
            sqlx::query(
                "INSERT INTO check_logs (monitor_id, status_code, latency_ms, is_up, checked_at, created_at)
                 VALUES ('m-100', 200, ?, 1, datetime('now'), datetime('now'))"
            )
            .bind(lat)
            .execute(&pool)
            .await
            .unwrap();
        }

        let res = get_latency_percentiles(&pool, "m-100", 24).await.unwrap();

        // With 100 samples:
        // P50: index round((99) * 0.50) = round(49.5) = 50 -> 10 + 50 * 4 = 210
        // P95: index round((99) * 0.95) = round(94.05) = 94 -> 10 + 94 * 4 = 386
        // P99: index round((99) * 0.99) = round(98.01) = 98 -> 10 + 98 * 4 = 402
        // Avg: sum(10..=406) / 100 = 208
        assert_eq!(res.p50, 210);
        assert_eq!(res.p95, 386);
        assert_eq!(res.p99, 402);
        assert_eq!(res.avg, 208);
    }

    #[tokio::test]
    async fn test_window_filtering_excludes_older_checks() {
        let pool = setup_test_db().await;

        sqlx::query(
            "INSERT INTO monitors (id, name, url, is_active, is_public) VALUES ('m-window', 'Window Test', 'https://test.local', 1, 1)"
        )
        .execute(&pool)
        .await
        .unwrap();

        // Insert check 26 hours ago (should be excluded when hours=24)
        sqlx::query(
            "INSERT INTO check_logs (monitor_id, status_code, latency_ms, is_up, checked_at, created_at)
             VALUES ('m-window', 200, 999, 1, datetime('now', '-26 hours'), datetime('now', '-26 hours'))"
        )
        .execute(&pool)
        .await
        .unwrap();

        // Insert check 2 hours ago (included)
        sqlx::query(
            "INSERT INTO check_logs (monitor_id, status_code, latency_ms, is_up, checked_at, created_at)
             VALUES ('m-window', 200, 50, 1, datetime('now', '-2 hours'), datetime('now', '-2 hours'))"
        )
        .execute(&pool)
        .await
        .unwrap();

        let res = get_latency_percentiles(&pool, "m-window", 24).await.unwrap();
        assert_eq!(res.p50, 50);
        assert_eq!(res.avg, 50);

        // When requesting 48 hours, both are included
        let res_48 = get_latency_percentiles(&pool, "m-window", 48).await.unwrap();
        assert_eq!(res_48.avg, (50 + 999) / 2);
    }

    #[tokio::test]
    async fn test_get_uptime_slas_and_recent_logs() {
        let pool = setup_test_db().await;

        sqlx::query(
            "INSERT INTO monitors (id, name, url, is_active, is_public) VALUES ('m-sla', 'SLA Test', 'https://test.local', 1, 1)"
        )
        .execute(&pool)
        .await
        .unwrap();

        // When no logs exist, SLAs return 100.0%
        let empty_slas = get_uptime_slas(&pool, "m-sla").await.unwrap();
        assert_eq!(empty_slas.uptime_24h, 100.0);
        assert_eq!(empty_slas.uptime_7d, 100.0);
        assert_eq!(empty_slas.uptime_30d, 100.0);

        // Add 3 UP logs and 1 DOWN log within last 24h
        for _ in 0..3 {
            sqlx::query(
                "INSERT INTO check_logs (monitor_id, status_code, latency_ms, is_up, checked_at, created_at)
                 VALUES ('m-sla', 200, 25, 1, datetime('now', '-1 hour'), datetime('now', '-1 hour'))"
            )
            .execute(&pool)
            .await
            .unwrap();
        }

        sqlx::query(
            "INSERT INTO check_logs (monitor_id, status_code, latency_ms, is_up, error_msg, checked_at, created_at)
             VALUES ('m-sla', 500, 45, 0, 'Internal Server Error', datetime('now', '-2 hours'), datetime('now', '-2 hours'))"
        )
        .execute(&pool)
        .await
        .unwrap();

        let slas = get_uptime_slas(&pool, "m-sla").await.unwrap();
        // 3 up out of 4 total = 75.0%
        assert_eq!(slas.uptime_24h, 75.0);
        assert_eq!(slas.uptime_7d, 75.0);
        assert_eq!(slas.uptime_30d, 75.0);

        let logs = get_recent_check_logs(&pool, "m-sla", 50).await.unwrap();
        assert_eq!(logs.len(), 4);
        assert_eq!(logs[0].response_time_ms, Some(25));
        assert!(logs[0].is_up);
    }

    #[tokio::test]
    async fn test_purge_historical_check_logs_retention() {
        let pool = setup_test_db().await;

        // Insert monitor
        sqlx::query(
            "INSERT INTO monitors (id, name, url, check_interval_secs) VALUES ('m-ret', 'Ret Test', 'https://example.com', 60)"
        )
        .execute(&pool)
        .await
        .unwrap();

        // Insert 5 old logs (older than 90 days)
        for _ in 0..5 {
            sqlx::query(
                "INSERT INTO check_logs (monitor_id, status_code, latency_ms, is_up, checked_at, created_at)
                 VALUES ('m-ret', 200, 20, 1, datetime('now', '-95 days'), datetime('now', '-95 days'))"
            )
            .execute(&pool)
            .await
            .unwrap();
        }

        // Insert 3 recent logs (recent 1 day)
        for _ in 0..3 {
            sqlx::query(
                "INSERT INTO check_logs (monitor_id, status_code, latency_ms, is_up, checked_at, created_at)
                 VALUES ('m-ret', 200, 20, 1, datetime('now', '-1 day'), datetime('now', '-1 day'))"
            )
            .execute(&pool)
            .await
            .unwrap();
        }

        let purged = purge_historical_check_logs(&pool, 90, 2).await.unwrap();
        assert_eq!(purged, 5);

        let remaining = get_recent_check_logs(&pool, "m-ret", 50).await.unwrap();
        assert_eq!(remaining.len(), 3);
    }
}
