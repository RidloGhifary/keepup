use std::collections::HashMap;
use std::time::Duration;

use chrono::Utc;
use reqwest::Client;
use sqlx::SqlitePool;
use tokio::time;
use tracing::{error, info, warn};

use super::http::{ping_http, CheckResult};
use crate::notifier::{self, AlertPayload};

// ── Row type for the monitors query ──────────────────────────────────────────

/// Lightweight projection of a monitor row – only the columns the scheduler
/// needs to execute a health check and evaluate the failure threshold.
#[derive(sqlx::FromRow, Clone)]
struct Monitor {
    id: String,
    name: String,
    url: String,
    #[allow(dead_code)]
    check_interval_secs: i32,
    timeout_secs: i32,
    max_retries: i32,
    last_status: String,
    down_since: Option<String>,
}

// ── In-memory per-monitor state ──────────────────────────────────────────────

/// Tracks the live health state for a single monitor across scheduler ticks.
#[derive(Debug, Clone)]
struct MonitorState {
    /// Current derived status (`"UP"` or `"DOWN"`).
    status: String,
    /// Running count of consecutive failed checks.
    consecutive_failures: i32,
    /// Timestamp of the most recent UP → DOWN or DOWN → UP transition.
    last_state_change: chrono::DateTime<Utc>,
}

impl Default for MonitorState {
    fn default() -> Self {
        Self {
            status: "UP".to_string(),
            consecutive_failures: 0,
            last_state_change: Utc::now(),
        }
    }
}

// ── Public entry point ───────────────────────────────────────────────────────

/// Run the health-check scheduler loop **forever**.
///
/// Every tick (1 s) the scheduler queries for active monitors whose last
/// check is older than their configured interval, spawns a check for each
/// one, evaluates the failure-threshold state machine, and dispatches
/// webhook alerts on state transitions.
///
/// Also listens on `reload_rx` broadcast channel to re-arm monitor checks
/// immediately upon edit/update.
pub async fn run_scheduler(
    pool: SqlitePool,
    client: Client,
    mut reload_rx: tokio::sync::broadcast::Receiver<String>,
) {
    info!("Health-check scheduler started");

    // In-memory map: monitor_id → live state.
    let mut states: HashMap<String, MonitorState> = HashMap::new();

    let mut tick = time::interval(Duration::from_secs(1));

    loop {
        tokio::select! {
            _ = tick.tick() => {
                let monitors = match fetch_due_monitors(&pool).await {
                    Ok(m) => m,
                    Err(e) => {
                        error!("Failed to query due monitors: {e}");
                        continue;
                    }
                };

                for monitor in monitors {
                    let pool = pool.clone();
                    let client = client.clone();

                    let state = states
                        .entry(monitor.id.clone())
                        .or_insert_with(|| {
                            let last_change = monitor
                                .down_since
                                .as_deref()
                                .and_then(|ds| chrono::DateTime::parse_from_rfc3339(ds).ok())
                                .map(|dt| dt.with_timezone(&Utc))
                                .unwrap_or_else(Utc::now);
                            MonitorState {
                                status: monitor.last_status.clone(),
                                consecutive_failures: 0,
                                last_state_change: last_change,
                            }
                        })
                        .clone();

                    let (new_state, alert) =
                        run_check_and_evaluate(&pool, &client, &monitor, &state).await;

                    states.insert(monitor.id.clone(), new_state);

                    if let Some(payload) = alert {
                        let pool = pool.clone();
                        let client = client.clone();
                        tokio::spawn(async move {
                            notifier::dispatch_alerts(&pool, &client, &payload).await;
                        });
                    }
                }
            }
            reload_res = reload_rx.recv() => {
                match reload_res {
                    Ok(monitor_id) => {
                        info!(monitor_id = %monitor_id, "Reload signal received; re-arming monitor check immediately");
                        if let Ok(Some(monitor)) = fetch_monitor_by_id(&pool, &monitor_id).await {
                            let pool = pool.clone();
                            let client = client.clone();

                            let state = states
                                .entry(monitor.id.clone())
                                .or_insert_with(|| {
                                    let last_change = monitor
                                        .down_since
                                        .as_deref()
                                        .and_then(|ds| chrono::DateTime::parse_from_rfc3339(ds).ok())
                                        .map(|dt| dt.with_timezone(&Utc))
                                        .unwrap_or_else(Utc::now);
                                    MonitorState {
                                        status: monitor.last_status.clone(),
                                        consecutive_failures: 0,
                                        last_state_change: last_change,
                                    }
                                })
                                .clone();

                            let (new_state, alert) =
                                run_check_and_evaluate(&pool, &client, &monitor, &state).await;

                            states.insert(monitor.id.clone(), new_state);

                            if let Some(payload) = alert {
                                let pool = pool.clone();
                                let client = client.clone();
                                tokio::spawn(async move {
                                    notifier::dispatch_alerts(&pool, &client, &payload).await;
                                });
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        warn!("Reload receiver lagged by {n} messages");
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        info!("Reload broadcast channel closed; scheduler stopping reload listener");
                        break;
                    }
                }
            }
        }
    }
}

// ── Core check + state-machine logic ─────────────────────────────────────────

/// Run a single HTTP check, insert the log, evaluate the state machine,
/// and return the updated `MonitorState` plus an optional `AlertPayload`
/// when a transition has occurred.
async fn run_check_and_evaluate(
    pool: &SqlitePool,
    client: &Client,
    monitor: &Monitor,
    prev: &MonitorState,
) -> (MonitorState, Option<AlertPayload>) {
    let result = ping_http(client, &monitor.url, monitor.timeout_secs as u64).await;

    // Persist the raw check result.
    if let Err(e) = insert_check_log(pool, &monitor.id, &result).await {
        error!(monitor_id = %monitor.id, "Failed to insert check log: {e}");
        return (prev.clone(), None);
    }

    // ── State-machine evaluation ─────────────────────────────────────

    let mut next = prev.clone();

    if result.is_up {
        // Success → reset failure counter.
        next.consecutive_failures = 0;

        if prev.status == "DOWN" {
            // ▸ Transition: DOWN → UP (resolved)
            let now = Utc::now();
            let down_since_dt = monitor
                .down_since
                .as_deref()
                .and_then(|ds| chrono::DateTime::parse_from_rfc3339(ds).ok())
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or(prev.last_state_change);

            let downtime_seconds = (now - down_since_dt).num_seconds().max(0) as u64;
            let downtime = format_downtime_duration(downtime_seconds);
            next.status = "UP".to_string();
            next.last_state_change = now;

            info!(
                monitor = %monitor.name,
                "State transition: DOWN → UP (resolved after {downtime})"
            );

            // Reset down_since to NULL and set last_status to UP in SQLite
            if let Err(e) = crate::db::monitors::update_monitor_status(pool, &monitor.id, "UP", None).await {
                error!(monitor_id = %monitor.id, "Failed to update monitor status in DB: {e}");
            }

            let payload = AlertPayload {
                monitor_id: Some(monitor.id.clone()),
                monitor_name: monitor.name.clone(),
                monitor_url: monitor.url.clone(),
                new_status: "UP".to_string(),
                old_status: "DOWN".to_string(),
                http_status: result.status_code.map(|c| c as u16),
                response_time_ms: Some(result.latency_ms as u64),
                error_msg: None,
                downtime_duration: Some(downtime),
                downtime_duration_secs: Some(downtime_seconds),
                timestamp: now.to_rfc3339(),
            };

            return (next, Some(payload));
        }

        info!(
            monitor = %monitor.name,
            latency_ms = result.latency_ms,
            "Check OK"
        );
    } else {
        // Failure → increment counter.
        next.consecutive_failures += 1;

        warn!(
            monitor = %monitor.name,
            failures = next.consecutive_failures,
            threshold = monitor.max_retries,
            error = ?result.error_msg,
            "Check FAILED"
        );

        if prev.status == "UP" && next.consecutive_failures >= monitor.max_retries {
            // ▸ Transition: UP → DOWN
            let now = Utc::now();
            let now_rfc3339 = now.to_rfc3339();
            next.status = "DOWN".to_string();
            next.last_state_change = now;

            info!(
                monitor = %monitor.name,
                "State transition: UP → DOWN"
            );

            // Record last_status = "DOWN" and down_since in SQLite
            if let Err(e) = crate::db::monitors::update_monitor_status(pool, &monitor.id, "DOWN", Some(&now_rfc3339)).await {
                error!(monitor_id = %monitor.id, "Failed to update monitor status in DB: {e}");
            }

            let payload = AlertPayload {
                monitor_id: Some(monitor.id.clone()),
                monitor_name: monitor.name.clone(),
                monitor_url: monitor.url.clone(),
                new_status: "DOWN".to_string(),
                old_status: "UP".to_string(),
                http_status: result.status_code.map(|c| c as u16),
                response_time_ms: Some(result.latency_ms as u64),
                error_msg: result.error_msg.clone(),
                downtime_duration: None,
                downtime_duration_secs: None,
                timestamp: now_rfc3339,
            };

            return (next, Some(payload));
        }
    }

    (next, None)
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Fetch an active monitor by its id.
async fn fetch_monitor_by_id(pool: &SqlitePool, id: &str) -> Result<Option<Monitor>, sqlx::Error> {
    sqlx::query_as::<_, Monitor>(
        r#"
        SELECT m.id, m.name, m.url, m.check_interval_secs, m.timeout_secs, m.max_retries, m.last_status, m.down_since
        FROM   monitors m
        WHERE  m.id = ? AND m.is_active = 1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// Return every active monitor whose last recorded check is older than its
/// `check_interval_secs` (or that has never been checked).
async fn fetch_due_monitors(pool: &SqlitePool) -> Result<Vec<Monitor>, sqlx::Error> {
    sqlx::query_as::<_, Monitor>(
        r#"
        SELECT m.id, m.name, m.url, m.check_interval_secs, m.timeout_secs, m.max_retries, m.last_status, m.down_since
        FROM   monitors m
        WHERE  m.is_active = 1
          AND  NOT EXISTS (
                 SELECT 1
                 FROM   check_logs cl
                 WHERE  cl.monitor_id = m.id
                   AND  cl.checked_at > datetime('now', '-' || m.check_interval_secs || ' seconds')
               )
        "#,
    )
    .fetch_all(pool)
    .await
}

/// Write a single check result into the `check_logs` table.
async fn insert_check_log(
    pool: &SqlitePool,
    monitor_id: &str,
    result: &CheckResult,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO check_logs (monitor_id, status_code, latency_ms, is_up, error_msg, checked_at, created_at)
        VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        "#,
    )
    .bind(monitor_id)
    .bind(result.status_code)
    .bind(result.latency_ms)
    .bind(result.is_up as i32)
    .bind(&result.error_msg)
    .execute(pool)
    .await?;

    Ok(())
}

/// Format duration in seconds to compact form (e.g. "4m 12s", "45s", "1h 4m 12s").
pub fn format_downtime_duration(secs: u64) -> String {
    if secs >= 3600 {
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        let s = secs % 60;
        format!("{h}h {m}m {s}s")
    } else if secs >= 60 {
        let m = secs / 60;
        let s = secs % 60;
        format!("{m}m {s}s")
    } else {
        format!("{secs}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_downtime_duration() {
        assert_eq!(format_downtime_duration(0), "0s");
        assert_eq!(format_downtime_duration(45), "45s");
        assert_eq!(format_downtime_duration(60), "1m 0s");
        assert_eq!(format_downtime_duration(252), "4m 12s");
        assert_eq!(format_downtime_duration(3600), "1h 0m 0s");
        assert_eq!(format_downtime_duration(3665), "1h 1m 5s");
    }
}
