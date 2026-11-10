use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::db::monitors::{DailyUptime, HourlyLatency};
use crate::error::AppError;

/// Batched metric aggregations across multiple monitors for public status pages.
#[derive(Debug, Clone)]
pub struct BatchMonitorMetrics {
    pub latest_is_up: Option<bool>,
    pub last_latency_ms: Option<i64>,
    pub uptime_24h: f64,
    pub uptime_history: Vec<DailyUptime>,
    pub latency_24h: Vec<HourlyLatency>,
    pub latency: LatencyPercentiles,
}

/// Compute public status metrics in batched O(1) set-based queries rather than 6N per-monitor queries.
pub async fn get_batch_public_metrics(
    pool: &SqlitePool,
    monitor_ids: &[String],
) -> Result<HashMap<String, BatchMonitorMetrics>, AppError> {
    if monitor_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let placeholders = vec!["?"; monitor_ids.len()].join(",");

    // 1. Latest check per monitor
    #[derive(sqlx::FromRow)]
    struct LatestRow {
        monitor_id: String,
        is_up: i32,
        latency_ms: Option<i64>,
    }

    let latest_sql = format!(
        r#"
        SELECT cl.monitor_id, cl.is_up, COALESCE(cl.response_time_ms, cl.latency_ms) AS latency_ms
        FROM check_logs cl
        INNER JOIN (
            SELECT monitor_id, MAX(id) AS max_id
            FROM check_logs
            WHERE monitor_id IN ({placeholders})
            GROUP BY monitor_id
        ) latest ON cl.id = latest.max_id
        "#
    );
    let mut query = sqlx::query_as::<_, LatestRow>(&latest_sql);
    for id in monitor_ids {
        query = query.bind(id);
    }
    let latest_rows = query.fetch_all(pool).await?;
    let mut latest_map: HashMap<String, (bool, Option<i64>)> = HashMap::new();
    for r in latest_rows {
        latest_map.insert(r.monitor_id, (r.is_up == 1, r.latency_ms));
    }

    // 2. 24h uptime aggregation
    #[derive(sqlx::FromRow)]
    struct AggRow {
        monitor_id: String,
        total: i64,
        up_count: i64,
    }
    let agg_sql = format!(
        r#"
        SELECT monitor_id,
               COUNT(*) AS total,
               COALESCE(SUM(CASE WHEN is_up = 1 THEN 1 ELSE 0 END), 0) AS up_count
        FROM check_logs
        WHERE monitor_id IN ({placeholders})
          AND checked_at >= datetime('now', '-24 hours')
        GROUP BY monitor_id
        "#
    );
    let mut query = sqlx::query_as::<_, AggRow>(&agg_sql);
    for id in monitor_ids {
        query = query.bind(id);
    }
    let agg_rows = query.fetch_all(pool).await?;
    let mut uptime_map: HashMap<String, f64> = HashMap::new();
    for r in agg_rows {
        let pct = if r.total > 0 {
            (r.up_count as f64 / r.total as f64) * 100.0
        } else {
            100.0
        };
        uptime_map.insert(r.monitor_id, (pct * 100.0).round() / 100.0);
    }

    // 3. 24h percentiles: fetch sorted latencies per monitor
    #[derive(sqlx::FromRow)]
    struct LatRow {
        monitor_id: String,
        lat: i32,
    }
    let lat_sql = format!(
        r#"
        SELECT monitor_id, COALESCE(response_time_ms, latency_ms) AS lat
        FROM check_logs
        WHERE monitor_id IN ({placeholders})
          AND checked_at >= datetime('now', '-24 hours')
          AND COALESCE(response_time_ms, latency_ms) IS NOT NULL
        ORDER BY monitor_id, lat ASC
        "#
    );
    let mut query = sqlx::query_as::<_, LatRow>(&lat_sql);
    for id in monitor_ids {
        query = query.bind(id);
    }
    let lat_rows = query.fetch_all(pool).await?;
    let mut lats_by_monitor: HashMap<String, Vec<i32>> = HashMap::new();
    for r in lat_rows {
        lats_by_monitor.entry(r.monitor_id).or_default().push(r.lat);
    }
    let mut pct_map: HashMap<String, LatencyPercentiles> = HashMap::new();
    for (mid, rows) in lats_by_monitor {
        let len = rows.len();
        if len == 0 {
            continue;
        }
        let sum: i64 = rows.iter().map(|&v| v as i64).sum();
        let avg = (sum / len as i64) as u32;
        let calc_p = |pct: f64| -> u32 {
            let idx = ((len as f64 - 1.0) * pct).round() as usize;
            rows[idx.min(len - 1)].max(0) as u32
        };
        pct_map.insert(
            mid,
            LatencyPercentiles {
                p50: calc_p(0.50),
                p95: calc_p(0.95),
                p99: calc_p(0.99),
                avg,
            },
        );
    }

    // 4. 7-day daily uptime history
    #[derive(sqlx::FromRow)]
    struct DailyRow {
        monitor_id: String,
        day: String,
        total: i64,
        up_count: i64,
    }
    let daily_sql = format!(
        r#"
        SELECT monitor_id,
               date(checked_at) AS day,
               COUNT(*) AS total,
               COALESCE(SUM(CASE WHEN is_up = 1 THEN 1 ELSE 0 END), 0) AS up_count
        FROM check_logs
        WHERE monitor_id IN ({placeholders})
          AND checked_at >= datetime('now', '-7 days')
        GROUP BY monitor_id, date(checked_at)
        "#
    );
    let mut query = sqlx::query_as::<_, DailyRow>(&daily_sql);
    for id in monitor_ids {
        query = query.bind(id);
    }
    let daily_rows = query.fetch_all(pool).await?;
    let mut daily_by_monitor: HashMap<String, HashMap<String, (i64, i64)>> = HashMap::new();
    for r in daily_rows {
        daily_by_monitor
            .entry(r.monitor_id)
            .or_default()
            .insert(r.day, (r.total, r.up_count));
    }

    let today = chrono::Utc::now().naive_utc().date();

    // 5. 24-hour hourly latency history
    #[derive(sqlx::FromRow)]
    struct HourlyRow {
        monitor_id: String,
        hour_slot: String,
        total: i64,
        up_count: i64,
        avg_latency: f64,
    }
    let hourly_sql = format!(
        r#"
        SELECT monitor_id,
               strftime('%Y-%m-%d %H:00:00', checked_at) AS hour_slot,
               COUNT(*) AS total,
               COALESCE(SUM(CASE WHEN is_up = 1 THEN 1 ELSE 0 END), 0) AS up_count,
               COALESCE(AVG(COALESCE(response_time_ms, latency_ms)), 0.0) AS avg_latency
        FROM check_logs
        WHERE monitor_id IN ({placeholders})
          AND checked_at >= datetime('now', '-24 hours')
        GROUP BY monitor_id, strftime('%Y-%m-%d %H:00:00', checked_at)
        "#
    );
    let mut query = sqlx::query_as::<_, HourlyRow>(&hourly_sql);
    for id in monitor_ids {
        query = query.bind(id);
    }
    let hourly_rows = query.fetch_all(pool).await?;
    let mut hourly_by_monitor: HashMap<String, HashMap<String, (i64, i64, f64)>> = HashMap::new();
    for r in hourly_rows {
        hourly_by_monitor
            .entry(r.monitor_id)
            .or_default()
            .insert(r.hour_slot, (r.total, r.up_count, r.avg_latency));
    }

    use chrono::Timelike;
    let now = chrono::Utc::now();
    let current_hour = now
        .date_naive()
        .and_hms_opt(now.hour(), 0, 0)
        .unwrap_or_else(|| now.naive_utc());

    // Assemble results
    let mut results = HashMap::with_capacity(monitor_ids.len());
    for id in monitor_ids {
        let (latest_is_up, last_latency_ms) = match latest_map.get(id) {
            Some(&(is_up, lat)) => (Some(is_up), lat),
            None => (None, None),
        };

        let uptime_24h = uptime_map.get(id).copied().unwrap_or(100.0);

        let latency = pct_map
            .get(id)
            .cloned()
            .unwrap_or(LatencyPercentiles {
                p50: 0,
                p95: 0,
                p99: 0,
                avg: 0,
            });

        // 7-day buckets
        let day_map = daily_by_monitor.get(id);
        let mut uptime_history = Vec::with_capacity(7);
        for i in (0..7).rev() {
            let date_obj = today - chrono::Duration::days(i);
            let date_str = date_obj.format("%Y-%m-%d").to_string();

            let (uptime_pct, status) = match day_map.and_then(|m| m.get(&date_str)) {
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

            uptime_history.push(DailyUptime {
                date: date_str,
                uptime_pct,
                status,
            });
        }

        // 24-hour hourly latency buckets
        let hour_map = hourly_by_monitor.get(id);
        let mut latency_24h = Vec::with_capacity(24);
        for i in (0..24).rev() {
            let slot_time = current_hour - chrono::Duration::hours(i);
            let sql_key = slot_time.format("%Y-%m-%d %H:00:00").to_string();
            let timestamp = slot_time.format("%Y-%m-%dT%H:00:00Z").to_string();

            let (avg_latency_ms, status) = match hour_map.and_then(|m| m.get(&sql_key)) {
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

        results.insert(
            id.clone(),
            BatchMonitorMetrics {
                latest_is_up,
                last_latency_ms,
                uptime_24h,
                uptime_history,
                latency_24h,
                latency,
            },
        );
    }

    Ok(results)
}

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
    ssl_days_remaining: Option<i64>,
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
            domain_days_remaining,
            ssl_days_remaining
        FROM check_logs
        WHERE monitor_id = ?
        ORDER BY checked_at DESC, id DESC
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
            tls_days_remaining: r.ssl_days_remaining,
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
