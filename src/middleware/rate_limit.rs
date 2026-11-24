use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{header, HeaderValue, Request, Response, StatusCode};
use governor::clock::QuantaInstant;
use governor::middleware::NoOpMiddleware;
use serde_json::json;
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::key_extractor::{KeyExtractor, SmartIpKeyExtractor};
use tower_governor::{GovernorError, GovernorLayer};

/// Proxy-aware IP key extractor (`NV-01`).
///
/// By default (`trust_proxy_headers == false`), extracts strictly from the TCP socket
/// `ConnectInfo<SocketAddr>` so clients cannot bypass rate limits by spoofing `X-Forwarded-For`.
/// When `trust_proxy_headers == true`, delegates to `SmartIpKeyExtractor`.
/// Falls back to `127.0.0.1` in in-memory `oneshot` test invocations without `ConnectInfo`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientIpKeyExtractor {
    pub trust_proxy_headers: bool,
}

impl KeyExtractor for ClientIpKeyExtractor {
    type Key = IpAddr;

    fn extract<T>(&self, req: &Request<T>) -> Result<Self::Key, GovernorError> {
        if self.trust_proxy_headers {
            if let Ok(ip) = SmartIpKeyExtractor.extract(req) {
                return Ok(ip);
            }
        }

        if let Some(connect_info) = req.extensions().get::<ConnectInfo<SocketAddr>>() {
            return Ok(connect_info.0.ip());
        }

        Ok(IpAddr::V4(Ipv4Addr::LOCALHOST))
    }
}

/// Creates the rate limiter layer configured for authentication & password-change protection:
/// - 5 request burst limit per IP
/// - 1 token replenished every 12 seconds (~5 requests/minute quota)
pub fn auth_rate_limiter_with_proxy(
    trust_proxy_headers: bool,
) -> GovernorLayer<ClientIpKeyExtractor, NoOpMiddleware<QuantaInstant>> {
    let config = Arc::new(
        GovernorConfigBuilder::default()
            .period(Duration::from_secs(12))
            .burst_size(5)
            .key_extractor(ClientIpKeyExtractor {
                trust_proxy_headers,
            })
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
                    "error": "Too many authentication attempts. Please wait a minute before trying again."
                });

                builder
                    .body(Body::from(serde_json::to_string(&body).unwrap_or_default()))
                    .unwrap_or_else(|_| {
                        Response::new(Body::from("{\"error\":\"Too many authentication attempts\"}"))
                    })
            })
            .finish()
            .expect("Governor rate-limiting configuration is valid"),
    );

    GovernorLayer { config }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::routing::post;
    use axum::Router;
    use tower::ServiceExt;

    async fn mock_login() -> &'static str {
        "logged_in"
    }

    #[tokio::test]
    async fn test_rate_limiter_ignores_spoofed_xff_by_default() {
        let app = Router::new()
            .route("/login", post(mock_login))
            .layer(auth_rate_limiter_with_proxy(false));

        let peer: SocketAddr = "203.0.113.10:54321".parse().unwrap();

        // Even if the client rotates X-Forwarded-For on every request, default peer-IP mode blocks the 6th attempt!
        for i in 1..=5 {
            let mut req = Request::builder()
                .uri("/login")
                .method("POST")
                .header("X-Forwarded-For", format!("198.51.100.{i}"))
                .body(Body::empty())
                .unwrap();
            req.extensions_mut().insert(ConnectInfo(peer));

            let res = app.clone().oneshot(req).await.unwrap();
            assert_eq!(res.status(), StatusCode::OK);
        }

        let mut sixth = Request::builder()
            .uri("/login")
            .method("POST")
            .header("X-Forwarded-For", "198.51.100.250")
            .body(Body::empty())
            .unwrap();
        sixth.extensions_mut().insert(ConnectInfo(peer));

        let res = app.clone().oneshot(sixth).await.unwrap();
        assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn test_rate_limiter_allows_up_to_burst_and_blocks_excess() {
        let app = Router::new()
            .route("/login", post(mock_login))
            .layer(auth_rate_limiter_with_proxy(true));

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

        let req = Request::builder()
            .uri("/login")
            .method("POST")
            .header("X-Forwarded-For", "192.168.1.100")
            .body(Body::empty())
            .unwrap();

        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);

        // Distinct IP when trust_proxy_headers=true succeeds
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
