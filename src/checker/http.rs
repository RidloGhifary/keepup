use std::time::{Duration, Instant};

use reqwest::header::{HeaderMap, HeaderName, HeaderValue, USER_AGENT};
use reqwest::{Client, Method};
use serde_json::Value;

#[derive(Clone, Debug)]
pub struct HttpOptions {
    pub method: String,
    pub headers_json: Option<String>,
    pub body: Option<String>,
    pub expected_status_codes: String,
    pub response_keyword: Option<String>,
    pub keyword_match_mode: String,
    pub follow_redirects: bool,
    pub ignore_tls_errors: bool,
    pub degraded_threshold_ms: Option<i32>,
}

impl Default for HttpOptions {
    fn default() -> Self {
        Self {
            method: "GET".to_string(),
            headers_json: None,
            body: None,
            expected_status_codes: "200-299".to_string(),
            response_keyword: None,
            keyword_match_mode: "contains".to_string(),
            follow_redirects: true,
            ignore_tls_errors: false,
            degraded_threshold_ms: None,
        }
    }
}

/// Validate HTTP settings shared by monitor creation and updates.
pub fn validate_options(options: &HttpOptions) -> Result<(), String> {
    parse_method(&options.method)?;
    parse_expected_status_codes(&options.expected_status_codes)?;

    if !matches!(
        options.keyword_match_mode.to_ascii_lowercase().as_str(),
        "contains" | "not_contains"
    ) {
        return Err("keyword_match_mode must be 'contains' or 'not_contains'".to_string());
    }
    if options
        .response_keyword
        .as_deref()
        .is_some_and(|keyword| keyword.trim().is_empty())
    {
        return Err("response_keyword cannot be empty when set".to_string());
    }
    if options.body.as_deref().is_some_and(|body| !body.is_empty())
        && !matches!(
            options.method.to_ascii_uppercase().as_str(),
            "POST" | "PUT" | "PATCH"
        )
    {
        return Err("Request body is supported for POST, PUT, and PATCH monitors".to_string());
    }
    if options
        .degraded_threshold_ms
        .is_some_and(|threshold| threshold < 1)
    {
        return Err("degraded_threshold_ms must be at least 1 millisecond".to_string());
    }

    parse_headers(options.headers_json.as_deref())?;
    Ok(())
}

fn parse_method(method: &str) -> Result<Method, String> {
    let normalized = method.trim().to_ascii_uppercase();
    if !matches!(
        normalized.as_str(),
        "GET" | "HEAD" | "POST" | "PUT" | "PATCH" | "DELETE" | "OPTIONS"
    ) {
        return Err(
            "http_method must be GET, HEAD, POST, PUT, PATCH, DELETE, or OPTIONS".to_string(),
        );
    }
    Method::from_bytes(normalized.as_bytes()).map_err(|_| "Invalid HTTP method".to_string())
}

fn parse_expected_status_codes(input: &str) -> Result<Vec<(u16, u16)>, String> {
    let mut ranges = Vec::new();
    for part in input
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        let (start, end) = if let Some((start, end)) = part.split_once('-') {
            let start = start
                .trim()
                .parse::<u16>()
                .map_err(|_| "Invalid expected status code range".to_string())?;
            let end = end
                .trim()
                .parse::<u16>()
                .map_err(|_| "Invalid expected status code range".to_string())?;
            (start, end)
        } else {
            let status = part
                .parse::<u16>()
                .map_err(|_| "Invalid expected status code".to_string())?;
            (status, status)
        };
        if !(100..=599).contains(&start) || !(100..=599).contains(&end) || start > end {
            return Err("Expected status codes must be between 100 and 599".to_string());
        }
        ranges.push((start, end));
    }
    if ranges.is_empty() {
        return Err("expected_status_codes cannot be empty".to_string());
    }
    Ok(ranges)
}

fn parse_headers(headers_json: Option<&str>) -> Result<HeaderMap, String> {
    let Some(headers_json) = headers_json.filter(|value| !value.trim().is_empty()) else {
        return Ok(HeaderMap::new());
    };
    let value: Value = serde_json::from_str(headers_json)
        .map_err(|_| "http_headers_json must be a JSON object of string values".to_string())?;
    let object = value
        .as_object()
        .ok_or_else(|| "http_headers_json must be a JSON object of string values".to_string())?;
    let mut headers = HeaderMap::new();
    for (name, value) in object {
        let name = HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| format!("Invalid HTTP header name '{name}'"))?;
        let value = value
            .as_str()
            .ok_or_else(|| "HTTP header values must be strings".to_string())?;
        let value = HeaderValue::from_str(value)
            .map_err(|_| format!("Invalid value for HTTP header '{name}'"))?;
        headers.insert(name, value);
    }
    Ok(headers)
}

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

/// Execute a configured HTTP request and measure response validation latency.
pub async fn ping_http(
    client: &Client,
    url: &str,
    timeout_secs: u64,
    options: &HttpOptions,
) -> CheckResult {
    let start = Instant::now();
    let fail = |status_code, error_msg| CheckResult {
        status_code,
        latency_ms: start.elapsed().as_millis() as i64,
        is_up: false,
        error_msg: Some(error_msg),
    };

    let method = match parse_method(&options.method) {
        Ok(method) => method,
        Err(error) => return fail(None, error),
    };
    let expected_codes = match parse_expected_status_codes(&options.expected_status_codes) {
        Ok(ranges) => ranges,
        Err(error) => return fail(None, error),
    };
    let mut headers = match parse_headers(options.headers_json.as_deref()) {
        Ok(headers) => headers,
        Err(error) => return fail(None, error),
    };
    if !headers.contains_key(USER_AGENT) {
        headers.insert(USER_AGENT, HeaderValue::from_static("KeepUp-Monitor/1.0"));
    }

    let request_client = if options.follow_redirects && !options.ignore_tls_errors {
        None
    } else {
        let mut builder = Client::builder()
            .user_agent("KeepUp-Monitor/1.0")
            .danger_accept_invalid_certs(options.ignore_tls_errors);
        if !options.follow_redirects {
            builder = builder.redirect(reqwest::redirect::Policy::none());
        }
        match builder.build() {
            Ok(client) => Some(client),
            Err(error) => return fail(None, format!("Failed to configure HTTP client: {error}")),
        }
    };
    let request_client = request_client.as_ref().unwrap_or(client);

    let mut request = request_client
        .request(method.clone(), url)
        .headers(headers)
        .timeout(Duration::from_secs(timeout_secs));
    if let Some(body) = options.body.as_deref().filter(|body| !body.is_empty()) {
        if matches!(method, Method::POST | Method::PUT | Method::PATCH) {
            request = request.body(body.to_string());
        }
    }

    let response = match request.send().await {
        Ok(response) => response,
        Err(error) => return fail(None, error.to_string()),
    };
    let status = response.status().as_u16();
    let status_ok = expected_codes
        .iter()
        .any(|(start, end)| (*start..=*end).contains(&status));
    if !status_ok {
        return CheckResult {
            status_code: Some(status as i32),
            latency_ms: start.elapsed().as_millis() as i64,
            is_up: false,
            error_msg: Some(format!(
                "HTTP {status} did not match expected status codes {}",
                options.expected_status_codes
            )),
        };
    }

    if let Some(keyword) = options
        .response_keyword
        .as_deref()
        .filter(|keyword| !keyword.is_empty())
    {
        let body = match response.text().await {
            Ok(body) => body,
            Err(error) => {
                return fail(
                    Some(status as i32),
                    format!("Failed to read response body: {error}"),
                )
            }
        };
        let contains = body.contains(keyword);
        let keyword_ok = match options.keyword_match_mode.to_ascii_lowercase().as_str() {
            "contains" => contains,
            "not_contains" => !contains,
            _ => false,
        };
        if !keyword_ok {
            let error = if contains {
                format!("Response body contains forbidden keyword '{keyword}'")
            } else {
                format!("Response body does not contain expected keyword '{keyword}'")
            };
            return CheckResult {
                status_code: Some(status as i32),
                latency_ms: start.elapsed().as_millis() as i64,
                is_up: false,
                error_msg: Some(error),
            };
        }
    }

    let latency_ms = start.elapsed().as_millis() as i64;
    let error_msg = options
        .degraded_threshold_ms
        .filter(|threshold| latency_ms > i64::from(*threshold))
        .map(|threshold| {
            format!("Response took {latency_ms} ms (degraded threshold: {threshold} ms)")
        });
    CheckResult {
        status_code: Some(status as i32),
        latency_ms,
        is_up: true,
        error_msg,
    }
}
