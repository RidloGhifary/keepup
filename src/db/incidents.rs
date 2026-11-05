use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::api::public::{PublicIncident, PublicIncidentUpdate};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Incident {
    pub id: String,
    pub title: String,
    pub status: String,
    pub severity: String,
    pub monitor_id: Option<String>,
    pub status_page_slug: Option<String>,
    pub is_auto: i32,
    pub created_at: String,
    pub updated_at: String,
    pub resolved_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct IncidentUpdate {
    pub id: String,
    pub incident_id: String,
    pub status: String,
    pub message: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentWithUpdates {
    #[serde(flatten)]
    pub incident: Incident,
    pub updates: Vec<IncidentUpdate>,
}

#[derive(Debug, Deserialize)]
pub struct CreateIncidentDto {
    pub title: String,
    pub status: Option<String>,
    pub severity: Option<String>,
    pub monitor_id: Option<String>,
    pub status_page_slug: Option<String>,
    pub initial_message: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateIncidentUpdateDto {
    pub status: String,
    pub message: String,
}

/// Create a new incident, optionally writing an initial timeline update in an atomic transaction.
pub async fn create_incident(
    pool: &SqlitePool,
    dto: CreateIncidentDto,
    is_auto: bool,
) -> Result<IncidentWithUpdates, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let id = format!("inc_{}", &uuid::Uuid::new_v4().simple().to_string()[..16]);
    let now = Utc::now().to_rfc3339();
    let status = dto.status.unwrap_or_else(|| "Investigating".to_string());
    let severity = dto.severity.unwrap_or_else(|| "major".to_string());

    sqlx::query(
        r#"
        INSERT INTO incidents (
            id, title, status, severity, monitor_id, status_page_slug, is_auto,
            created_at, updated_at, resolved_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)
        "#,
    )
    .bind(&id)
    .bind(&dto.title)
    .bind(&status)
    .bind(&severity)
    .bind(&dto.monitor_id)
    .bind(&dto.status_page_slug)
    .bind(if is_auto { 1 } else { 0 })
    .bind(&now)
    .bind(&now)
    .execute(&mut *tx)
    .await?;

    let mut updates = Vec::new();
    if let Some(msg) = dto.initial_message {
        let update_id = format!("upd_{}", &uuid::Uuid::new_v4().simple().to_string()[..16]);
        sqlx::query(
            r#"
            INSERT INTO incident_updates (id, incident_id, status, message, created_at)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(&update_id)
        .bind(&id)
        .bind(&status)
        .bind(&msg)
        .bind(&now)
        .execute(&mut *tx)
        .await?;

        updates.push(IncidentUpdate {
            id: update_id,
            incident_id: id.clone(),
            status: status.clone(),
            message: msg,
            created_at: now.clone(),
        });
    }

    tx.commit().await?;

    let incident = Incident {
        id,
        title: dto.title,
        status,
        severity,
        monitor_id: dto.monitor_id,
        status_page_slug: dto.status_page_slug,
        is_auto: if is_auto { 1 } else { 0 },
        created_at: now.clone(),
        updated_at: now,
        resolved_at: None,
    };

    Ok(IncidentWithUpdates { incident, updates })
}

/// Add an update to an existing incident and sync the incident's top-level status and updated_at.
pub async fn add_incident_update(
    pool: &SqlitePool,
    incident_id: &str,
    dto: CreateIncidentUpdateDto,
) -> Result<IncidentUpdate, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let update_id = format!("upd_{}", &uuid::Uuid::new_v4().simple().to_string()[..16]);
    let now = Utc::now().to_rfc3339();
    let is_resolved = dto.status.eq_ignore_ascii_case("resolved");

    sqlx::query(
        r#"
        INSERT INTO incident_updates (id, incident_id, status, message, created_at)
        VALUES (?, ?, ?, ?, ?)
        "#,
    )
    .bind(&update_id)
    .bind(incident_id)
    .bind(&dto.status)
    .bind(&dto.message)
    .bind(&now)
    .execute(&mut *tx)
    .await?;

    if is_resolved {
        sqlx::query(
            r#"
            UPDATE incidents
            SET status = ?, updated_at = ?, resolved_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&dto.status)
        .bind(&now)
        .bind(&now)
        .bind(incident_id)
        .execute(&mut *tx)
        .await?;
    } else {
        sqlx::query(
            r#"
            UPDATE incidents
            SET status = ?, updated_at = ?, resolved_at = NULL
            WHERE id = ?
            "#,
        )
        .bind(&dto.status)
        .bind(&now)
        .bind(incident_id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(IncidentUpdate {
        id: update_id,
        incident_id: incident_id.to_string(),
        status: dto.status,
        message: dto.message,
        created_at: now,
    })
}

/// List incidents with all their updates sorted by created_at DESC.
pub async fn list_incidents(
    pool: &SqlitePool,
    monitor_id: Option<&str>,
    status_page_slug: Option<&str>,
) -> Result<Vec<IncidentWithUpdates>, sqlx::Error> {
    let mut qb = sqlx::QueryBuilder::new(
        r#"
        SELECT id, title, status, severity, monitor_id, status_page_slug, is_auto,
               created_at, updated_at, resolved_at
        FROM incidents
        WHERE 1 = 1
        "#,
    );

    if let Some(m_id) = monitor_id {
        qb.push(" AND monitor_id = ").push_bind(m_id);
    }
    if let Some(slug) = status_page_slug {
        qb.push(" AND status_page_slug = ").push_bind(slug);
    }
    qb.push(" ORDER BY created_at DESC LIMIT 100");

    let incidents: Vec<Incident> = qb.build_query_as().fetch_all(pool).await?;

    let mut result = Vec::with_capacity(incidents.len());
    for inc in incidents {
        let updates: Vec<IncidentUpdate> = sqlx::query_as(
            r#"
            SELECT id, incident_id, status, message, created_at
            FROM incident_updates
            WHERE incident_id = ?
            ORDER BY created_at ASC
            "#,
        )
        .bind(&inc.id)
        .fetch_all(pool)
        .await?;

        result.push(IncidentWithUpdates {
            incident: inc,
            updates,
        });
    }

    Ok(result)
}

/// Get a single incident with its updates.
#[allow(dead_code)]
pub async fn get_incident_by_id(
    pool: &SqlitePool,
    id: &str,
) -> Result<Option<IncidentWithUpdates>, sqlx::Error> {
    let inc: Option<Incident> = sqlx::query_as(
        r#"
        SELECT id, title, status, severity, monitor_id, status_page_slug, is_auto,
               created_at, updated_at, resolved_at
        FROM incidents
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    match inc {
        Some(incident) => {
            let updates: Vec<IncidentUpdate> = sqlx::query_as(
                r#"
                SELECT id, incident_id, status, message, created_at
                FROM incident_updates
                WHERE incident_id = ?
                ORDER BY created_at ASC
                "#,
            )
            .bind(&incident.id)
            .fetch_all(pool)
            .await?;

            Ok(Some(IncidentWithUpdates { incident, updates }))
        }
        None => Ok(None),
    }
}

/// Delete an incident and its updates (ON DELETE CASCADE in schema).
pub async fn delete_incident(pool: &SqlitePool, id: &str) -> Result<bool, sqlx::Error> {
    let res = sqlx::query("DELETE FROM incidents WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

/// Auto-create an ongoing incident when a monitor transitions UP -> DOWN if one is not already open.
pub async fn auto_create_incident_on_down(
    pool: &SqlitePool,
    monitor_id: &str,
    monitor_name: &str,
    error_msg: &str,
) -> Result<Option<String>, sqlx::Error> {
    // Check if there is an active auto-incident for this monitor
    let existing: Option<String> = sqlx::query_scalar(
        r#"
        SELECT id FROM incidents
        WHERE monitor_id = ? AND is_auto = 1 AND status != 'Resolved'
        LIMIT 1
        "#,
    )
    .bind(monitor_id)
    .fetch_optional(pool)
    .await?;

    if existing.is_some() {
        return Ok(None);
    }

    let dto = CreateIncidentDto {
        title: format!("Outage detected: {monitor_name}"),
        status: Some("Investigating".to_string()),
        severity: Some("major".to_string()),
        monitor_id: Some(monitor_id.to_string()),
        status_page_slug: None,
        initial_message: Some(format!(
            "Automated health check reported an outage: {}",
            error_msg.trim()
        )),
    };

    let inc = create_incident(pool, dto, true).await?;
    Ok(Some(inc.incident.id))
}

/// Auto-resolve open auto-incidents when a monitor transitions DOWN -> UP.
pub async fn auto_resolve_incident_on_up(
    pool: &SqlitePool,
    monitor_id: &str,
) -> Result<bool, sqlx::Error> {
    let open_incidents: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT id FROM incidents
        WHERE monitor_id = ? AND is_auto = 1 AND status != 'Resolved'
        "#,
    )
    .bind(monitor_id)
    .fetch_all(pool)
    .await?;

    if open_incidents.is_empty() {
        return Ok(false);
    }

    for inc_id in &open_incidents {
        let _ = add_incident_update(
            pool,
            inc_id,
            CreateIncidentUpdateDto {
                status: "Resolved".to_string(),
                message: "Service recovered: health checks are passing normally.".to_string(),
            },
        )
        .await;
    }

    Ok(true)
}

/// Fetch public incidents visible to a status page (incidents assigned to this page, or to monitors
/// visible on this page, or global incidents with no page/monitor restrictions).
pub async fn get_public_incidents_for_page(
    pool: &SqlitePool,
    slug: Option<&str>,
    monitor_ids: &[String],
) -> Result<Vec<PublicIncident>, sqlx::Error> {
    // Fetch incidents that are either:
    // 1. Not resolved, OR
    // 2. Resolved within the last 7 days
    let raw_incidents: Vec<Incident> = sqlx::query_as(
        r#"
        SELECT id, title, status, severity, monitor_id, status_page_slug, is_auto,
               created_at, updated_at, resolved_at
        FROM incidents
        WHERE status != 'Resolved'
           OR resolved_at >= datetime('now', '-7 days')
        ORDER BY created_at DESC
        LIMIT 50
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut result = Vec::new();
    for inc in raw_incidents {
        // Filter scope
        let in_scope = match (&inc.status_page_slug, &inc.monitor_id) {
            (Some(page_slug), _) => slug.map(|s| s.eq_ignore_ascii_case(page_slug)).unwrap_or(false),
            (None, Some(m_id)) => monitor_ids.contains(m_id),
            (None, None) => true, // Global incident
        };

        if !in_scope {
            continue;
        }

        let updates: Vec<IncidentUpdate> = sqlx::query_as(
            r#"
            SELECT id, incident_id, status, message, created_at
            FROM incident_updates
            WHERE incident_id = ?
            ORDER BY created_at ASC
            "#,
        )
        .bind(&inc.id)
        .fetch_all(pool)
        .await?;

        result.push(PublicIncident {
            id: inc.id,
            title: inc.title,
            status: inc.status,
            severity: inc.severity,
            monitor_id: inc.monitor_id,
            started_at: inc.created_at,
            resolved_at: inc.resolved_at,
            updates: updates
                .into_iter()
                .map(|u| PublicIncidentUpdate {
                    id: u.id,
                    incident_id: u.incident_id,
                    status: u.status,
                    message: u.message,
                    created_at: u.created_at,
                })
                .collect(),
        });
    }

    Ok(result)
}
