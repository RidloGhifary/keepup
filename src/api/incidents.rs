use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;

use crate::db::incidents::{self, CreateIncidentDto, CreateIncidentUpdateDto, IncidentWithUpdates, IncidentUpdate};
use crate::error::AppError;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct ListIncidentsQuery {
    pub monitor_id: Option<String>,
    pub status_page_slug: Option<String>,
}

/// `GET /api/admin/incidents` — list all incidents with updates.
pub async fn list_incidents(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListIncidentsQuery>,
) -> Result<Json<Vec<IncidentWithUpdates>>, AppError> {
    let items = incidents::list_incidents(
        &state.db,
        query.monitor_id.as_deref(),
        query.status_page_slug.as_deref(),
    )
    .await?;
    Ok(Json(items))
}

/// `POST /api/admin/incidents` — create a new incident.
pub async fn create_incident(
    State(state): State<Arc<AppState>>,
    Json(dto): Json<CreateIncidentDto>,
) -> Result<(StatusCode, Json<IncidentWithUpdates>), AppError> {
    let res = incidents::create_incident(&state.db, dto, false).await?;
    state.invalidate_status_cache().await;
    let _ = state.events_tx.send(
        serde_json::json!({
            "event": "incident_created",
            "incident_id": res.incident.id,
            "title": res.incident.title,
            "status": res.incident.status,
        })
        .to_string(),
    );
    Ok((StatusCode::CREATED, Json(res)))
}

/// `POST /api/admin/incidents/:id/updates` — append a new update to an incident.
pub async fn add_update(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(dto): Json<CreateIncidentUpdateDto>,
) -> Result<Json<IncidentUpdate>, AppError> {
    let update = incidents::add_incident_update(&state.db, &id, dto).await?;
    state.invalidate_status_cache().await;
    let _ = state.events_tx.send(
        serde_json::json!({
            "event": "incident_updated",
            "incident_id": id,
            "status": update.status,
            "message": update.message,
        })
        .to_string(),
    );
    Ok(Json(update))
}

/// `DELETE /api/admin/incidents/:id` — delete an incident.
pub async fn delete_incident(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let deleted = incidents::delete_incident(&state.db, &id).await?;
    if deleted {
        state.invalidate_status_cache().await;
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound("Incident not found".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_incident_lifecycle_and_public_timeline() {
        let pool = crate::db::init_pool(":memory:").await.unwrap();
        crate::db::run_migrations(&pool).await.unwrap();

        let config = crate::config::Config::default();
        let state = AppState::new_for_test(pool.clone(), config);
        let app = crate::build_http_app(state.clone());
        let token = crate::api::auth::create_jwt(
            "admin",
            "test-secret-at-least-32-bytes-long-for-security",
            state.current_session_version(),
        )
        .unwrap();

        // 1. Create a monitor
        sqlx::query(
            "INSERT INTO monitors (id, name, url, check_interval_secs, timeout_secs, max_retries, is_active, is_public, last_status) \
             VALUES ('mon-inc-1', 'API Gateway', 'http://127.0.0.1:8080', 60, 5, 1, 1, 1, 'UP')"
        )
        .execute(&pool)
        .await
        .unwrap();

        // 2. Create an incident via Admin API
        let create_req = Request::builder()
            .method("POST")
            .uri("/api/admin/incidents")
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_vec(&serde_json::json!({
                "title": "Elevated Error Rates",
                "status": "Investigating",
                "severity": "major",
                "monitor_id": "mon-inc-1",
                "initial_message": "Investigating HTTP 500 errors in EU-West."
            })).unwrap()))
            .unwrap();

        let res = app.clone().oneshot(create_req).await.unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);
        let body_bytes = axum::body::to_bytes(res.into_body(), 64 * 1024).await.unwrap();
        let created: IncidentWithUpdates = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(created.incident.title, "Elevated Error Rates");
        assert_eq!(created.updates.len(), 1);
        let inc_id = created.incident.id;

        // 3. Post an update: Identified
        let upd_req = Request::builder()
            .method("POST")
            .uri(format!("/api/admin/incidents/{inc_id}/updates"))
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_vec(&serde_json::json!({
                "status": "Identified",
                "message": "Root cause traced to bad configuration deployment."
            })).unwrap()))
            .unwrap();
        let res_upd = app.clone().oneshot(upd_req).await.unwrap();
        assert_eq!(res_upd.status(), StatusCode::OK);

        // 4. Verify public status displays the incident and both timeline updates
        let pub_req = Request::builder()
            .method("GET")
            .uri("/api/public/status")
            .body(Body::empty())
            .unwrap();
        let pub_res = app.clone().oneshot(pub_req).await.unwrap();
        assert_eq!(pub_res.status(), StatusCode::OK);
        let pub_bytes = axum::body::to_bytes(pub_res.into_body(), 64 * 1024).await.unwrap();
        let pub_status: crate::api::public::PublicStatus = serde_json::from_slice(&pub_bytes).unwrap();
        assert_eq!(pub_status.incidents.len(), 1);
        assert_eq!(pub_status.incidents[0].updates.len(), 2);
        assert_eq!(pub_status.incidents[0].status, "Identified");

        // 5. Post resolution update: Resolved
        let res_req = Request::builder()
            .method("POST")
            .uri(format!("/api/admin/incidents/{inc_id}/updates"))
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_vec(&serde_json::json!({
                "status": "Resolved",
                "message": "Config rolled back; service fully restored."
            })).unwrap()))
            .unwrap();
        let res_resolved = app.clone().oneshot(res_req).await.unwrap();
        assert_eq!(res_resolved.status(), StatusCode::OK);

        // 6. Delete incident
        let del_req = Request::builder()
            .method("DELETE")
            .uri(format!("/api/admin/incidents/{inc_id}"))
            .header("Authorization", format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap();
        let del_res = app.clone().oneshot(del_req).await.unwrap();
        assert_eq!(del_res.status(), StatusCode::NO_CONTENT);
    }
}
