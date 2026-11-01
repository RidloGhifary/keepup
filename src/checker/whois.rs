use std::time::Duration;

use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Maximum number of connection retry attempts for transient socket failures.
pub const MAX_WHOIS_RETRIES: u32 = 3;

/// Base backoff delay in milliseconds.
pub const BASE_BACKOFF_MS: u64 = 500;

/// Maximum backoff delay in milliseconds (4 seconds).
pub const MAX_BACKOFF_MS: u64 = 4000;

/// Maximum number of WHOIS referrals to follow.
const MAX_REFERRALS: usize = 2;

/// Maximum bytes to read from a single WHOIS query response (128 KB).
const MAX_RESPONSE_BYTES: u64 = 128 * 1024;

/// Errors encountered during raw WHOIS TCP socket queries.
#[derive(Debug, thiserror::Error, PartialEq, Eq, Clone)]
pub enum WhoisError {
    #[error("I/O error during WHOIS query for {0}: {1}")]
    Io(String, String),

    #[error("Connection timeout querying WHOIS server {0} for {1}")]
    Timeout(String, String),

    #[error("No expiration date found in WHOIS response for {0}")]
    NoExpiration(String),

    #[error("Failed to parse date '{0}' from WHOIS response for {1}")]
    ParseDate(String, String),

    #[error("Invalid domain name: {0}")]
    InvalidDomain(String),

    #[error("Rate limited by WHOIS server {0} for {1}: {2}")]
    RateLimited(String, String, String),
}

/// Computes exponential backoff delay with ±15% pseudorandom jitter without external crates.
/// Formula: min(base_delay * 2^attempt, max_delay) + jitter
pub fn calculate_backoff_with_jitter(attempt: u32, base_ms: u64, max_ms: u64) -> u64 {
    let exp_factor = 1u64.checked_shl(attempt).unwrap_or(u64::MAX);
    let base_delay = (base_ms.saturating_mul(exp_factor)).min(max_ms);

    // Monotonic timestamp nanos for zero-dependency pseudo-random jitter
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(54321);

    // Map nanos to range [-150, 150] representing -15.0% to +15.0%
    let jitter_pct = ((nanos % 301) as i64) - 150;
    let jitter = (base_delay as i64 * jitter_pct) / 1000;

    (base_delay as i64 + jitter).max(1) as u64
}

/// Distinguishes transient socket conditions (connection reset, timeout, refused, rate-limited)
/// from fatal errors (invalid domain format, DNS lookup failures) to determine retry eligibility.
pub fn is_transient_error(err: &WhoisError) -> bool {
    match err {
        WhoisError::Timeout(_, _) => true,
        WhoisError::RateLimited(_, _, _) => true,
        WhoisError::Io(_, msg) => {
            let lower = msg.to_lowercase();
            // Fatal errors (e.g. DNS lookup failure) should fail immediately
            if lower.contains("failed to lookup address")
                || lower.contains("nodename nor servname provided")
                || lower.contains("name or service not known")
                || lower.contains("no such host")
                || lower.contains("invalid argument")
                || lower.contains("invalid domain")
            {
                return false;
            }

            // Transient socket conditions: retry
            lower.contains("connection reset")
                || lower.contains("connection refused")
                || lower.contains("connection aborted")
                || lower.contains("timed out")
                || lower.contains("timeout")
                || lower.contains("broken pipe")
                || lower.contains("network is unreachable")
                || lower.contains("unexpected eof")
                || lower.contains("stream closed")
                || lower.contains("failed to read")
                || lower.contains("connect to")
                || lower.contains("write to")
                || lower.contains("os { code: 54") // ECONNRESET on macOS
                || lower.contains("os { code: 61") // ECONNREFUSED on macOS
                || lower.contains("os { code: 104") // ECONNRESET on Linux
                || lower.contains("os { code: 111") // ECONNREFUSED on Linux
        }
        WhoisError::NoExpiration(_) => false,
        WhoisError::ParseDate(_, _) => false,
        WhoisError::InvalidDomain(_) => false,
    }
}

/// Inspects WHOIS text buffer for common registry rate-limiting notices.
pub fn check_rate_limiting(clean_domain: &str, server: &str, raw_response: &str) -> Option<WhoisError> {
    let lower = raw_response.to_lowercase();
    if lower.contains("rate limit")
        || lower.contains("quota exceeded")
        || lower.contains("query limit")
        || lower.contains("too many requests")
        || lower.contains("querying too fast")
        || lower.contains("temporarily blocked")
        || lower.contains("try again later")
    {
        Some(WhoisError::RateLimited(
            server.to_string(),
            clean_domain.to_string(),
            "Rate limit notice detected in WHOIS response".to_string(),
        ))
    } else {
        None
    }
}

/// Maps common top-level domains (including two-part ccTLDs) to their authoritative WHOIS servers.
/// Defaults to `whois.iana.org:43` when unknown.
pub fn get_whois_server_for_domain(domain: &str) -> &'static str {
    let d = domain.trim().to_lowercase();

    // Two-part ccTLDs
    if d.ends_with(".co.id")
        || d.ends_with(".web.id")
        || d.ends_with(".my.id")
        || d.ends_with(".or.id")
        || d.ends_with(".ac.id")
        || d.ends_with(".sch.id")
        || d.ends_with(".go.id")
        || d.ends_with(".biz.id")
    {
        return "whois.id:43";
    }
    if d.ends_with(".co.uk")
        || d.ends_with(".org.uk")
        || d.ends_with(".me.uk")
        || d.ends_with(".net.uk")
    {
        return "whois.nic.uk:43";
    }
    if d.ends_with(".com.au")
        || d.ends_with(".net.au")
        || d.ends_with(".org.au")
        || d.ends_with(".edu.au")
    {
        return "whois.auda.org.au:43";
    }
    if d.ends_with(".co.nz") || d.ends_with(".org.nz") || d.ends_with(".net.nz") {
        return "whois.srs.net.nz:43";
    }
    if d.ends_with(".co.jp") || d.ends_with(".ne.jp") || d.ends_with(".or.jp") {
        return "whois.jprs.jp:43";
    }

    // Top-level domains
    let tld = d.rsplit('.').next().unwrap_or("");
    match tld {
        "com" | "net" => "whois.verisign-grs.com:43",
        "org" => "whois.pir.org:43",
        "io" => "whois.nic.io:43",
        "id" => "whois.id:43",
        "uk" => "whois.nic.uk:43",
        "co" => "whois.nic.co:43",
        "info" => "whois.afilias.net:43",
        "biz" => "whois.nic.biz:43",
        "dev" | "app" | "page" => "whois.nic.google:43",
        "me" => "whois.nic.me:43",
        "ca" => "whois.cira.ca:43",
        "de" => "whois.denic.de:43",
        "nl" => "whois.domain-registry.nl:43",
        "fr" => "whois.afnic.fr:43",
        "au" => "whois.auda.org.au:43",
        "eu" => "whois.eu:43",
        "us" => "whois.nic.us:43",
        "in" => "whois.registry.in:43",
        "ai" => "whois.nic.ai:43",
        "cc" => "ccwhois.verisign-grs.com:43",
        "tv" => "tvwhois.verisign-grs.com:43",
        "xyz" => "whois.nic.xyz:43",
        "tech" | "store" | "online" | "site" => "whois.nic.radix:43",
        "ru" => "whois.tcinet.ru:43",
        "ch" => "whois.nic.ch:43",
        "se" => "whois.iis.se:43",
        "jp" => "whois.jprs.jp:43",
        _ => "whois.iana.org:43",
    }
}

/// Normalizes various registry date string representations into a `DateTime<Utc>`.
pub fn parse_whois_date(raw: &str) -> Option<DateTime<Utc>> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }

    // 1. Try RFC3339 parsing directly (e.g. 2027-10-31T23:59:59Z, 2027-10-31T23:59:59+00:00)
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }

    // Strip trailing timezone words or parentheses: " UTC", " GMT", " (UTC)", "Z"
    let mut cleaned = s;
    for suffix in &[" UTC", " GMT", " (UTC)", " (GMT)", "Z"] {
        if let Some(stripped) = cleaned.strip_suffix(suffix) {
            cleaned = stripped.trim();
        }
    }

    if let Ok(dt) = DateTime::parse_from_rfc3339(cleaned) {
        return Some(dt.with_timezone(&Utc));
    }

    // Formats supported across diverse WHOIS registrar engines
    let formats = [
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%d",
        "%d-%b-%Y",
        "%d-%b-%Y %H:%M:%S",
        "%d-%b-%Y %H:%M:%S%.f",
        "%d-%B-%Y",
        "%d-%B-%Y %H:%M:%S",
        "%Y.%m.%d",
        "%Y.%m.%d %H:%M:%S",
        "%Y/%m/%d",
        "%Y/%m/%d %H:%M:%S",
        "%d/%m/%Y",
        "%d/%m/%Y %H:%M:%S",
        "%d.%m.%Y",
        "%d.%m.%Y %H:%M:%S",
    ];

    for candidate in &[s, cleaned] {
        for fmt in &formats {
            if let Ok(naive) = NaiveDateTime::parse_from_str(candidate, fmt) {
                return Some(DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc));
            }
            if let Ok(naive_date) = NaiveDate::parse_from_str(candidate, fmt) {
                if let Some(naive) = naive_date.and_hms_opt(0, 0, 0) {
                    return Some(DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc));
                }
            }
        }
    }

    None
}

/// Matches unstructured WHOIS key-value lines for domain expiration dates.
pub fn extract_expiration_from_whois_text(
    clean_domain: &str,
    raw_response: &str,
) -> Result<DateTime<Utc>, WhoisError> {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(
            r"(?i)(?:Registry Expiry Date|Registrar Registration Expiration Date|Expiration Date|Expiry Date|expires|paid-till):\s*([^\r\n]+)",
        )
        .expect("Valid WHOIS expiration regex pattern")
    });

    let mut last_candidate = None;
    for cap in re.captures_iter(raw_response) {
        if let Some(m) = cap.get(1) {
            let candidate = m.as_str().trim();
            if !candidate.is_empty() {
                last_candidate = Some(candidate.to_string());
                if let Some(dt) = parse_whois_date(candidate) {
                    return Ok(dt);
                }
            }
        }
    }

    if let Some(raw) = last_candidate {
        Err(WhoisError::ParseDate(raw, clean_domain.to_string()))
    } else {
        Err(WhoisError::NoExpiration(clean_domain.to_string()))
    }
}

/// Extracts a referral WHOIS server hostname from an IANA or parent registry reply.
pub fn extract_referral_whois_server(raw_response: &str) -> Option<String> {
    static REF_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = REF_RE.get_or_init(|| {
        regex::Regex::new(r"(?i)(?:refer|whois|Whois Server):\s*([a-zA-Z0-9.-]+\.[a-zA-Z]{2,})")
            .expect("Valid referral WHOIS regex pattern")
    });

    for cap in re.captures_iter(raw_response) {
        if let Some(m) = cap.get(1) {
            let s = m.as_str().trim();
            if !s.is_empty() && !s.contains("iana.org") {
                return Some(s.to_string());
            }
        }
    }
    None
}

/// Executes a raw TCP socket query with 3 retry attempts, exponential backoff (500ms..4000ms),
/// and ±15% pseudorandom jitter for transient network or rate-limiting errors.
pub async fn query_raw_whois_socket(clean_domain: &str) -> Result<DateTime<Utc>, WhoisError> {
    let clean = clean_domain.trim();
    if clean.is_empty() || clean.contains(' ') || clean.starts_with('.') || clean.ends_with('.') {
        return Err(WhoisError::InvalidDomain(clean_domain.to_string()));
    }

    let initial_server = get_whois_server_for_domain(clean);
    query_whois_server_with_retry(
        clean,
        initial_server,
        MAX_WHOIS_RETRIES,
        BASE_BACKOFF_MS,
        MAX_BACKOFF_MS,
    )
    .await
}

/// Internal execution loop implementing exponential backoff with jitter and transient error categorization.
pub async fn query_whois_server_with_retry(
    clean_domain: &str,
    server: &str,
    max_retries: u32,
    base_backoff_ms: u64,
    max_backoff_ms: u64,
) -> Result<DateTime<Utc>, WhoisError> {
    let mut last_err = WhoisError::NoExpiration(clean_domain.to_string());

    for attempt in 0..max_retries {
        match query_whois_server_recursive(clean_domain, server, 0).await {
            Ok(dt) => return Ok(dt),
            Err(err) => {
                if !is_transient_error(&err) {
                    tracing::warn!(
                        domain = %clean_domain,
                        error = %err,
                        "Non-transient error during WHOIS query; aborting retries immediately"
                    );
                    return Err(err);
                }

                last_err = err;

                if attempt + 1 < max_retries {
                    let delay = calculate_backoff_with_jitter(attempt, base_backoff_ms, max_backoff_ms);
                    tracing::warn!(
                        "WHOIS query attempt {}/3 failed for {}: {}. Retrying in {}ms...",
                        attempt + 1,
                        clean_domain,
                        last_err,
                        delay
                    );
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                }
            }
        }
    }

    tracing::warn!(
        domain = %clean_domain,
        attempts = max_retries,
        final_error = %last_err,
        "All WHOIS query attempts failed"
    );
    Err(last_err)
}

/// Recursive helper for WHOIS server connection with referral chasing up to `MAX_REFERRALS`.
fn query_whois_server_recursive<'a>(
    clean_domain: &'a str,
    server: &'a str,
    depth: usize,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<DateTime<Utc>, WhoisError>> + Send + 'a>> {
    Box::pin(async move {
        let server_addr = if server.contains(':') {
            server.to_string()
        } else {
            format!("{server}:43")
        };

        let timeout_duration = Duration::from_secs(5);

        let raw_response = tokio::time::timeout(timeout_duration, async {
            let mut stream = TcpStream::connect(&server_addr).await.map_err(|e| {
                WhoisError::Io(
                    clean_domain.to_string(),
                    format!("connect to {server_addr} failed: {e}"),
                )
            })?;

            let query = format!("{clean_domain}\r\n");
            stream.write_all(query.as_bytes()).await.map_err(|e| {
                WhoisError::Io(
                    clean_domain.to_string(),
                    format!("write to {server_addr} failed: {e}"),
                )
            })?;

            let _ = stream.shutdown().await;

            let mut buf = Vec::with_capacity(4096);
            let mut handle = stream.take(MAX_RESPONSE_BYTES);
            let n = handle.read_to_end(&mut buf).await.map_err(|e| {
                WhoisError::Io(
                    clean_domain.to_string(),
                    format!("read from {server_addr} failed: {e}"),
                )
            })?;

            if n == 0 {
                return Err(WhoisError::Io(
                    clean_domain.to_string(),
                    format!("remote WHOIS server {server_addr} closed connection without sending data (unexpected EOF / socket reset)"),
                ));
            }

            Ok::<String, WhoisError>(String::from_utf8_lossy(&buf).to_string())
        })
        .await
        .map_err(|_| WhoisError::Timeout(server_addr.clone(), clean_domain.to_string()))??;

        // Check if registry returned rate-limiting or quota notice
        if let Some(rate_limit_err) = check_rate_limiting(clean_domain, &server_addr, &raw_response) {
            return Err(rate_limit_err);
        }

        // Try extracting expiration from this response
        if let Ok(dt) = extract_expiration_from_whois_text(clean_domain, &raw_response) {
            return Ok(dt);
        }

        // If no expiration was found and referral depth remaining, check for referral server
        if depth < MAX_REFERRALS {
            if let Some(referral_server) = extract_referral_whois_server(&raw_response) {
                if !referral_server.eq_ignore_ascii_case(server)
                    && !server_addr.starts_with(&referral_server)
                {
                    tracing::debug!(
                        domain = %clean_domain,
                        from = %server_addr,
                        to = %referral_server,
                        "Following referral WHOIS server"
                    );
                    return query_whois_server_recursive(
                        clean_domain,
                        &referral_server,
                        depth + 1,
                    )
                    .await;
                }
            }
        }

        // Re-run extract to preserve specific error (e.g. ParseDate vs NoExpiration)
        extract_expiration_from_whois_text(clean_domain, &raw_response)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tokio::net::TcpListener;

    #[test]
    fn test_calculate_backoff_with_jitter_ranges() {
        // Attempt 0: base 500ms, ±15% -> [425..575]
        for _ in 0..50 {
            let delay = calculate_backoff_with_jitter(0, 500, 4000);
            assert!(
                (425..=575).contains(&delay),
                "Attempt 0 delay {delay} outside [425, 575]"
            );
        }

        // Attempt 1: base 1000ms, ±15% -> [850..1150]
        for _ in 0..50 {
            let delay = calculate_backoff_with_jitter(1, 500, 4000);
            assert!(
                (850..=1150).contains(&delay),
                "Attempt 1 delay {delay} outside [850, 1150]"
            );
        }

        // Attempt 2: base 2000ms, ±15% -> [1700..2300]
        for _ in 0..50 {
            let delay = calculate_backoff_with_jitter(2, 500, 4000);
            assert!(
                (1700..=2300).contains(&delay),
                "Attempt 2 delay {delay} outside [1700, 2300]"
            );
        }

        // Attempt 3+: base capped at 4000ms, ±15% -> [3400..4600]
        for _ in 0..50 {
            let delay = calculate_backoff_with_jitter(3, 500, 4000);
            assert!(
                (3400..=4600).contains(&delay),
                "Attempt 3 delay {delay} outside [3400, 4600]"
            );
        }
    }

    #[test]
    fn test_transient_vs_fatal_error_classification() {
        // Transient errors -> true
        assert!(is_transient_error(&WhoisError::Timeout("whois.verisign-grs.com".to_string(), "example.com".to_string())));
        assert!(is_transient_error(&WhoisError::RateLimited("whois.pir.org".to_string(), "wikipedia.org".to_string(), "Quota exceeded".to_string())));
        assert!(is_transient_error(&WhoisError::Io("example.com".to_string(), "connection reset by peer".to_string())));
        assert!(is_transient_error(&WhoisError::Io("example.com".to_string(), "connection refused".to_string())));
        assert!(is_transient_error(&WhoisError::Io("example.com".to_string(), "unexpected eof while reading".to_string())));

        // Fatal errors -> false
        assert!(!is_transient_error(&WhoisError::InvalidDomain("invalid domain".to_string())));
        assert!(!is_transient_error(&WhoisError::NoExpiration("example.com".to_string())));
        assert!(!is_transient_error(&WhoisError::ParseDate("Bad Date".to_string(), "example.com".to_string())));
        assert!(!is_transient_error(&WhoisError::Io("bad.host".to_string(), "failed to lookup address information: nodename nor servname provided".to_string())));
        assert!(!is_transient_error(&WhoisError::Io("bad.host".to_string(), "name or service not known".to_string())));
    }

    #[test]
    fn test_get_whois_server_mapping() {
        assert_eq!(get_whois_server_for_domain("example.com"), "whois.verisign-grs.com:43");
        assert_eq!(get_whois_server_for_domain("net-service.net"), "whois.verisign-grs.com:43");
        assert_eq!(get_whois_server_for_domain("wikipedia.org"), "whois.pir.org:43");
        assert_eq!(get_whois_server_for_domain("github.io"), "whois.nic.io:43");
        assert_eq!(get_whois_server_for_domain("mycompany.id"), "whois.id:43");
        assert_eq!(get_whois_server_for_domain("portal.co.id"), "whois.id:43");
        assert_eq!(get_whois_server_for_domain("bbc.co.uk"), "whois.nic.uk:43");
        assert_eq!(get_whois_server_for_domain("canberra.com.au"), "whois.auda.org.au:43");
        assert_eq!(get_whois_server_for_domain("flutter.dev"), "whois.nic.google:43");
        assert_eq!(get_whois_server_for_domain("startup.xyz"), "whois.nic.xyz:43");
        assert_eq!(get_whois_server_for_domain("unknown.exampletld"), "whois.iana.org:43");
    }

    #[test]
    fn test_parse_whois_dates_various_formats() {
        let expected = Utc.with_ymd_and_hms(2027, 10, 31, 23, 59, 59).unwrap();

        // RFC3339
        assert_eq!(parse_whois_date("2027-10-31T23:59:59Z"), Some(expected));
        assert_eq!(parse_whois_date("2027-10-31T23:59:59+00:00"), Some(expected));

        // ISO datetime with space
        assert_eq!(parse_whois_date("2027-10-31 23:59:59"), Some(expected));
        assert_eq!(parse_whois_date("2027-10-31 23:59:59 UTC"), Some(expected));

        // Date only (time sets to 00:00:00)
        let expected_date_only = Utc.with_ymd_and_hms(2025, 8, 14, 0, 0, 0).unwrap();
        assert_eq!(parse_whois_date("2025-08-14"), Some(expected_date_only));
        assert_eq!(parse_whois_date("2025.08.14"), Some(expected_date_only));
        assert_eq!(parse_whois_date("2025/08/14"), Some(expected_date_only));

        // Legacy WHOIS DD-Mon-YYYY
        assert_eq!(parse_whois_date("14-Aug-2025"), Some(expected_date_only));
        assert_eq!(parse_whois_date("14-AUG-2025"), Some(expected_date_only));
        assert_eq!(parse_whois_date("14-aug-2025"), Some(expected_date_only));
        assert_eq!(parse_whois_date("14-Aug-2025 00:00:00"), Some(expected_date_only));
        assert_eq!(parse_whois_date("14-Aug-2025 00:00:00 GMT"), Some(expected_date_only));

        // Invalid format returns None
        assert_eq!(parse_whois_date("Not Available"), None);
        assert_eq!(parse_whois_date(""), None);
    }

    #[test]
    fn test_extract_expiration_from_whois_text() {
        let text_com = "
Domain Name: EXAMPLE.COM
Registry Expiry Date: 2027-10-31T23:59:59Z
";
        let dt = extract_expiration_from_whois_text("example.com", text_com).unwrap();
        assert_eq!(dt, Utc.with_ymd_and_hms(2027, 10, 31, 23, 59, 59).unwrap());

        let text_org = "
Domain Name: WIKIPEDIA.ORG
Registrar Registration Expiration Date: 2026-05-15 12:00:00
";
        let dt = extract_expiration_from_whois_text("wikipedia.org", text_org).unwrap();
        assert_eq!(dt, Utc.with_ymd_and_hms(2026, 5, 15, 12, 0, 0).unwrap());

        let text_uk = "
    Expiry date:  14-Aug-2025
";
        let dt = extract_expiration_from_whois_text("bbc.co.uk", text_uk).unwrap();
        assert_eq!(dt, Utc.with_ymd_and_hms(2025, 8, 14, 0, 0, 0).unwrap());

        let text_ru = "
paid-till:     2026-09-30
";
        let dt = extract_expiration_from_whois_text("yandex.ru", text_ru).unwrap();
        assert_eq!(dt, Utc.with_ymd_and_hms(2026, 9, 30, 0, 0, 0).unwrap());

        let text_dot = "
expires: 2026.11.20
";
        let dt = extract_expiration_from_whois_text("test.example", text_dot).unwrap();
        assert_eq!(dt, Utc.with_ymd_and_hms(2026, 11, 20, 0, 0, 0).unwrap());

        let text_no_exp = "Domain Name: EXAMPLE.COM\nStatus: clientHold\n";
        assert_eq!(
            extract_expiration_from_whois_text("example.com", text_no_exp),
            Err(WhoisError::NoExpiration("example.com".to_string()))
        );
    }

    #[test]
    fn test_extract_referral_whois_server() {
        let iana_text = "
% IANA WHOIS server
domain:       EXAMPLE.IO
refer:        whois.nic.io
status:       ACTIVE
";
        assert_eq!(
            extract_referral_whois_server(iana_text),
            Some("whois.nic.io".to_string())
        );

        let registrar_text = "
Whois Server: whois.godaddy.com
Registrar: GoDaddy.com, LLC
";
        assert_eq!(
            extract_referral_whois_server(registrar_text),
            Some("whois.godaddy.com".to_string())
        );

        let no_ref = "Domain: example.com\nRegistry Expiry Date: 2025-01-01T00:00:00Z\n";
        assert_eq!(extract_referral_whois_server(no_ref), None);
    }

    #[tokio::test]
    async fn test_mock_tcp_whois_socket_server() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let local_addr = listener.local_addr().unwrap();

        let server_task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut query_buf = [0u8; 128];
            let n = socket.read(&mut query_buf).await.unwrap();
            let query = String::from_utf8_lossy(&query_buf[..n]);
            assert!(query.contains("mockdomain.com"));

            let mock_response = "
Domain Name: MOCKDOMAIN.COM
Registry Expiry Date: 2028-06-25T12:00:00Z
Registrar: MockRegistrar LLC
";
            socket.write_all(mock_response.as_bytes()).await.unwrap();
            socket.shutdown().await.unwrap();
        });

        let target_server = format!("{local_addr}");
        let dt = query_whois_server_recursive("mockdomain.com", &target_server, 0)
            .await
            .unwrap();

        assert_eq!(dt, Utc.with_ymd_and_hms(2028, 6, 25, 12, 0, 0).unwrap());
        server_task.await.unwrap();
    }

    #[tokio::test]
    async fn test_retry_policy_recovers_after_two_transient_failures() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let local_addr = listener.local_addr().unwrap();
        let attempts_seen = Arc::new(AtomicUsize::new(0));
        let attempts_clone = attempts_seen.clone();

        let server_task = tokio::spawn(async move {
            // First 2 connection attempts drop socket immediately (transient failure)
            for _ in 0..2 {
                let (mut socket, _) = listener.accept().await.unwrap();
                attempts_clone.fetch_add(1, Ordering::SeqCst);
                // Drop connection abruptly without writing data (EOF / reset)
                let _ = socket.shutdown().await;
            }

            // 3rd attempt succeeds
            let (mut socket, _) = listener.accept().await.unwrap();
            attempts_clone.fetch_add(1, Ordering::SeqCst);
            let mut buf = [0u8; 64];
            let _ = socket.read(&mut buf).await;
            let response = "Registry Expiry Date: 2029-01-01T00:00:00Z\n";
            socket.write_all(response.as_bytes()).await.unwrap();
            let _ = socket.shutdown().await;
        });

        let target_server = format!("{local_addr}");
        // Fast backoff for testing (10ms base, 50ms max)
        let dt = query_whois_server_with_retry("retrytest.com", &target_server, 3, 10, 50)
            .await
            .unwrap();

        assert_eq!(dt, Utc.with_ymd_and_hms(2029, 1, 1, 0, 0, 0).unwrap());
        assert_eq!(attempts_seen.load(Ordering::SeqCst), 3);
        server_task.await.unwrap();
    }

    #[tokio::test]
    async fn test_retry_policy_exhaustion_on_continuous_transient_failures() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let local_addr = listener.local_addr().unwrap();
        let attempts_seen = Arc::new(AtomicUsize::new(0));
        let attempts_clone = attempts_seen.clone();

        let server_task = tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                attempts_clone.fetch_add(1, Ordering::SeqCst);
                let _ = socket.shutdown().await;
            }
        });

        let target_server = format!("{local_addr}");
        let res = query_whois_server_with_retry("failtest.com", &target_server, 3, 5, 20).await;

        assert!(res.is_err());
        assert_eq!(attempts_seen.load(Ordering::SeqCst), 3);
        server_task.abort();
    }

    #[tokio::test]
    async fn test_rate_limit_detection_triggers_retry() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let local_addr = listener.local_addr().unwrap();
        let attempts_seen = Arc::new(AtomicUsize::new(0));
        let attempts_clone = attempts_seen.clone();

        let server_task = tokio::spawn(async move {
            // Attempt 1: return rate limit notice
            let (mut socket, _) = listener.accept().await.unwrap();
            attempts_clone.fetch_add(1, Ordering::SeqCst);
            let mut buf = [0u8; 64];
            let _ = socket.read(&mut buf).await;
            let rl_response = "Error: Query limit exceeded. Please try again later.\n";
            socket.write_all(rl_response.as_bytes()).await.unwrap();
            let _ = socket.shutdown().await;

            // Attempt 2: return valid response
            let (mut socket, _) = listener.accept().await.unwrap();
            attempts_clone.fetch_add(1, Ordering::SeqCst);
            let _ = socket.read(&mut buf).await;
            let success_response = "Registry Expiry Date: 2028-12-15T00:00:00Z\n";
            socket.write_all(success_response.as_bytes()).await.unwrap();
            let _ = socket.shutdown().await;
        });

        let target_server = format!("{local_addr}");
        let dt = query_whois_server_with_retry("ratelimittest.com", &target_server, 3, 10, 50)
            .await
            .unwrap();

        assert_eq!(dt, Utc.with_ymd_and_hms(2028, 12, 15, 0, 0, 0).unwrap());
        assert_eq!(attempts_seen.load(Ordering::SeqCst), 2);
        server_task.await.unwrap();
    }

    #[tokio::test]
    async fn test_retry_aborts_immediately_on_invalid_domain() {
        let res = query_raw_whois_socket("invalid domain with spaces").await;
        assert_eq!(
            res,
            Err(WhoisError::InvalidDomain("invalid domain with spaces".to_string()))
        );
    }
}
