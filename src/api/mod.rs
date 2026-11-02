pub mod admin;
pub mod auth;
pub mod dto;
pub mod logs;
pub mod maintenance;
pub mod monitors;
pub mod public;
pub mod status_pages;

use std::sync::Arc;

use axum::routing::{get, post, put};
use axum::Router;

use crate::AppState;

/// Build a single shared API sub-router that is mounted identically at `/api` and `/api/v1`.
fn build_v1_router(state: Arc<AppState>) -> Router<Arc<AppState>> {
    // ── Authentication endpoints (login protected by IP rate limiter) ──
    let rate_limiter = crate::middleware::rate_limit::auth_rate_limiter();

    let login_route = Router::new()
        .route("/login", post(auth::login))
        .layer(rate_limiter);

    let auth_routes = Router::new()
        .merge(login_route)
        .route("/logout", post(auth::logout))
        .route("/me", get(auth::me));

    // ── Public status endpoints (unprotected) ─────────────────────────
    let public_api = Router::new()
        .merge(public::public_routes())
        .route("/monitors/:id/history", get(logs::public_history))
        .route("/monitors/:id/details", get(admin::get_monitor_details))
        .route("/badge/:id", get(public::badge));

    // ── Protected administrative routes ───────────────────────────────
    let protected = Router::new()
        .route("/monitors", get(monitors::list).post(monitors::create))
        .route(
            "/monitors/:id",
            get(monitors::get_one)
                .put(admin::update_monitor)
                .delete(monitors::delete),
        )
        .route("/monitors/:id/logs", get(logs::by_monitor))
        .route(
            "/status-pages",
            get(status_pages::list_status_pages).post(status_pages::create_status_page),
        )
        .route(
            "/status-pages/:slug",
            get(status_pages::get_status_page)
                .put(status_pages::update_status_page)
                .delete(status_pages::delete_status_page),
        )
        .route(
            "/admin/status-pages",
            get(status_pages::list_status_pages).post(status_pages::create_status_page),
        )
        .route(
            "/admin/status-pages/:slug",
            get(status_pages::get_status_page)
                .put(status_pages::update_status_page)
                .delete(status_pages::delete_status_page),
        )
        .route(
            "/maintenance",
            get(maintenance::list_maintenance).post(maintenance::create_maintenance),
        )
        .route(
            "/maintenance/:id",
            get(maintenance::get_maintenance)
                .put(maintenance::update_maintenance)
                .delete(maintenance::delete_maintenance),
        )
        .route(
            "/admin/maintenance",
            get(maintenance::list_maintenance).post(maintenance::create_maintenance),
        )
        .route(
            "/admin/maintenance/:id",
            get(maintenance::get_maintenance)
                .put(maintenance::update_maintenance)
                .delete(maintenance::delete_maintenance),
        )
        .route("/admin/change-password", post(admin::change_password))
        .route("/change-password", post(admin::change_password))
        .route("/admin/monitors/:id/ping", post(admin::ping_monitor))
        .route("/monitors/:id/ping", post(admin::ping_monitor))
        .route("/admin/assets/expiry", get(admin::get_asset_expiry))
        .route("/assets/expiry", get(admin::get_asset_expiry))
        .route(
            "/admin/monitors/:id/refresh-assets",
            post(admin::refresh_monitor_assets),
        )
        .route(
            "/monitors/:id/refresh-assets",
            post(admin::refresh_monitor_assets),
        )
        .route(
            "/admin/channels",
            get(admin::list_channels).post(admin::create_channel),
        )
        .route(
            "/channels",
            get(admin::list_channels).post(admin::create_channel),
        )
        .route(
            "/webhooks",
            get(admin::list_channels).post(admin::create_channel),
        )
        .route(
            "/admin/channels/:id",
            put(admin::update_channel).delete(admin::delete_channel),
        )
        .route(
            "/channels/:id",
            put(admin::update_channel).delete(admin::delete_channel),
        )
        .route(
            "/webhooks/:id",
            put(admin::update_channel).delete(admin::delete_channel),
        )
        .route("/admin/channels/:id/test", post(admin::test_channel))
        .route("/channels/:id/test", post(admin::test_channel))
        .route("/webhooks/:id/test", post(admin::test_channel))
        .route(
            "/admin/monitors/:id/channels",
            get(admin::get_monitor_channels),
        )
        .route("/monitors/:id/channels", get(admin::get_monitor_channels))
        .route_layer(axum::middleware::from_fn_with_state(
            state,
            crate::middleware::auth::require_auth,
        ));

    Router::new()
        .nest("/auth", auth_routes)
        .nest("/public", public_api)
        .route("/badge/:id", get(public::badge))
        .route("/admin/monitors/:id/details", get(admin::get_monitor_details))
        .route("/monitors/:id/details", get(admin::get_monitor_details))
        .merge(protected)
}

/// Build the full API router mounted at `/api` and `/api/v1`.
pub fn router(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .nest("/api", build_v1_router(state.clone()))
        .nest("/api/v1", build_v1_router(state))
}
