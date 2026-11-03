pub mod admin;
pub mod auth;
pub mod dto;
pub mod incidents;
pub mod logs;
pub mod maintenance;
pub mod monitors;
pub mod observability;
pub mod public;
pub mod push;
pub mod status_pages;

use std::sync::Arc;

use axum::routing::{get, post, put};
use axum::Router;

use crate::AppState;

/// Build a single shared API sub-router that is mounted identically at `/api` and `/api/v1`.
fn build_v1_router(state: Arc<AppState>) -> Router<Arc<AppState>> {
    // ── Authentication endpoints (login + password change protected by IP rate limiter) ──
    let rate_limiter = crate::middleware::rate_limit::auth_rate_limiter_with_proxy(
        state.config.trust_proxy_headers,
    );

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

    let password_routes = Router::new()
        .route("/admin/change-password", post(admin::change_password))
        .route("/change-password", post(admin::change_password))
        .layer(crate::middleware::rate_limit::auth_rate_limiter_with_proxy(
            state.config.trust_proxy_headers,
        ));

    // ── Protected administrative routes ───────────────────────────────
    let protected = Router::new()
        .merge(password_routes)
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
        .route("/admin/dashboard", get(observability::dashboard_batch_handler))
        .route("/dashboard", get(observability::dashboard_batch_handler))
        .route("/admin/export", get(observability::export_handler))
        .route("/export", get(observability::export_handler))
        .route(
            "/admin/import",
            post(observability::import_handler)
                .layer(axum::extract::DefaultBodyLimit::max(10 * 1024 * 1024)),
        )
        .route(
            "/import",
            post(observability::import_handler)
                .layer(axum::extract::DefaultBodyLimit::max(10 * 1024 * 1024)),
        )
        .route("/admin/events", get(observability::events_sse_handler))
        .route("/events", get(observability::events_sse_handler))
        .route(
            "/admin/incidents",
            get(incidents::list_incidents).post(incidents::create_incident),
        )
        .route(
            "/incidents",
            get(incidents::list_incidents).post(incidents::create_incident),
        )
        .route(
            "/admin/incidents/:id/updates",
            post(incidents::add_update),
        )
        .route(
            "/incidents/:id/updates",
            post(incidents::add_update),
        )
        .route(
            "/admin/incidents/:id",
            axum::routing::delete(incidents::delete_incident),
        )
        .route(
            "/incidents/:id",
            axum::routing::delete(incidents::delete_incident),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            state,
            crate::middleware::auth::require_auth,
        ));

    Router::new()
        .nest("/auth", auth_routes)
        .nest("/public", public_api)
        .route("/push/:token", get(push::push_handler).post(push::push_handler))
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
