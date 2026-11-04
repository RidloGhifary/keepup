use std::sync::Arc;

use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderName, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Redirect, Response};
use axum_extra::extract::cookie::CookieJar;
use rust_embed::RustEmbed;

use crate::api::auth::{validate_jwt, SESSION_COOKIE_NAME};
use crate::AppState;

/// Embedded static directory containing dashboard UI assets.
#[derive(RustEmbed)]
#[folder = "$STATIC_DIR"]
struct Assets;

/// Helper to escape HTML characters in injected tag attributes.
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Helper to determine the Cache-Control policy based on file extension.
fn cache_control_for(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("");
    match ext {
        "html" | "htm" => "no-cache, must-revalidate",
        "css" | "js" | "png" | "jpg" | "jpeg" | "svg" | "ico" => {
            "public, max-age=86400, stale-while-revalidate=3600"
        }
        _ => "public, max-age=3600",
    }
}

/// Direct handler for public status pages (e.g. `/status` and `/status/*path`).
pub async fn status_page_handler(State(state): State<Arc<AppState>>, uri: Uri) -> Response {
    let raw_path = uri.path();
    let sub = raw_path.strip_prefix("/status/").unwrap_or("");
    if !sub.is_empty() {
        if let Some(file) = Assets::get(sub) {
            let mime_type = guess_mime(sub);
            return Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, HeaderValue::from_static(mime_type))
                .header(
                    header::CACHE_CONTROL,
                    HeaderValue::from_static(cache_control_for(sub)),
                )
                .body(Body::from(file.data))
                .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response());
        }
    }

    let slug = if raw_path == "/status" || raw_path == "/status/" {
        "default"
    } else {
        let trimmed = raw_path.strip_prefix("/status/").unwrap_or("default");
        let s = trimmed.split('/').next().unwrap_or("default");
        if s.is_empty() {
            "default"
        } else {
            s
        }
    };

    let page_opt = crate::db::status_pages::get_status_page(&state.db, slug)
        .await
        .unwrap_or(None);

    if slug != "default" {
        if let Some(ref page) = page_opt {
            if page.is_public == 0 {
                return Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .header(header::CONTENT_TYPE, HeaderValue::from_static("text/plain; charset=utf-8"))
                    .body(Body::from("404 Not Found"))
                    .unwrap_or_else(|_| StatusCode::NOT_FOUND.into_response());
            }
        } else {
            return Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header(header::CONTENT_TYPE, HeaderValue::from_static("text/plain; charset=utf-8"))
                .body(Body::from("404 Not Found"))
                .unwrap_or_else(|_| StatusCode::NOT_FOUND.into_response());
        }
    }

    let allow_indexing = page_opt.as_ref().map(|p| p.allow_indexing).unwrap_or(true);
    let title = page_opt
        .as_ref()
        .and_then(|p| p.meta_title.clone())
        .or_else(|| page_opt.as_ref().map(|p| p.title.clone()))
        .unwrap_or_else(|| "KeepUp — Status & Dashboard".to_string());
    let desc = page_opt
        .as_ref()
        .and_then(|p| p.meta_description.clone())
        .or_else(|| page_opt.as_ref().and_then(|p| p.description.clone()))
        .unwrap_or_else(|| "Real-time system status and service health telemetry powered by KeepUp.".to_string());
    let og_image = page_opt
        .as_ref()
        .and_then(|p| p.og_image_url.clone())
        .unwrap_or_default();
    let canonical = page_opt
        .as_ref()
        .and_then(|p| p.canonical_url.clone())
        .unwrap_or_default();
    let robots_directive = if allow_indexing {
        "index, follow"
    } else {
        "noindex, nofollow"
    };

    match Assets::get("status.html") {
        Some(file) => {
            let mut html = String::from_utf8_lossy(&file.data).to_string();

            if let Some(pos) = html.find("<title>") {
                if let Some(end_pos) = html[pos..].find("</title>") {
                    html.replace_range(
                        pos..pos + end_pos + 8,
                        &format!("<title>{}</title>", escape_html(&title)),
                    );
                }
            }

            let mut seo_tags = format!(
                r#"
  <meta name="description" content="{}">
  <meta name="robots" content="{}">
  <meta property="og:type" content="website">
  <meta property="og:title" content="{}">
  <meta property="og:description" content="{}">
  <meta name="twitter:card" content="summary_large_image">
  <meta name="twitter:title" content="{}">
  <meta name="twitter:description" content="{}">"#,
                escape_html(&desc),
                robots_directive,
                escape_html(&title),
                escape_html(&desc),
                escape_html(&title),
                escape_html(&desc)
            );

            if !og_image.is_empty() {
                seo_tags.push_str(&format!(
                    r#"
  <meta property="og:image" content="{}">
  <meta name="twitter:image" content="{}">"#,
                    escape_html(&og_image),
                    escape_html(&og_image)
                ));
            }

            if !canonical.is_empty() {
                seo_tags.push_str(&format!(
                    r#"
  <link rel="canonical" href="{}">"#,
                    escape_html(&canonical)
                ));
            }

            if let Some(head_idx) = html.find("</head>") {
                html.insert_str(head_idx, &seo_tags);
            }

            let mut builder = Response::builder()
                .status(StatusCode::OK)
                .header(
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("text/html; charset=utf-8"),
                )
                .header(
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("no-cache, must-revalidate"),
                )
                .header(
                    HeaderName::from_static("x-robots-tag"),
                    HeaderValue::from_static(robots_directive),
                );

            if !canonical.is_empty() {
                if let Ok(canon_val) = HeaderValue::from_str(&format!("<{canonical}>; rel=\"canonical\"")) {
                    builder = builder.header(header::LINK, canon_val);
                }
            }

            builder
                .body(Body::from(html))
                .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
        }
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

    let is_status_html = path == "status" || path == "status.html" || path.starts_with("status/");
    if is_status_html {
        return status_page_handler(State(state), uri).await;
    }

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
        "docs" | "docs.html" => "docs.html",
        "landing" | "landing.html" => "landing.html",
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
                .header(
                    header::CACHE_CONTROL,
                    HeaderValue::from_static(cache_control_for(target_file)),
                )
                .header(
                    HeaderName::from_static("x-robots-tag"),
                    HeaderValue::from_static("noindex, nofollow, noarchive"),
                )
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
                .header(
                    HeaderName::from_static("x-robots-tag"),
                    HeaderValue::from_static("noindex, nofollow, noarchive"),
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
