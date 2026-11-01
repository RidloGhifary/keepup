pub mod admin;
pub mod auth;
pub mod dto;
pub mod logs;
pub mod maintenance;
pub mod monitors;
pub mod public;
pub mod status;
pub mod status_pages;
pub mod webhooks;

use std::sync::Arc;

use axum::routing::{get, post, put};
use axum::Router;

use crate::AppState;

/// Build the full API router, protecting administrative routes with auth middleware
/// while keeping public status and auth endpoints accessible.
pub fn router(state: Arc<AppState>) -> Router<Arc<AppState>> {
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
        .route("/badge/:id", get(public::badge));

    // ── Direct /api administrative routes (protected with require_auth) ──
    let api_direct_protected = Router::new()
        .route("/monitors", get(monitors::list).post(monitors::create))
        .route(
            "/monitors/:id",
            get(monitors::get_one)
                .put(admin::update_monitor)
                .delete(monitors::delete),
        )
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
            "/webhooks",
            get(webhooks::list_webhooks).post(webhooks::create_webhook),
        )
        .route(
            "/webhooks/:id",
            put(webhooks::update_webhook).delete(webhooks::delete_webhook),
        )
        .route("/webhooks/test", post(webhooks::test_webhook_direct))
        .route("/webhooks/:id/test", post(webhooks::test_webhook))
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
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            crate::middleware::auth::require_auth,
        ));

    // ── Admin endpoints under /api/v1 (protected with require_auth) ────
    let admin_v1_protected = Router::new()
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
            "/webhooks",
            get(webhooks::list_webhooks).post(webhooks::create_webhook),
        )
        .route(
            "/webhooks/:id",
            put(webhooks::update_webhook).delete(webhooks::delete_webhook),
        )
        .route("/webhooks/test", post(webhooks::test_webhook_direct))
        .route("/webhooks/:id/test", post(webhooks::test_webhook))
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
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            crate::middleware::auth::require_auth,
        ));

    // Assemble /api and /api/v1
    let api_combined = Router::new()
        .nest("/auth", auth_routes.clone())
        .nest("/public", public_api.clone())
        .route("/badge/:id", get(public::badge))
        .merge(api_direct_protected);

    let v1_combined = Router::new()
        .nest("/auth", auth_routes)
        .nest("/public", public_api)
        .route("/badge/:id", get(public::badge))
        .merge(admin_v1_protected);

    Router::new()
        .nest("/api", api_combined)
        .nest("/api/v1", v1_combined)
}
