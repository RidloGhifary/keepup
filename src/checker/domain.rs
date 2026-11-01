use std::time::Duration;

use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// Output of a domain registration expiration lookup via RDAP.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DomainExpiryInfo {
    pub domain: String,
    pub expires_at: String,
    pub days_remaining: i64,
}

/// Errors encountered during RDAP and WHOIS domain expiration checks.
#[derive(Debug, thiserror::Error)]
pub enum DomainCheckError {
    #[allow(dead_code)]
    #[error("HTTP error during RDAP lookup: {0}")]
    Http(String),

    #[error("Domain {0} not found in RDAP or WHOIS registry")]
    NotFound(String),

    #[allow(dead_code)]
    #[error("Failed to parse RDAP response: {0}")]
    Parse(String),

    #[allow(dead_code)]
    #[error("No expiration event found in RDAP data for {0}")]
    NoExpirationEvent(String),

    #[allow(dead_code)]
    #[error("Domain expiration lookup failed for {0}: RDAP ({1}); WHOIS fallback ({2})")]
    CombinedFallbackFailed(String, String, String),

    #[error("WHOIS fallback query failed for {domain}: {error}")]
    WhoisFailed { domain: String, error: String },
}

/// Isolates the apex domain hostname from any arbitrary HTTP/HTTPS URL, host string, or path.
///
/// Strips scheme prefixes (`http://`, `https://`), userinfo, port numbers, path components,
/// query strings, and fragments. Identifies common two-part ccTLDs (such as `.co.uk`, `.com.au`,
/// `.co.id`, `.org.uk`) so the true registrable apex domain is queried.
pub fn extract_apex_domain(raw_url: &str) -> String {
    let mut host = raw_url.trim();

    // 1. Strip scheme
    if let Some(pos) = host.find("://") {
        host = &host[pos + 3..];
    } else if host.starts_with("//") {
        host = &host[2..];
    }

    // 2. Strip path, query, fragment
    if let Some(pos) = host.find('/') {
        host = &host[..pos];
    }
    if let Some(pos) = host.find('?') {
        host = &host[..pos];
    }
    if let Some(pos) = host.find('#') {
        host = &host[..pos];
    }

    // 3. Strip userinfo (user:pass@host)
    if let Some(pos) = host.rfind('@') {
        host = &host[pos + 1..];
    }

    // 4. Strip port number
    if let Some(pos) = host.find(':') {
        host = &host[..pos];
    }

    let host = host.trim().trim_end_matches('.').to_lowercase();
    if host.parse::<std::net::IpAddr>().is_ok() {
        return host;
    }

    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() <= 2 {
        return host;
    }

    // Check for common two-part ccTLDs (.co.uk, .com.au, .co.id, etc.)
    let second_last = parts[parts.len() - 2];
    let last = parts[parts.len() - 1];
    let is_two_part_tld = (second_last == "co"
        || second_last == "com"
        || second_last == "org"
        || second_last == "net"
        || second_last == "edu"
        || second_last == "gov"
        || second_last == "ac")
        && last.len() == 2;

    if is_two_part_tld && parts.len() >= 3 {
        parts[parts.len() - 3..].join(".")
    } else {
        parts[parts.len() - 2..].join(".")
    }
}

/// Extract the expiration timestamp from an RDAP JSON response's `events` array.
pub fn parse_rdap_expiration(value: &serde_json::Value) -> Option<String> {
    let events = value.get("events")?.as_array()?;
    for event in events {
        let action = event.get("eventAction")?.as_str()?;
        if action.eq_ignore_ascii_case("expiration") {
            let date_str = event.get("eventDate")?.as_str()?;
            return Some(date_str.to_string());
        }
    }
    None
}

/// Calculates the integer difference in days between the parsed expiration timestamp and `now`.
pub fn calculate_days_remaining(
    expires_at_str: &str,
    now: DateTime<Utc>,
) -> Result<(DateTime<Utc>, i64), chrono::ParseError> {
    // Attempt standard RFC3339 parsing first
    let expires_at = DateTime::parse_from_rfc3339(expires_at_str)
        .map(|dt| dt.with_timezone(&Utc))
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(expires_at_str, "%Y-%m-%dT%H:%M:%SZ")
                .map(|naive| DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc))
        })?;

    let duration = expires_at.signed_duration_since(now);
    Ok((expires_at, duration.num_days()))
}

/// Query `https://rdap.org/domain/<apex_domain>` using the shared HTTP client,
/// with automatic raw WHOIS TCP fallback over port 43 when RDAP returns 404, 501,
/// connection timeout, network error, or missing expiration payload.
pub async fn check_domain_expiration(
    client: &Client,
    raw_url: &str,
) -> Result<DomainExpiryInfo, DomainCheckError> {
    let apex_domain = extract_apex_domain(raw_url);
    if apex_domain.is_empty() || apex_domain == "localhost" || apex_domain.parse::<std::net::IpAddr>().is_ok() {
        return Err(DomainCheckError::NotFound(apex_domain));
    }

    let rdap_url = format!("https://rdap.org/domain/{apex_domain}");

    // 1. Attempt standard HTTPS RDAP lookup
    let rdap_result: Result<(DateTime<Utc>, i64), String> = async {
        let resp = client
            .get(&rdap_url)
            .header(
                reqwest::header::USER_AGENT,
                "KeepUp-Monitor/1.0 (RDAP; +https://github.com/RidloGhifary/keepup)",
            )
            .header(
                reqwest::header::ACCEPT,
                "application/rdap+json, application/json",
            )
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| format!("HTTP request error: {e}"))?;

        let status = resp.status();
        if status == reqwest::StatusCode::NOT_FOUND || status == reqwest::StatusCode::NOT_IMPLEMENTED {
            return Err(format!("RDAP returned HTTP {status}"));
        }
        if !status.is_success() {
            return Err(format!("RDAP returned HTTP {status}"));
        }

        let value: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse RDAP JSON: {e}"))?;

        let expires_str = parse_rdap_expiration(&value)
            .ok_or_else(|| "No expiration event found in RDAP payload".to_string())?;

        calculate_days_remaining(&expires_str, Utc::now())
            .map_err(|e| format!("Invalid RDAP timestamp format: {e}"))
    }
    .await;

    // 2. Process RDAP success or trigger WHOIS TCP socket fallback
    match rdap_result {
        Ok((expires_at, days_remaining)) => Ok(DomainExpiryInfo {
            domain: apex_domain,
            expires_at: expires_at.to_rfc3339(),
            days_remaining,
        }),
        Err(rdap_reason) => {
            tracing::warn!(
                domain = %apex_domain,
                reason = %rdap_reason,
                "RDAP lookup failed or incomplete; attempting raw WHOIS TCP socket fallback"
            );

            match crate::checker::whois::query_raw_whois_socket(&apex_domain).await {
                Ok(whois_dt) => {
                    let duration = whois_dt.signed_duration_since(Utc::now());
                    Ok(DomainExpiryInfo {
                        domain: apex_domain,
                        expires_at: whois_dt.to_rfc3339(),
                        days_remaining: duration.num_days(),
                    })
                }
                Err(whois_err) => {
                    tracing::warn!(
                        domain = %apex_domain,
                        rdap_error = %rdap_reason,
                        whois_error = %whois_err,
                        "Both RDAP and raw WHOIS TCP fallback failed"
                    );
                    Err(DomainCheckError::WhoisFailed {
                        domain: apex_domain,
                        error: format!("RDAP: {rdap_reason}; WHOIS: {whois_err}"),
                    })
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::json;

    #[test]
    fn test_extract_apex_domain() {
        assert_eq!(extract_apex_domain("https://example.com/health"), "example.com");
        assert_eq!(extract_apex_domain("http://api.example.com:8080/v1/ping?foo=bar#hash"), "example.com");
        assert_eq!(extract_apex_domain("https://sub.auth.service.co.uk/path"), "service.co.uk");
        assert_eq!(extract_apex_domain("https://test.domain.com.au/api"), "domain.com.au");
        assert_eq!(extract_apex_domain("github.com"), "github.com");
        assert_eq!(extract_apex_domain("clean-domain.dev"), "clean-domain.dev");
        assert_eq!(extract_apex_domain("http://user:pass@admin.corp.net/"), "corp.net");
    }

    #[test]
    fn test_parse_rdap_expiration() {
        let rdap_json = json!({
            "objectClassName": "domain",
            "handle": "2138514_DOMAIN_COM-VRSN",
            "ldhName": "EXAMPLE.COM",
            "events": [
                {
                    "eventAction": "registration",
                    "eventDate": "1995-08-14T04:00:00Z"
                },
                {
                    "eventAction": "expiration",
                    "eventDate": "2025-08-13T04:00:00Z"
                },
                {
                    "eventAction": "last changed",
                    "eventDate": "2024-08-14T07:01:23Z"
                }
            ]
        });

        let expiration = parse_rdap_expiration(&rdap_json);
        assert_eq!(expiration, Some("2025-08-13T04:00:00Z".to_string()));

        // Missing expiration event returns None
        let no_exp_json = json!({
            "events": [
                {
                    "eventAction": "registration",
                    "eventDate": "1995-08-14T04:00:00Z"
                }
            ]
        });
        assert_eq!(parse_rdap_expiration(&no_exp_json), None);

        // Empty events array returns None
        assert_eq!(parse_rdap_expiration(&json!({})), None);
    }

    #[test]
    fn test_calculate_days_remaining() {
        let now = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();

        // Expiring in 30 days
        let (_, days) = calculate_days_remaining("2025-01-31T00:00:00Z", now).unwrap();
        assert_eq!(days, 30);

        // Expiring in 100 days
        let (_, days) = calculate_days_remaining("2025-04-11T00:00:00Z", now).unwrap();
        assert_eq!(days, 100);

        // Expired 5 days ago
        let (_, days) = calculate_days_remaining("2024-12-27T00:00:00Z", now).unwrap();
        assert_eq!(days, -5);
    }

    #[tokio::test]
    async fn test_check_domain_expiration_invalid_domain() {
        let client = Client::new();
        let res = check_domain_expiration(&client, "localhost").await;
        assert!(matches!(res, Err(DomainCheckError::NotFound(_))));

        let res_ip = check_domain_expiration(&client, "127.0.0.1:8080").await;
        assert!(matches!(res_ip, Err(DomainCheckError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_check_domain_expiration_fallback_whois_failed() {
        let client = Client::new();
        let res = check_domain_expiration(&client, "nonexistent-domain-12345.invalidtld").await;
        assert!(matches!(res, Err(DomainCheckError::WhoisFailed { .. })));
        if let Err(DomainCheckError::WhoisFailed { domain, error }) = res {
            assert_eq!(domain, "nonexistent-domain-12345.invalidtld");
            assert!(!error.is_empty());
        }
    }
}
