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
}
