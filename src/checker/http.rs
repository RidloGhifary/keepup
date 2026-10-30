use std::time::{Duration, Instant};

use reqwest::Client;

/// Outcome of a single HTTP health-check ping.
pub struct CheckResult {
    /// HTTP status code returned by the target (if a response was received).
    pub status_code: Option<i32>,
    /// Round-trip latency in milliseconds.
    pub latency_ms: i64,
    /// Whether the target is considered healthy.
    pub is_up: bool,
    /// Human-readable error description (populated on failure).
    pub error_msg: Option<String>,
}

/// Send an HTTP GET request to `url` and measure round-trip latency.
///
/// A check is considered **up** when a response is received and its status
/// code falls within the 2xx range.  Network errors, timeouts, and non-2xx
/// responses are all treated as failures.
pub async fn ping_http(client: &Client, url: &str, timeout_secs: u64) -> CheckResult {
    let start = Instant::now();

    let result = client
        .get(url)
        .header(reqwest::header::USER_AGENT, "KeepUp-Monitor/1.0")
        .timeout(Duration::from_secs(timeout_secs))
        .send()
        .await;

    let latency_ms = start.elapsed().as_millis() as i64;

    match result {
        Ok(resp) => {
            let status = resp.status().as_u16() as i32;
            let is_up = resp.status().is_success();
            CheckResult {
                status_code: Some(status),
                latency_ms,
                is_up,
                error_msg: if is_up {
                    None
                } else {
                    Some(format!("HTTP {status}"))
                },
            }
        }
        Err(err) => CheckResult {
            status_code: None,
            latency_ms,
            is_up: false,
            error_msg: Some(err.to_string()),
        },
    }
}
