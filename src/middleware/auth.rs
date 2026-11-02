use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use axum_extra::extract::cookie::CookieJar;
use serde_json::json;

use crate::api::auth::{
    validate_jwt, verify_same_origin_for_cookie, SESSION_COOKIE_NAME,
};
use crate::AppState;

/// Middleware ensuring the incoming request has a valid, unrevoked `keepup_session` cookie
/// or `Authorization: Bearer` token, and enforcing `Origin` verification on unsafe cookie requests (`NV-03`).
pub async fn require_auth(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    req: Request,
    next: Next,
) -> Response {
    let bearer_token = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .map(|s| s.to_string());

    let cookie_token = jar.get(SESSION_COOKIE_NAME).map(|c| c.value().to_string());

    let using_cookie = bearer_token.is_none() && cookie_token.is_some();
    let token = bearer_token.or(cookie_token);

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

    if using_cookie
        && !matches!(
            *req.method(),
            Method::GET | Method::HEAD | Method::OPTIONS
        )
    {
        if let Err(err) =
            verify_same_origin_for_cookie(req.headers(), &state.config.cors_allowed_origins)
        {
            return err.into_response();
        }
    }

    let expected_ver = state.current_session_version();
    match validate_jwt(&token, &state.config.jwt_secret, expected_ver) {
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
