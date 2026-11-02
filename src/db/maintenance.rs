use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::error::AppError;

/// Maintenance window row directly mapped from SQLite.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MaintenanceWindowRow {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub start_time: String,
    pub end_time: String,
    pub created_at: String,
}

/// Full maintenance window model including assigned target monitor IDs and dynamic active status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceWindowWithMonitors {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub start_time: String,
    pub end_time: String,
    pub created_at: String,
    pub is_active: bool,
    pub monitor_ids: Vec<String>,
}

/// Public representation of maintenance window exposed to status page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicMaintenanceWindow {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub start_time: String,
    pub end_time: String,
    pub is_active: bool,
    pub monitor_ids: Vec<String>,
}

/// DTO for creating a new planned maintenance window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMaintenanceDto {
    pub title: String,
    pub description: Option<String>,
    pub start_time: String,
    pub end_time: String,
    #[serde(default)]
    pub monitor_ids: Vec<String>,
}

/// DTO for updating an existing planned maintenance window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMaintenanceDto {
    pub title: Option<String>,
    pub description: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub monitor_ids: Option<Vec<String>>,
}

/// Normalizes datetime string representations (RFC 3339, ISO 8601, HTML datetime-local)
/// into a standardized UTC DateTime.
pub fn parse_datetime_str(input: &str) -> Result<DateTime<Utc>, AppError> {
    let s = input.trim();
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(dt.with_timezone(&Utc));
    }

    // Try HTML datetime-local: YYYY-MM-DDTHH:MM or YYYY-MM-DDTHH:MM:SS
    if let Ok(naive) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
        return Ok(DateTime::from_naive_utc_and_offset(naive, Utc));
    }
    if let Ok(naive) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M") {
        return Ok(DateTime::from_naive_utc_and_offset(naive, Utc));
    }

    // Try SQL timestamp formats: YYYY-MM-DD HH:MM:SS or YYYY-MM-DD HH:MM
    if let Ok(naive) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Ok(DateTime::from_naive_utc_and_offset(naive, Utc));
    }
    if let Ok(naive) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M") {
        return Ok(DateTime::from_naive_utc_and_offset(naive, Utc));
    }

    Err(AppError::BadRequest(format!(
        "Invalid datetime format: '{input}'. Expected ISO 8601 or YYYY-MM-DDTHH:MM"
    )))
}

/// Checks whether a specific monitor is currently undergoing active planned maintenance.
pub async fn is_monitor_in_maintenance(
    pool: &SqlitePool,
    monitor_id: &str,
) -> Result<bool, sqlx::Error> {
    let row = sqlx::query_scalar::<_, i32>(
        r#"
        SELECT 1
        FROM maintenance_windows mw
        JOIN maintenance_monitors mm ON mw.id = mm.maintenance_id
        WHERE (mm.monitor_id = ? OR mm.monitor_id = '*')
          AND datetime('now') >= datetime(mw.start_time)
          AND datetime('now') <= datetime(mw.end_time)
        LIMIT 1
        "#,
    )
    .bind(monitor_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.is_some())
}

/// Fetch list of all maintenance windows (active, upcoming, past) with assigned monitors.
pub async fn list_maintenance_windows(
    pool: &SqlitePool,
) -> Result<Vec<MaintenanceWindowWithMonitors>, AppError> {
    let windows = sqlx::query_as::<_, MaintenanceWindowRow>(
        r#"
        SELECT id, title, description, start_time, end_time, created_at
        FROM maintenance_windows
        ORDER BY start_time DESC
        "#,
    )
    .fetch_all(pool)
    .await?;

    let now = Utc::now();
    let mut results = Vec::with_capacity(windows.len());

    for w in windows {
        let monitor_ids = sqlx::query_scalar::<_, String>(
            "SELECT monitor_id FROM maintenance_monitors WHERE maintenance_id = ? ORDER BY monitor_id ASC",
        )
        .bind(&w.id)
        .fetch_all(pool)
        .await?;

        let is_active = match (parse_datetime_str(&w.start_time), parse_datetime_str(&w.end_time)) {
            (Ok(start), Ok(end)) => now >= start && now <= end,
            _ => false,
        };

        results.push(MaintenanceWindowWithMonitors {
            id: w.id,
            title: w.title,
            description: w.description,
            start_time: w.start_time,
            end_time: w.end_time,
            created_at: w.created_at,
            is_active,
            monitor_ids,
        });
    }

    Ok(results)
}

/// Fetch a single maintenance window by ID.
pub async fn get_maintenance_window(
    pool: &SqlitePool,
    id: &str,
) -> Result<Option<MaintenanceWindowWithMonitors>, AppError> {
    let window = sqlx::query_as::<_, MaintenanceWindowRow>(
        r#"
        SELECT id, title, description, start_time, end_time, created_at
        FROM maintenance_windows
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    let Some(w) = window else {
        return Ok(None);
    };

    let monitor_ids = sqlx::query_scalar::<_, String>(
        "SELECT monitor_id FROM maintenance_monitors WHERE maintenance_id = ? ORDER BY monitor_id ASC",
    )
    .bind(&w.id)
    .fetch_all(pool)
    .await?;

    let now = Utc::now();
    let is_active = match (parse_datetime_str(&w.start_time), parse_datetime_str(&w.end_time)) {
        (Ok(start), Ok(end)) => now >= start && now <= end,
        _ => false,
    };

    Ok(Some(MaintenanceWindowWithMonitors {
        id: w.id,
        title: w.title,
        description: w.description,
        start_time: w.start_time,
        end_time: w.end_time,
        created_at: w.created_at,
        is_active,
        monitor_ids,
    }))
}

/// Create a new maintenance window.
pub async fn create_maintenance_window(
    pool: &SqlitePool,
    payload: CreateMaintenanceDto,
) -> Result<MaintenanceWindowWithMonitors, AppError> {
    if payload.title.trim().is_empty() {
        return Err(AppError::BadRequest("Title cannot be empty".to_string()));
    }

    let start_dt = parse_datetime_str(&payload.start_time)?;
    let end_dt = parse_datetime_str(&payload.end_time)?;

    if end_dt <= start_dt {
        return Err(AppError::BadRequest(
            "end_time must be strictly after start_time".to_string(),
        ));
    }

    let id = Uuid::new_v4().to_string();
    let start_str = start_dt.to_rfc3339();
    let end_str = end_dt.to_rfc3339();

    sqlx::query(
        r#"
        INSERT INTO maintenance_windows (id, title, description, start_time, end_time, created_at)
        VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
        "#,
    )
    .bind(&id)
    .bind(payload.title.trim())
    .bind(payload.description.as_deref().map(str::trim))
    .bind(&start_str)
    .bind(&end_str)
    .execute(pool)
    .await?;

    for monitor_id in &payload.monitor_ids {
        sqlx::query(
            "INSERT OR IGNORE INTO maintenance_monitors (maintenance_id, monitor_id) VALUES (?, ?)",
        )
        .bind(&id)
        .bind(monitor_id)
        .execute(pool)
        .await?;
    }

    let now = Utc::now();
    let is_active = now >= start_dt && now <= end_dt;

    Ok(MaintenanceWindowWithMonitors {
        id,
        title: payload.title.trim().to_string(),
        description: payload.description.map(|s| s.trim().to_string()),
        start_time: start_str,
        end_time: end_str,
        created_at: Utc::now().to_rfc3339(),
        is_active,
        monitor_ids: payload.monitor_ids,
    })
}

/// Update an existing maintenance window.
pub async fn update_maintenance_window(
    pool: &SqlitePool,
    id: &str,
    payload: UpdateMaintenanceDto,
) -> Result<MaintenanceWindowWithMonitors, AppError> {
    let existing = get_maintenance_window(pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Maintenance window '{id}' not found")))?;

    let title = payload
        .title
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .unwrap_or(existing.title);

    let description = if payload.description.is_some() {
        payload.description.map(|d| d.trim().to_string())
    } else {
        existing.description
    };

    let start_dt = if let Some(ref st) = payload.start_time {
        parse_datetime_str(st)?
    } else {
        parse_datetime_str(&existing.start_time)?
    };

    let end_dt = if let Some(ref et) = payload.end_time {
        parse_datetime_str(et)?
    } else {
        parse_datetime_str(&existing.end_time)?
    };

    if end_dt <= start_dt {
        return Err(AppError::BadRequest(
            "end_time must be strictly after start_time".to_string(),
        ));
    }

    let start_str = start_dt.to_rfc3339();
    let end_str = end_dt.to_rfc3339();

    let mut tx = pool.begin().await?;

    sqlx::query(
        r#"
        UPDATE maintenance_windows
        SET title = ?, description = ?, start_time = ?, end_time = ?
        WHERE id = ?
        "#,
    )
    .bind(&title)
    .bind(&description)
    .bind(&start_str)
    .bind(&end_str)
    .bind(id)
    .execute(&mut *tx)
    .await?;

    let monitor_ids = if let Some(new_monitors) = payload.monitor_ids {
        sqlx::query("DELETE FROM maintenance_monitors WHERE maintenance_id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;

        for monitor_id in &new_monitors {
            sqlx::query(
                "INSERT OR IGNORE INTO maintenance_monitors (maintenance_id, monitor_id) VALUES (?, ?)",
            )
            .bind(id)
            .bind(monitor_id)
            .execute(&mut *tx)
            .await?;
        }
        new_monitors
    } else {
        existing.monitor_ids
    };

    tx.commit().await?;

    let now = Utc::now();
    let is_active = now >= start_dt && now <= end_dt;

    Ok(MaintenanceWindowWithMonitors {
        id: id.to_string(),
        title,
        description,
        start_time: start_str,
        end_time: end_str,
        created_at: existing.created_at,
        is_active,
        monitor_ids,
    })
}

/// Delete a maintenance window by ID.
pub async fn delete_maintenance_window(pool: &SqlitePool, id: &str) -> Result<bool, AppError> {
    let rows_affected = sqlx::query("DELETE FROM maintenance_windows WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected();

    if rows_affected == 0 {
        return Err(AppError::NotFound(format!("Maintenance window '{id}' not found")));
    }

    Ok(true)
}

/// Fetch active and upcoming maintenance windows (start_time <= NOW() + 7 days AND end_time >= NOW())
/// for public status pages.
pub async fn get_public_maintenance_windows(
    pool: &SqlitePool,
) -> Result<Vec<PublicMaintenanceWindow>, AppError> {
    let windows = sqlx::query_as::<_, MaintenanceWindowRow>(
        r#"
        SELECT id, title, description, start_time, end_time, created_at
        FROM maintenance_windows
        WHERE datetime(start_time) <= datetime('now', '+7 days')
          AND datetime(end_time) >= datetime('now')
        ORDER BY datetime(start_time) ASC
        "#,
    )
    .fetch_all(pool)
    .await?;

    let now = Utc::now();
    let mut results = Vec::with_capacity(windows.len());

    for w in windows {
        let monitor_ids = sqlx::query_scalar::<_, String>(
            "SELECT monitor_id FROM maintenance_monitors WHERE maintenance_id = ? ORDER BY monitor_id ASC",
        )
        .bind(&w.id)
        .fetch_all(pool)
        .await?;

        let is_active = match (parse_datetime_str(&w.start_time), parse_datetime_str(&w.end_time)) {
            (Ok(start), Ok(end)) => now >= start && now <= end,
            _ => false,
        };

        results.push(PublicMaintenanceWindow {
            id: w.id,
            title: w.title,
            description: w.description,
            start_time: w.start_time,
            end_time: w.end_time,
            is_active,
            monitor_ids,
        });
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn setup_test_db() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("Failed to run migrations");

        // Seed a test monitor
        sqlx::query(
            "INSERT INTO monitors (id, name, url, check_interval_secs, timeout_secs, max_retries, is_active, is_public) \
             VALUES ('mon-1', 'API Gateway', 'https://api.example.com', 60, 5, 3, 1, 1)",
        )
        .execute(&pool)
        .await
        .unwrap();

        pool
    }

    #[tokio::test]
    async fn test_maintenance_crud_lifecycle() {
        let pool = setup_test_db().await;

        let now = Utc::now();
        let start = now - Duration::minutes(10);
        let end = now + Duration::minutes(30);

        // 1. Create
        let created = create_maintenance_window(
            &pool,
            CreateMaintenanceDto {
                title: "Planned Database Migration".to_string(),
                description: Some("Upgrading to Postgres 16".to_string()),
                start_time: start.to_rfc3339(),
                end_time: end.to_rfc3339(),
                monitor_ids: vec!["mon-1".to_string()],
            },
        )
        .await
        .unwrap();

        assert_eq!(created.title, "Planned Database Migration");
        assert!(created.is_active);
        assert_eq!(created.monitor_ids, vec!["mon-1"]);

        // 2. Query is_monitor_in_maintenance
        let in_maint = is_monitor_in_maintenance(&pool, "mon-1").await.unwrap();
        assert!(in_maint, "mon-1 should be recognized as in maintenance");

        let other_in_maint = is_monitor_in_maintenance(&pool, "mon-unknown").await.unwrap();
        assert!(!other_in_maint);

        // 3. Update
        let updated = update_maintenance_window(
            &pool,
            &created.id,
            UpdateMaintenanceDto {
                title: Some("DB Upgrade (Extended)".to_string()),
                description: None,
                start_time: None,
                end_time: None,
                monitor_ids: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(updated.title, "DB Upgrade (Extended)");
        assert_eq!(updated.description, Some("Upgrading to Postgres 16".to_string()));

        // 4. Public list
        let public_windows = get_public_maintenance_windows(&pool).await.unwrap();
        assert_eq!(public_windows.len(), 1);
        assert!(public_windows[0].is_active);

        // 5. Delete
        let deleted = delete_maintenance_window(&pool, &created.id).await.unwrap();
        assert!(deleted);

        let after_delete_in_maint = is_monitor_in_maintenance(&pool, "mon-1").await.unwrap();
        assert!(!after_delete_in_maint);
    }

    #[test]
    fn test_parse_datetime_variations() {
        assert!(parse_datetime_str("2026-10-31T14:30:00Z").is_ok());
        assert!(parse_datetime_str("2026-10-31T14:30").is_ok());
        assert!(parse_datetime_str("2026-10-31 14:30:00").is_ok());
        assert!(parse_datetime_str("invalid").is_err());
    }

    #[tokio::test]
    async fn test_overnight_maintenance_window_not_active_after_end_and_future_7d_cap() {
        use chrono::Timelike;
        let pool = setup_test_db().await;

        let now = Utc::now();
        let today = now.date_naive();
        // Window ended earlier on the same calendar day (or if running between 00:00..02:00 UTC, test +20d cap & yesterday->today 00:00:01Z)
        let (overnight_start, overnight_end) = if now.hour() >= 2 {
            let start = (today - Duration::days(1))
                .and_hms_opt(23, 0, 0)
                .unwrap()
                .and_utc();
            let end = today.and_hms_opt(1, 0, 0).unwrap().and_utc();
            (start, end)
        } else {
            let start = (today - Duration::days(1))
                .and_hms_opt(22, 0, 0)
                .unwrap()
                .and_utc();
            let end = (now - Duration::seconds(5)).max(start + Duration::minutes(1));
            (start, end)
        };

        create_maintenance_window(
            &pool,
            CreateMaintenanceDto {
                title: "Overnight Window".to_string(),
                description: None,
                start_time: overnight_start.to_rfc3339(),
                end_time: overnight_end.to_rfc3339(),
                monitor_ids: vec!["mon-1".to_string()],
            },
        )
        .await
        .unwrap();

        let active = is_monitor_in_maintenance(&pool, "mon-1").await.unwrap();
        assert!(
            !active,
            "overnight maintenance window ({overnight_start} -> {overnight_end}) must NOT be active at {now}"
        );

        // Window starting 15 days in the future must NOT appear in get_public_maintenance_windows
        create_maintenance_window(
            &pool,
            CreateMaintenanceDto {
                title: "Far Future Maintenance".to_string(),
                description: None,
                start_time: (now + Duration::days(15)).to_rfc3339(),
                end_time: (now + Duration::days(15) + Duration::hours(2)).to_rfc3339(),
                monitor_ids: vec!["mon-1".to_string()],
            },
        )
        .await
        .unwrap();

        let pub_windows = get_public_maintenance_windows(&pool).await.unwrap();
        assert!(
            pub_windows.is_empty(),
            "past overnight window and +15d future window must both be excluded from public windows: got {:?}",
            pub_windows
        );
    }
}
