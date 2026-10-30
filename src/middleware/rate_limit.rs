use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{header, HeaderValue, Response, StatusCode};
use governor::clock::QuantaInstant;
use governor::middleware::NoOpMiddleware;
use serde_json::json;
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::key_extractor::SmartIpKeyExtractor;
use tower_governor::{GovernorError, GovernorLayer};

/// Creates the rate limiter layer configured for login protection:
/// - 5 request burst limit per IP
/// - 1 token replenished every 12 seconds (~5 requests/minute quota)
/// - Smart IP extraction (X-Forwarded-For, X-Real-IP, Forwarded, or Peer IP)
/// - Custom 429 JSON response matching KeepUp error schema
pub fn auth_rate_limiter() -> GovernorLayer<SmartIpKeyExtractor, NoOpMiddleware<QuantaInstant>> {
    let config = Arc::new(
        GovernorConfigBuilder::default()
            .period(Duration::from_secs(12))
            .burst_size(5)
            .key_extractor(SmartIpKeyExtractor)
            .error_handler(|err| {
                let mut builder = Response::builder()
                    .status(StatusCode::TOO_MANY_REQUESTS)
                    .header(header::CONTENT_TYPE, "application/json");

                if let GovernorError::TooManyRequests { mut headers, .. } = err {
                    if let Some(headers) = headers.take() {
                        if let Some(res_headers) = builder.headers_mut() {
                            *res_headers = headers;
                            res_headers.insert(
                                header::CONTENT_TYPE,
                                HeaderValue::from_static("application/json"),
                            );
                        }
                    }
                }

                let body = json!({
                    "error": "Too many login attempts. Please wait a minute before trying again."
                });

                builder
                    .body(Body::from(serde_json::to_string(&body).unwrap_or_default()))
                    .unwrap_or_else(|_| Response::new(Body::from("{\"error\":\"Too many login attempts\"}")))
            })
            .finish()
            .expect("Governor rate-limiting configuration is valid"),
    );

    GovernorLayer { config }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::http::Request;
    use axum::routing::post;
    use axum::Router;
    use tower::ServiceExt;

    async fn mock_login() -> &'static str {
        "logged_in"
    }

    #[tokio::test]
    async fn test_rate_limiter_allows_up_to_burst_and_blocks_excess() {
        let app = Router::new()
            .route("/login", post(mock_login))
            .layer(auth_rate_limiter());

        // First 5 requests from the same IP (192.168.1.100 via X-Forwarded-For) should succeed
        for i in 1..=5 {
            let req = Request::builder()
                .uri("/login")
                .method("POST")
                .header("X-Forwarded-For", "192.168.1.100")
                .body(Body::empty())
                .unwrap();

            let res = app.clone().oneshot(req).await.unwrap();
            assert_eq!(
                res.status(),
                StatusCode::OK,
                "Request #{i} should succeed within burst limit"
            );
        }

        // 6th request from the same IP should be rate-limited with 429 Too Many Requests
        let req = Request::builder()
            .uri("/login")
            .method("POST")
            .header("X-Forwarded-For", "192.168.1.100")
            .body(Body::empty())
            .unwrap();

        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            res.headers().get("content-type").unwrap(),
            "application/json"
        );

        let body = to_bytes(res.into_body(), 1024).await.unwrap();
        let json_body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            json_body["error"],
            "Too many login attempts. Please wait a minute before trying again."
        );

        // A different IP (192.168.1.200) should still succeed (has its own quota)
        let diff_ip_req = Request::builder()
            .uri("/login")
            .method("POST")
            .header("X-Forwarded-For", "192.168.1.200")
            .body(Body::empty())
            .unwrap();

        let diff_res = app.clone().oneshot(diff_ip_req).await.unwrap();
        assert_eq!(diff_res.status(), StatusCode::OK);
    }
}
