use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use axum_extra::extract::cookie::CookieJar;
use serde_json::json;

use crate::api::auth::{validate_jwt, SESSION_COOKIE_NAME};
use crate::AppState;

/// Middleware ensuring the incoming request has a valid `keepup_session` cookie or Bearer token.
pub async fn require_auth(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    req: Request,
    next: Next,
) -> Response {
    // 1. Try to read token from HttpOnly cookie
    let token = jar
        .get(SESSION_COOKIE_NAME)
        .map(|c| c.value().to_string())
        .or_else(|| {
            // Fallback: Check standard Authorization: Bearer <token> header
            req.headers()
                .get(axum::http::header::AUTHORIZATION)
                .and_then(|h| h.to_str().ok())
                .and_then(|h| h.strip_prefix("Bearer "))
                .map(|s| s.to_string())
        });

    let Some(token) = token else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error": "Authentication required. Please log in.",
                "code": "UNAUTHORIZED"
            })),
        )
            .into_response();
    };

    // 2. Validate JWT signature and expiration
    match validate_jwt(&token, &state.config.jwt_secret) {
        Ok(_) => next.run(req).await,
        Err(_) => (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error": "Session expired or invalid. Please log in again.",
                "code": "INVALID_SESSION"
            })),
        )
            .into_response(),
    }
}
