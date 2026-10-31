use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;

use crate::api::dto::{CreateStatusPageDto, UpdateStatusPageDto};
use crate::db::status_pages::{
    self, StatusPage, StatusPageDetail, StatusPageMonitorAssignment, StatusPageWithCount,
};
use crate::error::AppError;
use crate::AppState;

/// `GET /api/admin/status-pages` — List all status pages with monitor counts.
pub async fn list_status_pages(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<StatusPageWithCount>>, AppError> {
    let pages = status_pages::list_status_pages(&state.db).await?;
    Ok(Json(pages))
}

/// `GET /api/admin/status-pages/:slug` — Get a specific status page detail.
pub async fn get_status_page(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> Result<Json<StatusPageDetail>, AppError> {
    let page = status_pages::get_status_page(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Status page '{slug}' not found")))?;
    let monitors = status_pages::get_status_page_monitors(&state.db, &slug).await?;

    Ok(Json(StatusPageDetail {
        slug: page.slug,
        title: page.title,
        description: page.description,
        is_public: page.is_public == 1,
        created_at: page.created_at,
        monitors,
    }))
}

/// `POST /api/admin/status-pages` — Create a new status page.
pub async fn create_status_page(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateStatusPageDto>,
) -> Result<(StatusCode, Json<StatusPage>), AppError> {
    let monitor_assignments = payload.monitors.map(|list| {
        list.into_iter()
            .map(|m| StatusPageMonitorAssignment {
                monitor_id: m.monitor_id,
                group_name: m.group_name,
                display_order: m.display_order,
            })
            .collect()
    });

    let page = status_pages::create_status_page(
        &state.db,
        &payload.slug,
        &payload.title,
        payload.description.as_deref(),
        payload.is_public,
        monitor_assignments,
    )
    .await?;

    Ok((StatusCode::CREATED, Json(page)))
}

/// `PUT /api/admin/status-pages/:slug` — Update title, description, and monitor assignments.
pub async fn update_status_page(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
    Json(payload): Json<UpdateStatusPageDto>,
) -> Result<Json<StatusPageDetail>, AppError> {
    let monitor_assignments = payload.monitors.map(|list| {
        list.into_iter()
            .map(|m| StatusPageMonitorAssignment {
                monitor_id: m.monitor_id,
                group_name: m.group_name,
                display_order: m.display_order,
            })
            .collect()
    });

    let detail = status_pages::update_status_page(
        &state.db,
        &slug,
        &payload.title,
        payload.description.as_deref(),
        payload.is_public,
        monitor_assignments,
    )
    .await?;

    Ok(Json(detail))
}

/// `DELETE /api/admin/status-pages/:slug` — Delete a status page.
pub async fn delete_status_page(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> Result<StatusCode, AppError> {
    status_pages::delete_status_page(&state.db, &slug).await?;
    Ok(StatusCode::NO_CONTENT)
}
