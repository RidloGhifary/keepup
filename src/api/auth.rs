use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use subtle::ConstantTimeEq;
use time::Duration;

use crate::error::AppError;
use crate::AppState;

pub const SESSION_COOKIE_NAME: &str = "keepup_session";

/// JWT claims embedded in the authentication cookie.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
}

/// Request payload for logging in.
#[derive(Debug, Deserialize)]
pub struct LoginPayload {
    pub username: String,
    pub password: String,
}

/// Constant-time comparison between two strings to prevent timing attacks.
pub fn constant_time_compare(a: &str, b: &str) -> bool {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    if a_bytes.len() != b_bytes.len() {
        return false;
    }
    a_bytes.ct_eq(b_bytes).into()
}

/// Generate a signed JWT token valid for 7 days.
pub fn create_jwt(user: &str, secret: &str) -> Result<String, jsonwebtoken::errors::Error> {
    let now = chrono::Utc::now().timestamp() as usize;
    let exp = (chrono::Utc::now() + chrono::Duration::days(7)).timestamp() as usize;
    let claims = Claims {
        sub: user.to_string(),
        exp,
        iat: now,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
}

/// Validate and decode a JWT token string.
pub fn validate_jwt(token: &str, secret: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )?;
    Ok(token_data.claims)
}

/// `POST /api/auth/login`
///
/// Validates username & password against configured single-admin credentials.
/// On success, issues a signed JWT stored inside an HttpOnly cookie.
pub async fn login(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    Json(payload): Json<LoginPayload>,
) -> Result<(CookieJar, Json<Value>), AppError> {
    let user_valid = constant_time_compare(&payload.username, &state.config.admin_user);
    let pass_valid = constant_time_compare(&payload.password, &state.config.admin_password);

    if !user_valid || !pass_valid {
        return Err(AppError::Unauthorized(
            "Invalid username or password".to_string(),
        ));
    }

    let token = create_jwt(&state.config.admin_user, &state.config.jwt_secret)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("JWT creation failed: {e}")))?;

    let is_secure = std::env::var("COOKIE_SECURE")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);

    let cookie = Cookie::build((SESSION_COOKIE_NAME, token))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(is_secure)
        .max_age(Duration::days(7))
        .build();

    let jar = jar.add(cookie);
    Ok((
        jar,
        Json(json!({
            "authenticated": true,
            "user": state.config.admin_user,
            "message": "Login successful"
        })),
    ))
}

/// `POST /api/auth/logout`
///
/// Clears the `keepup_session` HttpOnly cookie.
pub async fn logout(jar: CookieJar) -> (CookieJar, Json<Value>) {
    let is_secure = std::env::var("COOKIE_SECURE")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);

    let removal_cookie = Cookie::build((SESSION_COOKIE_NAME, ""))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(is_secure)
        .max_age(Duration::seconds(0))
        .build();

    let jar = jar.remove(removal_cookie);
    (
        jar,
        Json(json!({
            "authenticated": false,
            "message": "Logged out successfully"
        })),
    )
}

/// `GET /api/auth/me`
///
/// Checks the validity of the current admin session cookie.
pub async fn me(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
) -> Result<Json<Value>, AppError> {
    let token = jar
        .get(SESSION_COOKIE_NAME)
        .map(|c| c.value().to_string())
        .ok_or_else(|| AppError::Unauthorized("Not authenticated".to_string()))?;

    let claims = validate_jwt(&token, &state.config.jwt_secret)
        .map_err(|_| AppError::Unauthorized("Session expired or invalid".to_string()))?;

    Ok(Json(json!({
        "authenticated": true,
        "user": claims.sub
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_time_compare() {
        assert!(constant_time_compare("admin", "admin"));
        assert!(!constant_time_compare("admin", "wrong"));
        assert!(!constant_time_compare("admin", "admi"));
        assert!(!constant_time_compare("admin", "administrator"));
    }

    #[test]
    fn test_jwt_lifecycle() {
        let secret = "supersecretkey32characterslong123";
        let token = create_jwt("admin", secret).expect("Token generation failed");
        let claims = validate_jwt(&token, secret).expect("Token validation failed");
        assert_eq!(claims.sub, "admin");

        // Wrong secret should fail
        assert!(validate_jwt(&token, "wrongsecret").is_err());

        // Malformed token should fail
        assert!(validate_jwt("not.a.valid.jwt", secret).is_err());
    }

    #[test]
    fn test_expired_jwt_fails() {
        let secret = "supersecretkey32characterslong123";
        // Create an expired token (expired 1 hour ago)
        let claims = Claims {
            sub: "admin".to_string(),
            exp: (chrono::Utc::now() - chrono::Duration::hours(1)).timestamp() as usize,
            iat: (chrono::Utc::now() - chrono::Duration::hours(2)).timestamp() as usize,
        };
        let expired_token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .expect("Failed to encode token");

        assert!(validate_jwt(&expired_token, secret).is_err());
    }
}
