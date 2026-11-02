use std::collections::HashMap;
use std::time::Duration;

use chrono::Utc;
use reqwest::Client;
use sqlx::SqlitePool;
use tokio::time;
use tracing::{error, info, warn};

use super::http::{ping_http, CheckResult};
use crate::alerting::AlertPayload;

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
    #[sqlx(default)]
    pub check_domain_expiry: i32,
    #[sqlx(default)]
    #[allow(dead_code)]
    pub domain_warning_days: i32,
    pub domain_expires_at: Option<String>,
    pub domain_days_remaining: Option<i32>,
    pub last_domain_alert_days: Option<i32>,
    pub domain_last_checked_at: Option<String>,
    #[sqlx(default)]
    pub check_ssl: i32,
    #[sqlx(default)]
    #[allow(dead_code)]
    pub ssl_warning_days: i32,
    pub ssl_expires_at: Option<String>,
    pub ssl_days_remaining: Option<i32>,
    pub ssl_last_checked_at: Option<String>,
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
                            crate::alerting::dispatch_alerts(&pool, &client, &payload).await;
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
                                    crate::alerting::dispatch_alerts(&pool, &client, &payload).await;
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
    // ── Check if monitor is in active maintenance window ────────────
    let in_maintenance = crate::db::maintenance::is_monitor_in_maintenance(pool, &monitor.id)
        .await
        .unwrap_or(false);

    // ── Check RDAP domain registration expiration if enabled ────────
    let mut domain_days: Option<i32> = monitor.domain_days_remaining;
    let mut domain_expires: Option<String> = monitor.domain_expires_at.clone();

    if monitor.check_domain_expiry == 1 {
        let needs_lookup = match &monitor.domain_last_checked_at {
            None => true,
            Some(last_str) => match chrono::DateTime::parse_from_rfc3339(last_str) {
                Ok(dt) => (Utc::now() - dt.with_timezone(&Utc)).num_hours() >= 12,
                Err(_) => true,
            },
        } || monitor.domain_expires_at.is_none();

        if needs_lookup {
            match crate::checker::domain::check_domain_expiration(client, &monitor.url).await {
                Ok(info) => {
                    domain_days = Some(info.days_remaining as i32);
                    domain_expires = Some(info.expires_at.clone());
                    if let Err(e) = crate::db::monitors::update_domain_expiry(
                        pool,
                        &monitor.id,
                        Some(&info.expires_at),
                        Some(info.days_remaining as i32),
                    )
                    .await
                    {
                        error!(monitor_id = %monitor.id, "Failed to update domain expiry in DB: {e}");
                    }
                }
                Err(e) => {
                    warn!(monitor = %monitor.name, "RDAP domain expiration check failed: {e}");
                }
            }
        } else if let Some(ref exp_str) = monitor.domain_expires_at {
            if let Ok((_, remaining)) = crate::checker::domain::calculate_days_remaining(exp_str, Utc::now()) {
                domain_days = Some(remaining as i32);
            }
        }

        // Milestone Alert Suppression Tracking: only dispatch at [30, 14, 7, 3, 1] days
        if let Some(days) = domain_days {
            if days <= monitor.domain_warning_days {
                let milestone = [0, 1, 3, 7, 14, 30]
                    .into_iter()
                    .find(|&m| days <= m && m <= monitor.domain_warning_days);

                if let Some(m) = milestone {
                    if monitor.last_domain_alert_days != Some(m) {
                        info!(
                            monitor = %monitor.name,
                            days_remaining = days,
                            milestone = m,
                            "Domain expiration alert milestone reached; dispatching alert"
                        );

                        if let Err(e) = crate::db::monitors::update_domain_alert_milestone(
                            pool,
                            &monitor.id,
                            Some(m),
                        )
                        .await
                        {
                            error!(monitor_id = %monitor.id, "Failed to record domain alert milestone: {e}");
                        }

                        let pool_alert = pool.clone();
                        let client_alert = client.clone();
                        let alert_payload = AlertPayload {
                            monitor_id: Some(monitor.id.clone()),
                            monitor_name: monitor.name.clone(),
                            monitor_url: monitor.url.clone(),
                            new_status: "DOMAIN_EXPIRING_SOON".to_string(),
                            old_status: monitor.last_status.clone(),
                            http_status: None,
                            response_time_ms: None,
                            error_msg: Some(if days <= 0 {
                                format!("Domain registration has expired ({days} days remaining)")
                            } else {
                                format!("Domain registration expires in {days} day(s)")
                            }),
                            downtime_duration: None,
                            downtime_duration_secs: None,
                            timestamp: Utc::now().to_rfc3339(),
                            domain_days_remaining: Some(days as i64),
                            domain_expires_at: domain_expires.clone(),
                        };

                        tokio::spawn(async move {
                            crate::alerting::dispatch_alerts(&pool_alert, &client_alert, &alert_payload).await;
                        });
                    }
                }
            } else if monitor.last_domain_alert_days.is_some() {
                let _ = crate::db::monitors::update_domain_alert_milestone(pool, &monitor.id, None).await;
            }
        }
    }

    // ── Check TLS / SSL certificate expiration if enabled ───────────
    let mut ssl_days: Option<i32> = monitor.ssl_days_remaining;
    if monitor.check_ssl == 1 {
        let needs_ssl_lookup = match &monitor.ssl_last_checked_at {
            None => true,
            Some(last_str) => match chrono::DateTime::parse_from_rfc3339(last_str) {
                Ok(dt) => (Utc::now() - dt.with_timezone(&Utc)).num_hours() >= 12,
                Err(_) => true,
            },
        } || monitor.ssl_expires_at.is_none();

        if needs_ssl_lookup {
            match crate::checker::tls::check_ssl_expiration(&monitor.url, 10).await {
                Ok(info) => {
                    ssl_days = Some(info.days_remaining as i32);
                    if let Err(e) = crate::db::monitors::update_ssl_expiry(
                        pool,
                        &monitor.id,
                        Some(&info.expires_at),
                        Some(info.days_remaining as i32),
                    )
                    .await
                    {
                        error!(monitor_id = %monitor.id, "Failed to update SSL expiry in DB: {e}");
                    }
                }
                Err(e) => {
                    warn!(monitor = %monitor.name, "TLS/SSL certificate check failed: {e}");
                }
            }
        }
    }

    if in_maintenance {
        let result = ping_http(client, &monitor.url, monitor.timeout_secs as u64).await;

        // Persist check log with is_up = true (no SLA penalty) and error_msg = "MAINTENANCE"
        let maint_result = CheckResult {
            status_code: result.status_code,
            latency_ms: result.latency_ms,
            is_up: true,
            error_msg: Some("MAINTENANCE".to_string()),
        };
        if let Err(e) = insert_check_log(pool, &monitor.id, &maint_result, domain_days, ssl_days).await {
            error!(monitor_id = %monitor.id, "Failed to insert check log during maintenance: {e}");
        }

        if prev.status != "MAINTENANCE" {
            info!(
                monitor = %monitor.name,
                "Entering planned maintenance window; suppressing alerts"
            );
            if let Err(e) = crate::db::monitors::update_monitor_status(pool, &monitor.id, "MAINTENANCE", None).await {
                error!(monitor_id = %monitor.id, "Failed to update monitor status to MAINTENANCE in DB: {e}");
            }
        }

        let mut next = prev.clone();
        next.status = "MAINTENANCE".to_string();
        next.consecutive_failures = 0;
        return (next, None);
    }

    let result = ping_http(client, &monitor.url, monitor.timeout_secs as u64).await;

    // Persist the raw check result.
    if let Err(e) = insert_check_log(pool, &monitor.id, &result, domain_days, ssl_days).await {
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
                domain_days_remaining: domain_days.map(|d| d as i64),
                domain_expires_at: domain_expires.clone(),
            };

            return (next, Some(payload));
        } else if prev.status == "MAINTENANCE" {
            let now = Utc::now();
            next.status = "UP".to_string();
            next.last_state_change = now;

            info!(
                monitor = %monitor.name,
                "Exiting planned maintenance window -> UP"
            );

            if let Err(e) = crate::db::monitors::update_monitor_status(pool, &monitor.id, "UP", None).await {
                error!(monitor_id = %monitor.id, "Failed to update monitor status in DB: {e}");
            }

            return (next, None);
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

        if (prev.status == "UP" || prev.status == "MAINTENANCE") && next.consecutive_failures >= monitor.max_retries {
            // ▸ Transition: UP/MAINTENANCE → DOWN
            let now = Utc::now();
            let now_rfc3339 = now.to_rfc3339();
            let old_status = prev.status.clone();
            next.status = "DOWN".to_string();
            next.last_state_change = now;

            info!(
                monitor = %monitor.name,
                "State transition: {old_status} → DOWN"
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
                old_status,
                http_status: result.status_code.map(|c| c as u16),
                response_time_ms: Some(result.latency_ms as u64),
                error_msg: result.error_msg.clone(),
                downtime_duration: None,
                downtime_duration_secs: None,
                timestamp: now_rfc3339,
                domain_days_remaining: domain_days.map(|d| d as i64),
                domain_expires_at: domain_expires,
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
        SELECT m.id, m.name, m.url, m.check_interval_secs, m.timeout_secs, m.max_retries, m.last_status, m.down_since,
               m.check_domain_expiry, m.domain_warning_days, m.domain_expires_at, m.domain_days_remaining,
               m.last_domain_alert_days, m.domain_last_checked_at,
               m.check_ssl, m.ssl_warning_days, m.ssl_expires_at, m.ssl_days_remaining, m.ssl_last_checked_at
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
        SELECT m.id, m.name, m.url, m.check_interval_secs, m.timeout_secs, m.max_retries, m.last_status, m.down_since,
               m.check_domain_expiry, m.domain_warning_days, m.domain_expires_at, m.domain_days_remaining,
               m.last_domain_alert_days, m.domain_last_checked_at,
               m.check_ssl, m.ssl_warning_days, m.ssl_expires_at, m.ssl_days_remaining, m.ssl_last_checked_at
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
    domain_days_remaining: Option<i32>,
    ssl_days_remaining: Option<i32>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO check_logs (monitor_id, status_code, latency_ms, response_time_ms, is_up, error_msg, checked_at, created_at, domain_days_remaining, ssl_days_remaining)
        VALUES (?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, ?, ?)
        "#,
    )
    .bind(monitor_id)
    .bind(result.status_code)
    .bind(result.latency_ms)
    .bind(result.latency_ms)
    .bind(result.is_up as i32)
    .bind(&result.error_msg)
    .bind(domain_days_remaining)
    .bind(ssl_days_remaining)
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

    #[tokio::test]
    async fn test_maintenance_window_suppresses_alerts() {
        use chrono::Duration;
        use sqlx::sqlite::SqlitePoolOptions;

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .unwrap();

        let monitor_id = "test-sched-maint-1";
        sqlx::query(
            "INSERT INTO monitors (id, name, url, check_interval_secs, timeout_secs, max_retries, is_active, is_public, last_status) \
             VALUES (?, 'Test Service', 'http://127.0.0.1:9', 60, 1, 1, 1, 1, 'UP')"
        )
        .bind(monitor_id)
        .execute(&pool)
        .await
        .unwrap();

        let now = Utc::now();
        let start = now - Duration::minutes(5);
        let end = now + Duration::hours(1);

        crate::db::maintenance::create_maintenance_window(
            &pool,
            crate::db::maintenance::CreateMaintenanceDto {
                title: "Scheduled DB Patching".to_string(),
                description: None,
                start_time: start.to_rfc3339(),
                end_time: end.to_rfc3339(),
                monitor_ids: vec![monitor_id.to_string()],
            },
        )
        .await
        .unwrap();

        let client = reqwest::Client::new();
        let monitor = Monitor {
            id: monitor_id.to_string(),
            name: "Test Service".to_string(),
            url: "http://127.0.0.1:9".to_string(), // Unreachable endpoint
            check_interval_secs: 60,
            timeout_secs: 1,
            max_retries: 1,
            last_status: "UP".to_string(),
            down_since: None,
            check_domain_expiry: 0,
            domain_warning_days: 30,
            domain_expires_at: None,
            domain_days_remaining: None,
            last_domain_alert_days: None,
            domain_last_checked_at: None,
            check_ssl: 0,
            ssl_warning_days: 30,
            ssl_expires_at: None,
            ssl_days_remaining: None,
            ssl_last_checked_at: None,
        };

        let prev = MonitorState {
            status: "UP".to_string(),
            consecutive_failures: 0,
            last_state_change: now,
        };

        let (new_state, alert) = run_check_and_evaluate(&pool, &client, &monitor, &prev).await;

        // Alerts must be suppressed (None)
        assert!(alert.is_none(), "Alert must be suppressed during maintenance");
        // State must transition to MAINTENANCE
        assert_eq!(new_state.status, "MAINTENANCE");

        // Verify monitor's last_status in DB is updated to MAINTENANCE
        let updated_mon = fetch_monitor_by_id(&pool, monitor_id).await.unwrap().unwrap();
        assert_eq!(updated_mon.last_status, "MAINTENANCE");

        // Check log must exist with is_up = 1 and error_msg = "MAINTENANCE"
        let log: (i32, Option<String>) = sqlx::query_as("SELECT is_up, error_msg FROM check_logs WHERE monitor_id = ? ORDER BY id DESC LIMIT 1")
            .bind(monitor_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(log.0, 1, "is_up should be 1 during maintenance to prevent SLA penalty");
        assert_eq!(log.1.as_deref(), Some("MAINTENANCE"));
    }

    #[tokio::test]
    async fn test_domain_expiry_milestone_suppression() {
        use sqlx::sqlite::SqlitePoolOptions;

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .unwrap();

        let monitor_id = "test-sched-domain-1";
        sqlx::query(
            "INSERT INTO monitors (id, name, url, check_interval_secs, timeout_secs, max_retries, is_active, is_public, last_status, check_domain_expiry, domain_warning_days, domain_days_remaining, domain_expires_at, last_domain_alert_days, domain_last_checked_at) \
             VALUES (?, 'Domain Test', 'http://127.0.0.1:9', 60, 1, 1, 1, 1, 'UP', 1, 30, 14, datetime('now', '+14 days'), NULL, datetime('now'))"
        )
        .bind(monitor_id)
        .execute(&pool)
        .await
        .unwrap();

        let client = reqwest::Client::new();
        let monitor = fetch_monitor_by_id(&pool, monitor_id).await.unwrap().unwrap();

        let prev = MonitorState {
            status: "UP".to_string(),
            consecutive_failures: 0,
            last_state_change: Utc::now(),
        };

        // First evaluation: 14 days remaining <= 30 warning threshold. Milestone 14 has not been alerted.
        let (_new_state, _alert) = run_check_and_evaluate(&pool, &client, &monitor, &prev).await;

        // DB should record last_domain_alert_days = 14
        let updated = fetch_monitor_by_id(&pool, monitor_id).await.unwrap().unwrap();
        assert_eq!(updated.last_domain_alert_days, Some(14));

        // Second evaluation with same monitor (milestone 14 already alerted):
        // Re-run evaluation: should NOT re-alert milestone 14 (suppressed)
        let (_new_state2, _alert2) = run_check_and_evaluate(&pool, &client, &updated, &prev).await;
        let updated2 = fetch_monitor_by_id(&pool, monitor_id).await.unwrap().unwrap();
        assert_eq!(updated2.last_domain_alert_days, Some(14));
    }
}
