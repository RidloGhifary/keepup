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
struct Assets;

/// Fallback and static file handler for Axum.
///
/// Handles root `/` (serving `index.html`), `/status` (serving `status.html`),
/// `/modal` (serving `modal.html`), and any other embedded assets within `static/`.
pub async fn static_handler(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    uri: Uri,
) -> Response {
    let raw_path = uri.path();
    let path = raw_path.trim_start_matches('/');

    let target_file = match path {
        "" | "index.html" => {
            // Require authentication for the admin dashboard
            let is_authenticated = jar
                .get(SESSION_COOKIE_NAME)
                .map(|c| validate_jwt(c.value(), &state.config.jwt_secret).is_ok())
                .unwrap_or(false);

            if !is_authenticated {
                return Redirect::to("/login.html").into_response();
            }
            "index.html"
        }
        "login" | "login.html" => {
            // If already authenticated, redirect directly to dashboard
            let is_authenticated = jar
                .get(SESSION_COOKIE_NAME)
                .map(|c| validate_jwt(c.value(), &state.config.jwt_secret).is_ok())
                .unwrap_or(false);

            if is_authenticated {
                return Redirect::to("/").into_response();
            }
            "login.html"
        }
        "status" | "status.html" => "status.html",
        "modal" | "modal.html" => "modal.html",
        "favicon.ico" => "favicon.svg",
        other => other,
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
