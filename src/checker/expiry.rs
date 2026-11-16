use reqwest::Client;
use std::time::Instant;

use crate::checker::http::CheckResult;
use crate::checker::{domain, tls};

pub fn evaluate_expiry_status(
    ssl_days: Option<i64>,
    domain_days: Option<i64>,
    threshold_days: i32,
) -> (bool, Option<String>) {
    let min_days = match (ssl_days, domain_days) {
        (Some(s), Some(d)) => Some(s.min(d)),
        (Some(s), None) => Some(s),
        (None, Some(d)) => Some(d),
        (None, None) => None,
    };

    match min_days {
        None => (
            false,
            Some("Failed to inspect SSL/Domain expiration".to_string()),
        ),
        Some(days) if days <= 0 => {
            let detail = match (ssl_days, domain_days) {
                (Some(s), Some(d)) if s <= 0 && d <= 0 => {
                    format!(
                        "Both TLS certificate ({s}d) and domain registration ({d}d) have expired"
                    )
                }
                (Some(s), _) if s <= 0 => format!("TLS certificate has expired ({s}d remaining)"),
                (_, Some(d)) if d <= 0 => {
                    format!("Domain registration has expired ({d}d remaining)")
                }
                _ => format!("Asset expired ({days}d remaining)"),
            };
            (false, Some(detail))
        }
        Some(days) => {
            let is_warning = days <= (threshold_days as i64);
            let msg = if is_warning {
                Some(format!(
                    "Asset expiring soon: {days} day(s) remaining (threshold: {threshold_days}d)"
                ))
            } else {
                None
            };
            (true, msg)
        }
    }
}

/// Inspect both TLS/SSL certificate and RDAP/WHOIS domain expiration for a target host.
///
/// Runs async TLS handshake on port 443 and RDAP/WHOIS queries concurrently.
/// Flags monitor as:
/// - Unhealthy (`is_up = false`, `DOWN`) if both inspections fail or if either asset has expired (days <= 0).
/// - Degraded/Warning (`is_up = true`) if days remaining is within `threshold_days`.
/// - Healthy (`is_up = true`) if days remaining exceeds `threshold_days`.
pub async fn ping_expiry(
    client: &Client,
    target: &str,
    threshold_days: i32,
    timeout_secs: u64,
) -> CheckResult {
    let start = Instant::now();

    let (ssl_res, domain_res) = tokio::join!(
        tls::check_ssl_expiration(target, timeout_secs),
        domain::check_domain_expiration(client, target)
    );

    let latency_ms = start.elapsed().as_millis() as i64;

    let ssl_days = ssl_res.as_ref().map(|s| s.days_remaining).ok();
    let domain_days = domain_res.as_ref().map(|d| d.days_remaining).ok();

    let (is_up, error_msg) = evaluate_expiry_status(ssl_days, domain_days, threshold_days);
    let status_code = if is_up { Some(200) } else { None };

    CheckResult {
        status_code,
        latency_ms,
        is_up,
        error_msg,
    }
}

/// Sanitizes and isolates the fully qualified domain name (FQDN) or bare host.
///
/// Strips protocol prefixes (`http://`, `https://`, `//`, etc.), paths, query
/// strings, fragments, userinfo credentials, and port numbers.
pub fn sanitize_domain_target(target: &str) -> String {
    let mut s = target.trim();
    if let Some(pos) = s.find("://") {
        s = &s[pos + 3..];
    } else if let Some(stripped) = s.strip_prefix("//") {
        s = stripped;
    }
    if let Some(pos) = s.find('/') {
        s = &s[..pos];
    }
    if let Some(pos) = s.find('?') {
        s = &s[..pos];
    }
    if let Some(pos) = s.find('#') {
        s = &s[..pos];
    }
    if let Some(pos) = s.rfind('@') {
        s = &s[pos + 1..];
    }
    if let Some(pos) = s.find(':') {
        s = &s[..pos];
    }
    s.trim().trim_end_matches('.').to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_domain_target() {
        assert_eq!(sanitize_domain_target("example.com"), "example.com");
        assert_eq!(sanitize_domain_target("  api.example.com  "), "api.example.com");
        assert_eq!(sanitize_domain_target("https://example.com"), "example.com");
        assert_eq!(sanitize_domain_target("http://example.com/api/v1/health?debug=1#frag"), "example.com");
        assert_eq!(sanitize_domain_target("https://user:pass@vault.example.org:8443/keys"), "vault.example.org");
        assert_eq!(sanitize_domain_target("//cdn.example.co.uk:443/"), "cdn.example.co.uk");
        assert_eq!(sanitize_domain_target("HTTPS://SUB.DOMAIN.EXAMPLE.COM."), "sub.domain.example.com");
    }

    #[test]
    fn test_evaluate_expiry_healthy() {
        let (is_up, err) = evaluate_expiry_status(Some(90), Some(120), 30);
        assert!(is_up);
        assert_eq!(err, None);
    }

    #[test]
    fn test_evaluate_expiry_degraded_warning() {
        let (is_up, err) = evaluate_expiry_status(Some(20), Some(120), 30);
        assert!(is_up);
        assert!(err.unwrap().contains("expiring soon: 20 day(s) remaining"));
    }

    #[test]
    fn test_evaluate_expiry_down_when_expired() {
        let (is_up, err) = evaluate_expiry_status(Some(-1), Some(120), 30);
        assert!(!is_up);
        assert!(err.unwrap().contains("TLS certificate has expired"));

        let (is_up2, err2) = evaluate_expiry_status(Some(50), Some(0), 30);
        assert!(!is_up2);
        assert!(err2.unwrap().contains("Domain registration has expired"));
    }

    #[test]
    fn test_evaluate_expiry_down_when_no_data() {
        let (is_up, err) = evaluate_expiry_status(None, None, 30);
        assert!(!is_up);
        assert!(err.unwrap().contains("Failed to inspect"));
    }
}
