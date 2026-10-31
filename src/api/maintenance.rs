use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;

use crate::db::maintenance::{
    self, CreateMaintenanceDto, MaintenanceWindowWithMonitors, UpdateMaintenanceDto,
};
use crate::error::AppError;
use crate::AppState;

/// `GET /api/admin/maintenance` or `GET /api/maintenance`
///
/// Returns list of all active, upcoming, and past planned maintenance windows.
pub async fn list_maintenance(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<MaintenanceWindowWithMonitors>>, AppError> {
    let windows = maintenance::list_maintenance_windows(&state.db).await?;
    Ok(Json(windows))
}

/// `GET /api/admin/maintenance/:id` or `GET /api/maintenance/:id`
pub async fn get_maintenance(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<MaintenanceWindowWithMonitors>, AppError> {
    let window = maintenance::get_maintenance_window(&state.db, &id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Maintenance window '{id}' not found")))?;
    Ok(Json(window))
}

/// `POST /api/admin/maintenance` or `POST /api/maintenance`
///
/// Schedules a new planned maintenance window and links target monitors.
pub async fn create_maintenance(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateMaintenanceDto>,
) -> Result<(StatusCode, Json<MaintenanceWindowWithMonitors>), AppError> {
    let created = maintenance::create_maintenance_window(&state.db, payload).await?;
    Ok((StatusCode::CREATED, Json(created)))
}

/// `PUT /api/admin/maintenance/:id` or `PUT /api/maintenance/:id`
///
/// Updates an existing planned maintenance window.
pub async fn update_maintenance(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateMaintenanceDto>,
) -> Result<Json<MaintenanceWindowWithMonitors>, AppError> {
    let updated = maintenance::update_maintenance_window(&state.db, &id, payload).await?;
    Ok(Json(updated))
}

/// `DELETE /api/admin/maintenance/:id` or `DELETE /api/maintenance/:id`
///
/// Cancels and deletes a scheduled maintenance window.
pub async fn delete_maintenance(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    maintenance::delete_maintenance_window(&state.db, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use sqlx::sqlite::SqlitePoolOptions;
    use tower::ServiceExt;

    async fn setup_app() -> (axum::Router, Arc<AppState>) {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("Failed to run migrations");

        // Seed monitor
        sqlx::query("INSERT INTO monitors (id, name, url) VALUES ('mon-api-1', 'API Service', 'https://api.test')")
            .execute(&pool).await.unwrap();

        let config = crate::config::Config::from_env();
        let http_client = reqwest::Client::new();
        let (reload_tx, _) = tokio::sync::broadcast::channel(16);

        let state = Arc::new(AppState {
            db: pool,
            config,
            http_client,
            reload_tx,
        });

        let app = crate::api::router(state.clone()).with_state(state.clone());
        (app, state)
    }

    #[tokio::test]
    async fn test_admin_maintenance_api_endpoints() {
        let (app, state) = setup_app().await;

        let token = crate::api::auth::create_jwt(
            &state.config.admin_user,
            &state.config.jwt_secret,
        )
        .unwrap();

        let now = Utc::now();
        let start = (now + Duration::minutes(10)).to_rfc3339();
        let end = (now + Duration::hours(2)).to_rfc3339();

        // 1. POST /api/admin/maintenance (Create)
        let create_body = serde_json::json!({
            "title": "Scheduled Redis Cluster Upgrade",
            "description": "Failover testing and engine upgrade to v7.2",
            "start_time": start,
            "end_time": end,
            "monitor_ids": ["mon-api-1"]
        });

        let req = axum::http::Request::builder()
            .method("POST")
            .uri("/api/admin/maintenance")
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {token}"))
            .body(axum::body::Body::from(create_body.to_string()))
            .unwrap();

        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);
        let bytes = axum::body::to_bytes(res.into_body(), 1024 * 64).await.unwrap();
        let created: MaintenanceWindowWithMonitors = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(created.title, "Scheduled Redis Cluster Upgrade");
        assert_eq!(created.monitor_ids, vec!["mon-api-1"]);

        let maint_id = created.id;

        // 2. GET /api/admin/maintenance (List)
        let req = axum::http::Request::builder()
            .method("GET")
            .uri("/api/admin/maintenance")
            .header("Authorization", format!("Bearer {token}"))
            .body(axum::body::Body::empty())
            .unwrap();

        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(res.into_body(), 1024 * 64).await.unwrap();
        let list: Vec<MaintenanceWindowWithMonitors> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, maint_id);

        // 3. PUT /api/admin/maintenance/:id (Update)
        let update_body = serde_json::json!({
            "title": "Scheduled Redis Cluster Upgrade (Extended)"
        });

        let req = axum::http::Request::builder()
            .method("PUT")
            .uri(format!("/api/admin/maintenance/{maint_id}"))
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {token}"))
            .body(axum::body::Body::from(update_body.to_string()))
            .unwrap();

        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(res.into_body(), 1024 * 64).await.unwrap();
        let updated: MaintenanceWindowWithMonitors = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(updated.title, "Scheduled Redis Cluster Upgrade (Extended)");

        // 4. DELETE /api/admin/maintenance/:id (Delete)
        let req = axum::http::Request::builder()
            .method("DELETE")
            .uri(format!("/api/admin/maintenance/{maint_id}"))
            .header("Authorization", format!("Bearer {token}"))
            .body(axum::body::Body::empty())
            .unwrap();

        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::NO_CONTENT);

        // Verify it is gone
        let req = axum::http::Request::builder()
            .method("GET")
            .uri(format!("/api/admin/maintenance/{maint_id}"))
            .header("Authorization", format!("Bearer {token}"))
            .body(axum::body::Body::empty())
            .unwrap();

        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }
}
