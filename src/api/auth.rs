use std::sync::atomic::Ordering;
use std::sync::Arc;

use argon2::password_hash::rand_core::{OsRng, RngCore};
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::extract::State;
use axum::http::{header, HeaderMap};
use axum::Json;
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use time::Duration;
use tracing::warn;

use crate::config::Config;
use crate::error::AppError;
use crate::AppState;

pub const SESSION_COOKIE_NAME: &str = "keepup_session";

fn default_session_ver() -> u64 {
    1
}

/// JWT claims embedded in the authentication cookie.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
    #[serde(default = "default_session_ver")]
    pub ver: u64,
}

/// Request payload for logging in.
#[derive(Debug, Deserialize)]
pub struct LoginPayload {
    pub username: String,
    pub password: String,
}

/// Constant-time comparison between two strings by comparing their fixed-size SHA-256 digests,
/// eliminating early-return length leaks (`SEC-01`).
pub fn constant_time_compare(a: &str, b: &str) -> bool {
    let digest_a = Sha256::digest(a.as_bytes());
    let digest_b = Sha256::digest(b.as_bytes());
    digest_a.ct_eq(&digest_b).into()
}

/// Compute a random-salted Argon2id PHC password hash (`SEC-01`).
pub fn hash_password(password: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);
    let phc = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .expect("Argon2id hashing with valid salt must succeed")
        .to_string();
    format!("argon2:{phc}")
}

/// Verify a candidate password against active credentials (`"argon2:"`, legacy `"sha256:"`, or plaintext fallback).
pub fn verify_password(attempt: &str, stored: &str) -> bool {
    if let Some(phc) = stored.strip_prefix("argon2:") {
        let Ok(parsed) = PasswordHash::new(phc) else {
            return false;
        };
        Argon2::default()
            .verify_password(attempt.as_bytes(), &parsed)
            .is_ok()
    } else if let Some(expected_hash) = stored.strip_prefix("sha256:") {
        let mut hasher = Sha256::new();
        hasher.update(b"keepup_salt_v1:");
        hasher.update(attempt.as_bytes());
        let computed = hex::encode(hasher.finalize());
        constant_time_compare(&computed, expected_hash)
    } else {
        constant_time_compare(attempt, stored)
    }
}

/// Persist or load `jwt_secret`, bootstrap random initial `admin_password` when unset/default,
/// and return the current monotonic `session_version` (`SEC-01`, `SEC-06`).
pub async fn bootstrap_security_state(
    pool: &sqlx::SqlitePool,
    config: &mut Config,
) -> anyhow::Result<u64> {
    // 1. Persist or load JWT signing secret when not explicitly set via environment
    if !config.jwt_secret_from_env {
        let saved: Option<(String,)> =
            sqlx::query_as("SELECT value FROM app_config WHERE key = 'jwt_secret'")
                .fetch_optional(pool)
                .await?;
        if let Some((secret,)) = saved {
            config.jwt_secret = secret;
        } else {
            let mut bytes = [0u8; 32];
            OsRng.fill_bytes(&mut bytes);
            let generated = hex::encode(bytes);
            sqlx::query(
                "INSERT INTO app_config (key, value) VALUES ('jwt_secret', $1) \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            )
            .bind(&generated)
            .execute(pool)
            .await?;
            config.jwt_secret = generated;
        }
    }

    // 2. Bootstrap a strong random admin password on first startup if ADMIN_PASSWORD was unset or 'admin'
    if !config.admin_password_from_env {
        let existing_pw: Option<(String,)> =
            sqlx::query_as("SELECT value FROM app_config WHERE key = 'admin_password'")
                .fetch_optional(pool)
                .await?;
        if existing_pw.is_none() {
            const ALPHABET: &[u8] =
                b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789";
            let mut raw = [0u8; 20];
            OsRng.fill_bytes(&mut raw);
            let generated: String = raw
                .iter()
                .map(|b| ALPHABET[(*b as usize) % ALPHABET.len()] as char)
                .collect();
            let hashed = hash_password(&generated);
            sqlx::query(
                "INSERT INTO app_config (key, value) VALUES ('admin_password', $1) \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            )
            .bind(&hashed)
            .execute(pool)
            .await?;
            config.admin_password = generated.clone();
            warn!(
                user = %config.admin_user,
                bootstrap_password = %generated,
                "ADMIN_PASSWORD was not configured; generated one-time random admin password (change it in Settings or set ADMIN_PASSWORD)"
            );
        }
    }

    // 3. Load or initialize monotonic session_version
    let saved_ver: Option<(String,)> =
        sqlx::query_as("SELECT value FROM app_config WHERE key = 'session_version'")
            .fetch_optional(pool)
            .await?;
    let ver = match saved_ver.and_then(|(v,)| v.parse::<u64>().ok()) {
        Some(v) if v >= 1 => v,
        _ => {
            sqlx::query(
                "INSERT INTO app_config (key, value) VALUES ('session_version', '1') \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            )
            .execute(pool)
            .await?;
            1
        }
    };

    Ok(ver)
}

/// Atomically increments `session_version` in memory and SQLite `app_config`,
/// immediately invalidating all previously minted JWTs (`SEC-06`).
pub async fn bump_session_version(state: &AppState) -> Result<u64, AppError> {
    let next_ver = state.session_version.fetch_add(1, Ordering::SeqCst) + 1;
    sqlx::query(
        "INSERT INTO app_config (key, value) VALUES ('session_version', $1) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(next_ver.to_string())
    .execute(&state.db)
    .await
    .map_err(AppError::Database)?;
    Ok(next_ver)
}

/// Fetch active admin password from SQLite `app_config`, falling back to static config.
pub async fn get_active_admin_password(
    pool: &sqlx::SqlitePool,
    default_password: &str,
) -> String {
    let row: Result<Option<(String,)>, sqlx::Error> =
        sqlx::query_as("SELECT value FROM app_config WHERE key = 'admin_password'")
            .fetch_optional(pool)
            .await;

    match row {
        Ok(Some((val,))) => val,
        _ => default_password.to_string(),
    }
}

/// Generate a signed JWT token valid for 7 days bound to `ver`.
pub fn create_jwt(
    user: &str,
    secret: &str,
    ver: u64,
) -> Result<String, jsonwebtoken::errors::Error> {
    let now = chrono::Utc::now().timestamp() as usize;
    let exp = (chrono::Utc::now() + chrono::Duration::days(7)).timestamp() as usize;
    let claims = Claims {
        sub: user.to_string(),
        exp,
        iat: now,
        ver,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
}

/// Validate and decode a JWT token string, verifying signature, expiration, and `expected_ver`.
pub fn validate_jwt(
    token: &str,
    secret: &str,
    expected_ver: u64,
) -> Result<Claims, jsonwebtoken::errors::Error> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )?;
    if token_data.claims.ver != expected_ver {
        return Err(jsonwebtoken::errors::Error::from(
            jsonwebtoken::errors::ErrorKind::InvalidToken,
        ));
    }
    Ok(token_data.claims)
}

/// Build an HttpOnly `keepup_session` cookie.
pub fn build_session_cookie(token: String) -> Cookie<'static> {
    let is_secure = std::env::var("COOKIE_SECURE")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);

    Cookie::build((SESSION_COOKIE_NAME, token))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(is_secure)
        .max_age(Duration::days(7))
        .build()
}

fn extract_origin_host(origin: &str) -> Option<&str> {
    let rest = origin
        .strip_prefix("https://")
        .or_else(|| origin.strip_prefix("http://"))?;
    let authority = rest.split(['/', '?', '#']).next()?;
    if authority.is_empty() {
        None
    } else {
        Some(authority)
    }
}

/// Enforces `Origin` / `Host` match on unsafe cookie-authenticated requests (`NV-03`).
pub fn verify_same_origin_for_cookie(
    headers: &HeaderMap,
    allowed_origins: &[String],
) -> Result<(), AppError> {
    let Some(origin_val) = headers.get(header::ORIGIN) else {
        return Ok(());
    };
    let origin = origin_val
        .to_str()
        .map_err(|_| AppError::Forbidden("Invalid Origin header".to_string()))?
        .trim();

    if origin.eq_ignore_ascii_case("null") {
        return Err(AppError::Forbidden(
            "Cross-origin request from null Origin rejected".to_string(),
        ));
    }

    if allowed_origins
        .iter()
        .any(|allowed| allowed.eq_ignore_ascii_case(origin))
    {
        return Ok(());
    }

    let Some(origin_host) = extract_origin_host(origin) else {
        return Err(AppError::Forbidden("Malformed Origin header".to_string()));
    };

    let host = headers
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .map(str::trim)
        .unwrap_or("");

    let forwarded_host = headers
        .get("x-forwarded-host")
        .and_then(|h| h.to_str().ok())
        .map(str::trim)
        .unwrap_or("");

    let matches_host = !host.is_empty() && origin_host.eq_ignore_ascii_case(host);
    let matches_forwarded = !forwarded_host.is_empty() && origin_host.eq_ignore_ascii_case(forwarded_host);

    if matches_host || matches_forwarded {
        Ok(())
    } else {
        Err(AppError::Forbidden(
            "Cross-origin cookie request blocked".to_string(),
        ))
    }
}

/// `POST /api/auth/login`
pub async fn login(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    Json(payload): Json<LoginPayload>,
) -> Result<(CookieJar, Json<Value>), AppError> {
    let user_valid = constant_time_compare(&payload.username, &state.config.admin_user);
    let active_password =
        get_active_admin_password(&state.db, &state.config.admin_password).await;
    let pass_valid = verify_password(&payload.password, &active_password);

    if !user_valid || !pass_valid {
        return Err(AppError::Unauthorized(
            "Invalid username or password".to_string(),
        ));
    }

    let ver = state.current_session_version();
    let token = create_jwt(&state.config.admin_user, &state.config.jwt_secret, ver)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("JWT creation failed: {e}")))?;

    let jar = jar.add(build_session_cookie(token));
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
/// Enforces same-origin on cookie logout, revokes active JWTs via `session_version` bump,
/// and clears the `keepup_session` HttpOnly cookie.
pub async fn logout(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    jar: CookieJar,
) -> Result<(CookieJar, Json<Value>), AppError> {
    let cookie_token = jar.get(SESSION_COOKIE_NAME).map(|c| c.value().to_string());
    let bearer_token = headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .map(|s| s.to_string());

    if cookie_token.is_some() && bearer_token.is_none() {
        verify_same_origin_for_cookie(&headers, &state.config.cors_allowed_origins)?;
    }

    if let Some(tok) = bearer_token.or(cookie_token) {
        let cur_ver = state.current_session_version();
        if validate_jwt(&tok, &state.config.jwt_secret, cur_ver).is_ok() {
            let _ = bump_session_version(&state).await?;
        }
    }

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
    Ok((
        jar,
        Json(json!({
            "authenticated": false,
            "message": "Logged out successfully"
        })),
    ))
}

/// `GET /api/auth/me`
pub async fn me(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    let token = jar
        .get(SESSION_COOKIE_NAME)
        .map(|c| c.value().to_string())
        .or_else(|| {
            headers
                .get(header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|h| h.strip_prefix("Bearer ").or_else(|| h.strip_prefix("bearer ")))
                .map(|s| s.trim().to_string())
        })
        .ok_or_else(|| AppError::Unauthorized("Not authenticated".to_string()))?;

    let claims = validate_jwt(
        &token,
        &state.config.jwt_secret,
        state.current_session_version(),
    )
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
        let token = create_jwt("admin", secret, 1).expect("Token generation failed");
        let claims = validate_jwt(&token, secret, 1).expect("Token validation failed");
        assert_eq!(claims.sub, "admin");
        assert_eq!(claims.ver, 1);

        // Wrong session_version must fail (revoked)
        assert!(validate_jwt(&token, secret, 2).is_err());

        // Wrong secret should fail
        assert!(validate_jwt(&token, "wrongsecret", 1).is_err());

        // Malformed token should fail
        assert!(validate_jwt("not.a.valid.jwt", secret, 1).is_err());
    }

    #[test]
    fn test_expired_jwt_fails() {
        let secret = "supersecretkey32characterslong123";
        let claims = Claims {
            sub: "admin".to_string(),
            exp: (chrono::Utc::now() - chrono::Duration::hours(1)).timestamp() as usize,
            iat: (chrono::Utc::now() - chrono::Duration::hours(2)).timestamp() as usize,
            ver: 1,
        };
        let expired_token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .expect("Failed to encode token");

        assert!(validate_jwt(&expired_token, secret, 1).is_err());
    }

    #[test]
    fn test_hash_and_verify_password() {
        let plain = "mypassword123";
        let hashed1 = hash_password(plain);
        let hashed2 = hash_password(plain);
        assert!(
            hashed1.starts_with("argon2:$argon2id$"),
            "expected argon2id PHC prefix, got {hashed1}"
        );
        assert_ne!(hashed1, hashed2, "argon2id hashes must use unique random salts");
        assert!(verify_password(plain, &hashed1));
        assert!(verify_password(plain, &hashed2));
        assert!(!verify_password("wrongpassword", &hashed1));

        // Legacy salted sha256 compatibility
        let mut legacy_hasher = Sha256::new();
        legacy_hasher.update(b"keepup_salt_v1:");
        legacy_hasher.update(plain.as_bytes());
        let legacy_sha = format!("sha256:{}", hex::encode(legacy_hasher.finalize()));
        assert!(verify_password(plain, &legacy_sha));
        assert!(!verify_password("wrongpassword", &legacy_sha));

        // Plaintext fallback support
        assert!(verify_password("admin", "admin"));
        assert!(!verify_password("wrong", "admin"));
    }

    #[tokio::test]
    async fn test_database_error_is_sanitized() {
        use axum::response::IntoResponse;
        let err = AppError::Database(sqlx::Error::RowNotFound);
        let resp = err.into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::INTERNAL_SERVER_ERROR);
        let bytes = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["error"], "Internal server error");
    }

    #[tokio::test]
    async fn test_bootstrap_security_state_persists_jwt_secret_and_admin_password() {
        let pool = crate::db::init_pool("sqlite::memory:").await.unwrap();
        crate::db::run_migrations(&pool).await.unwrap();

        let mut cfg1 = Config::from_env();
        cfg1.jwt_secret_from_env = false;
        cfg1.admin_password_from_env = false;

        let ver1 = bootstrap_security_state(&pool, &mut cfg1).await.unwrap();
        assert_eq!(ver1, 1);
        let persisted_secret = cfg1.jwt_secret.clone();
        let generated_pw = cfg1.admin_password.clone();
        assert_ne!(generated_pw, "admin");

        // Stored hash in app_config verifies generated_pw with Argon2id
        let stored_hash = get_active_admin_password(&pool, "admin").await;
        assert!(stored_hash.starts_with("argon2:$argon2id$"));
        assert!(verify_password(&generated_pw, &stored_hash));

        // Second boot reloads the exact same jwt_secret without overwriting admin_password
        let mut cfg2 = Config::from_env();
        cfg2.jwt_secret_from_env = false;
        cfg2.admin_password_from_env = false;
        let ver2 = bootstrap_security_state(&pool, &mut cfg2).await.unwrap();
        assert_eq!(ver2, 1);
        assert_eq!(cfg2.jwt_secret, persisted_secret);
        let stored_hash_after = get_active_admin_password(&pool, "admin").await;
        assert_eq!(stored_hash_after, stored_hash);
    }

    #[tokio::test]
    async fn test_logout_revokes_jwt_and_origin_csrf_and_security_headers() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;

        let pool = crate::db::init_pool("sqlite::memory:").await.unwrap();
        crate::db::run_migrations(&pool).await.unwrap();

        let state = AppState::new_for_test(pool, Config::from_env());
        let app = crate::build_http_app(state.clone());

        // Login to obtain session cookie
        let login_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/login")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header("Origin", "http://localhost:3000")
                    .body(Body::from(
                        json!({
                            "username": state.config.admin_user,
                            "password": state.config.admin_password,
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(login_res.status(), StatusCode::OK);

        // Default CORS must NOT reflect http://localhost:3000 when CORS_ALLOWED_ORIGINS is unset
        assert!(
            login_res
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .is_none(),
            "wildcard localhost CORS must not be enabled by default"
        );
        // Security headers must be present
        assert_eq!(
            login_res
                .headers()
                .get("x-frame-options")
                .and_then(|v| v.to_str().ok()),
            Some("DENY")
        );
        assert_eq!(
            login_res
                .headers()
                .get("x-content-type-options")
                .and_then(|v| v.to_str().ok()),
            Some("nosniff")
        );

        let set_cookie = login_res
            .headers()
            .get(header::SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        let raw_jwt = set_cookie
            .split(';')
            .next()
            .unwrap()
            .strip_prefix("keepup_session=")
            .unwrap()
            .to_string();

        // Cross-origin POST with cookie and mismatched Origin vs Host must be rejected with 403
        let csrf_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/logout")
                    .header(header::HOST, "keepup.example.com")
                    .header(header::ORIGIN, "http://evil.localhost:3000")
                    .header(header::COOKIE, format!("keepup_session={raw_jwt}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(csrf_res.status(), StatusCode::FORBIDDEN);

        // Same-origin logout succeeds and revokes the JWT server-side
        let logout_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/logout")
                    .header(header::HOST, "keepup.example.com")
                    .header(header::ORIGIN, "https://keepup.example.com")
                    .header(header::COOKIE, format!("keepup_session={raw_jwt}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(logout_res.status(), StatusCode::OK);

        // Replaying the logged-out JWT as Bearer token must now return 401 Unauthorized
        let replay_res = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/monitors")
                    .header(header::AUTHORIZATION, format!("Bearer {raw_jwt}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(replay_res.status(), StatusCode::UNAUTHORIZED);
    }
}
