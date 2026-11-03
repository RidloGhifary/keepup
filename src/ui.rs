use std::sync::Arc;

use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Redirect, Response};
use axum_extra::extract::cookie::CookieJar;
use rust_embed::RustEmbed;

use crate::api::auth::{validate_jwt, SESSION_COOKIE_NAME};
use crate::AppState;

/// Embedded static directory containing dashboard UI assets.
#[derive(RustEmbed)]
#[folder = "static/"]
pub(crate) struct Assets;

/// Direct handler for public status pages (e.g. `/status` and `/status/*path`).
pub async fn status_page_handler(
    State(state): State<Arc<AppState>>,
    uri: Uri,
) -> Response {
    let raw_path = uri.path();
    let sub = raw_path.strip_prefix("/status/").unwrap_or("");
    if sub == "rss.xml" {
        return crate::api::observability::rss_feed_handler(State(state), None)
            .await
            .unwrap_or_else(|e| e.into_response());
    } else if let Some(slug) = sub.strip_suffix("/rss.xml") {
        return crate::api::observability::rss_feed_handler(
            State(state),
            Some(axum::extract::Path(slug.to_string())),
        )
        .await
        .unwrap_or_else(|e| e.into_response());
    }

    if !sub.is_empty() {
        // Disallow serving administrative HTML templates through public /status/* path
        if sub.ends_with(".html") && sub != "status.html" {
            return StatusCode::NOT_FOUND.into_response();
        }

        if let Some(file) = Assets::get(sub) {
            let mime_type = guess_mime(sub);
            return Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, HeaderValue::from_static(mime_type))
                .body(Body::from(file.data))
                .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response());
        }
    }

    match Assets::get("status.html") {
        Some(file) => Response::builder()
            .status(StatusCode::OK)
            .header(
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/html; charset=utf-8"),
            )
            .body(Body::from(file.data))
            .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response()),
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header(
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/plain; charset=utf-8"),
            )
            .body(Body::from("404 Not Found"))
            .unwrap_or_else(|_| StatusCode::NOT_FOUND.into_response()),
    }
}

/// Fallback and static file handler for Axum.
///
/// Handles root `/` (serving `index.html`), `/status` and `/status/*` (serving `status.html`),
/// `/modal` (serving `modal.html`), and any other embedded assets within `static/`.
pub async fn static_handler(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    uri: Uri,
) -> Response {
    let raw_path = uri.path();
    let path = raw_path.trim_start_matches('/');

    let target_file = if path == "status" || path == "status.html" {
        "status.html"
    } else if path.starts_with("status/") {
        let sub = path.strip_prefix("status/").unwrap_or("");
        if Assets::get(sub).is_some() {
            sub
        } else {
            "status.html"
        }
    } else {
        match path {
            "" | "index.html" => {
                // Require authentication for the admin dashboard
                let ver = state.current_session_version();
                let is_authenticated = jar
                    .get(SESSION_COOKIE_NAME)
                    .map(|c| validate_jwt(c.value(), &state.config.jwt_secret, ver).is_ok())
                    .unwrap_or(false);

                if !is_authenticated {
                    return Redirect::to("/login.html").into_response();
                }
                "index.html"
            }
            "modal" | "modal.html" => {
                // Require authentication for the admin modal view
                let ver = state.current_session_version();
                let is_authenticated = jar
                    .get(SESSION_COOKIE_NAME)
                    .map(|c| validate_jwt(c.value(), &state.config.jwt_secret, ver).is_ok())
                    .unwrap_or(false);

                if !is_authenticated {
                    return Redirect::to("/login.html").into_response();
                }
                "modal.html"
            }
            "login" | "login.html" => {
                // If already authenticated, redirect directly to dashboard
                let ver = state.current_session_version();
                let is_authenticated = jar
                    .get(SESSION_COOKIE_NAME)
                    .map(|c| validate_jwt(c.value(), &state.config.jwt_secret, ver).is_ok())
                    .unwrap_or(false);

                if is_authenticated {
                    return Redirect::to("/").into_response();
                }
                "login.html"
            }
            "favicon.ico" => "favicon.svg",
            other => other,
        }
    };

    match Assets::get(target_file) {
        Some(file) => {
            let mime_type = guess_mime(target_file);
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, HeaderValue::from_static(mime_type))
                .body(Body::from(file.data))
                .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
        }
        None => {
            // If asset not found, return a clean 404
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header(
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("text/plain; charset=utf-8"),
                )
                .body(Body::from("404 Not Found"))
                .unwrap_or_else(|_| StatusCode::NOT_FOUND.into_response())
        }
    }
}

/// Helper to determine the Content-Type header based on file extension.
fn guess_mime(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("");
    match ext {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "application/javascript; charset=utf-8",
        "json" => "application/json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;
    use sqlx::sqlite::SqlitePoolOptions;
    use tower::ServiceExt;

    async fn build_ui_app() -> (axum::Router, Arc<AppState>) {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let state = AppState::new_for_test(pool, crate::config::Config::from_env());
        let app = axum::Router::new()
            .route("/status", axum::routing::get(status_page_handler))
            .route("/status/*path", axum::routing::get(status_page_handler))
            .fallback(static_handler)
            .with_state(state.clone());
        (app, state)
    }

    #[tokio::test]
    async fn test_unauthenticated_modal_redirects_to_login() {
        let (app, state) = build_ui_app().await;
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/modal.html")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            res.status().is_redirection(),
            "expected redirect for unauthenticated /modal.html, got {}",
            res.status()
        );
        assert_eq!(
            res.headers()
                .get(header::LOCATION)
                .and_then(|v| v.to_str().ok()),
            Some("/login.html")
        );

        let token = crate::api::auth::create_jwt(
            &state.config.admin_user,
            &state.config.jwt_secret,
            state.current_session_version(),
        )
        .unwrap();
        let auth_res = app
            .oneshot(
                Request::builder()
                    .uri("/modal.html")
                    .header(
                        header::COOKIE,
                        format!("{}={}", SESSION_COOKIE_NAME, token),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(auth_res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_output_css_served_with_css_mime() {
        let (app, _state) = build_ui_app().await;
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/output.css")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok()),
            Some("text/css; charset=utf-8")
        );
    }

    #[tokio::test]
    async fn test_duplicate_docs_html_returns_404() {
        let (app, _state) = build_ui_app().await;
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/docs.html")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_status_path_does_not_leak_admin_html() {
        let (app, _state) = build_ui_app().await;
        for path in ["/status/index.html", "/status/modal.html", "/status/login.html"] {
            let res = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(path)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(
                res.status(),
                StatusCode::NOT_FOUND,
                "{path} must return 404 through public status route"
            );
        }
    }

    #[test]
    fn test_embedded_html_contains_no_cdn_scripts() {
        let html_files = ["index.html", "login.html", "modal.html", "status.html"];
        for name in html_files {
            let asset = Assets::get(name).unwrap_or_else(|| panic!("missing {name}"));
            let text = std::str::from_utf8(asset.data.as_ref()).unwrap();
            assert!(
                !text.contains("cdn.tailwindcss.com"),
                "{name} must not load tailwind from CDN (use /output.css)"
            );
        }
    }

    #[test]
    fn test_frontend_scripts_have_no_dynamic_onclick_interpolation() {
        let js_files = ["app.js", "modal.js", "status.js"];
        for name in js_files {
            let asset = Assets::get(name).unwrap_or_else(|| panic!("missing {name}"));
            let text = std::str::from_utf8(asset.data.as_ref()).unwrap();
            assert!(
                !text.contains("onclick=\"${escapeHtml("),
                "{name} must not interpolate dynamic values into inline onclick attributes (SEC-08)"
            );
            assert!(
                !text.contains(".replace(/'/g"),
                "{name} must not perform brittle single-quote escaping for inline onclick (SEC-08)"
            );
        }
    }

    #[test]
    fn test_status_page_contract_rss_incidents_no_ping() {
        let asset = Assets::get("status.html").expect("status.html must exist");
        let text = std::str::from_utf8(asset.data.as_ref()).unwrap();

        // Must NOT contain public Ping button (#4)
        assert!(
            !text.contains("triggerDetailPing") && !text.contains("detail-modal-ping-btn"),
            "status.html must not contain unauthenticated Ping button"
        );

        // Must contain incident timeline container and RSS feed link (N2, N3)
        assert!(text.contains("incidents"), "status.html must render incidents container");
        assert!(text.contains("rss.xml"), "status.html must link to RSS feed");
    }

    #[test]
    fn test_admin_app_js_uses_events_and_dashboard_batch() {
        let asset = Assets::get("app.js").expect("app.js must exist");
        let text = std::str::from_utf8(asset.data.as_ref()).unwrap();

        assert!(
            text.contains("/api/events"),
            "app.js must connect to live SSE via /api/events (F5)"
        );
        assert!(
            text.contains("/api/admin/dashboard"),
            "app.js must use batched /api/admin/dashboard endpoint (#8)"
        );
        assert!(
            !text.contains("/api/webhooks"),
            "app.js must use /api/admin/channels instead of legacy /api/webhooks (#12)"
        );
    }
}
