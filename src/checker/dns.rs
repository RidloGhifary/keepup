use std::net::{IpAddr, Ipv4Addr};
use std::time::{Duration, Instant};

use hickory_resolver::config::{NameServerConfig, ResolverConfig, ResolverOpts};
use hickory_resolver::net::runtime::TokioRuntimeProvider;
use hickory_resolver::proto::rr::RecordType;
use hickory_resolver::TokioResolver;

use super::http::CheckResult;

/// Clean domain string by stripping protocols, ports, and paths.
pub fn clean_domain(target: &str) -> String {
    let t = target.trim();
    let t = t
        .strip_prefix("dns://")
        .or_else(|| t.strip_prefix("https://"))
        .or_else(|| t.strip_prefix("http://"))
        .unwrap_or(t);
    let t = t.split('/').next().unwrap_or(t);
    let t = t.split(':').next().unwrap_or(t);
    t.trim().to_string()
}

/// Parse record type string into hickory_resolver RecordType.
pub fn parse_record_type(rt_str: Option<&str>) -> RecordType {
    match rt_str.map(|s| s.to_ascii_uppercase()).as_deref() {
        Some("AAAA") => RecordType::AAAA,
        Some("CNAME") => RecordType::CNAME,
        Some("TXT") => RecordType::TXT,
        Some("MX") => RecordType::MX,
        _ => RecordType::A,
    }
}

/// Helper to build a TokioResolver with system config, falling back to public DNS.
fn build_resolver(timeout_secs: u64) -> Result<TokioResolver, String> {
    let mut opts = ResolverOpts::default();
    opts.timeout = Duration::from_secs(timeout_secs);
    opts.attempts = 1;

    let builder = match TokioResolver::builder_tokio() {
        Ok(b) => b.with_options(opts),
        Err(_) => {
            let ns1 = NameServerConfig::udp_and_tcp(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)));
            let ns2 = NameServerConfig::udp_and_tcp(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)));
            let cfg = ResolverConfig::from_name_servers(vec![ns1, ns2]);
            TokioResolver::builder_with_config(cfg, TokioRuntimeProvider::default()).with_options(opts)
        }
    };

    builder.build().map_err(|e| format!("Failed to build DNS resolver: {e}"))
}

/// Perform a DNS lookup and validate result against expected value if supplied.
pub async fn ping_dns(
    target: &str,
    record_type_str: Option<&str>,
    expected_value: Option<&str>,
    timeout_secs: u64,
) -> CheckResult {
    let start = Instant::now();
    let domain = clean_domain(target);

    if domain.is_empty() {
        return CheckResult {
            status_code: None,
            latency_ms: 0,
            is_up: false,
            is_degraded: false,
            error_msg: Some("DNS target domain cannot be empty".to_string()),
        };
    }

    let record_type = parse_record_type(record_type_str);

    let resolver = match build_resolver(timeout_secs) {
        Ok(r) => r,
        Err(err) => {
            return CheckResult {
                status_code: None,
                latency_ms: 0,
                is_up: false,
                is_degraded: false,
                error_msg: Some(err),
            };
        }
    };

    let lookup_fut = resolver.lookup(domain.clone(), record_type);
    let result = tokio::time::timeout(Duration::from_secs(timeout_secs), lookup_fut).await;
    let latency_ms = start.elapsed().as_millis() as i64;

    match result {
        Ok(Ok(lookup)) => {
            let mut records: Vec<String> = Vec::new();
            for ans in lookup.answers() {
                records.push(ans.data.to_string());
            }

            if records.is_empty() {
                return CheckResult {
                    status_code: None,
                    latency_ms,
                    is_up: false,
                    is_degraded: false,
                    error_msg: Some(format!("No DNS records returned for {domain} ({record_type})")),
                };
            }

            if let Some(expected) = expected_value {
                let exp_clean = expected.trim().to_lowercase();
                if !exp_clean.is_empty() {
                    let matched = records.iter().any(|r| {
                        let r_lower = r.trim().to_lowercase();
                        r_lower == exp_clean
                            || r_lower.contains(&exp_clean)
                            || r_lower.trim_end_matches('.') == exp_clean.trim_end_matches('.')
                    });

                    if !matched {
                        return CheckResult {
                            status_code: None,
                            latency_ms,
                            is_up: false,
                            is_degraded: false,
                            error_msg: Some(format!(
                                "DNS answers [{}] did not match expected value '{}'",
                                records.join(", "),
                                expected
                            )),
                        };
                    }
                }
            }

            CheckResult {
                status_code: Some(records.len() as i32),
                latency_ms,
                is_up: true,
                is_degraded: false,
                error_msg: None,
            }
        }
        Ok(Err(err)) => CheckResult {
            status_code: None,
            latency_ms,
            is_up: false,
            is_degraded: false,
            error_msg: Some(format!("DNS lookup failed: {err}")),
        },
        Err(_) => CheckResult {
            status_code: None,
            latency_ms: (timeout_secs * 1000) as i64,
            is_up: false,
            is_degraded: false,
            error_msg: Some(format!("DNS lookup for {domain} timed out after {timeout_secs}s")),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_domain() {
        assert_eq!(clean_domain("https://example.com/api/test"), "example.com");
        assert_eq!(clean_domain("dns://1.1.1.1:53"), "1.1.1.1");
        assert_eq!(clean_domain("google.com"), "google.com");
    }

    #[test]
    fn test_parse_record_type() {
        assert_eq!(parse_record_type(Some("A")), RecordType::A);
        assert_eq!(parse_record_type(Some("aaaa")), RecordType::AAAA);
        assert_eq!(parse_record_type(Some("cname")), RecordType::CNAME);
        assert_eq!(parse_record_type(Some("TXT")), RecordType::TXT);
        assert_eq!(parse_record_type(Some("MX")), RecordType::MX);
        assert_eq!(parse_record_type(None), RecordType::A);
        assert_eq!(parse_record_type(Some("UNKNOWN")), RecordType::A);
    }

    #[tokio::test]
    async fn test_ping_dns_cloudflare() {
        let res = ping_dns("one.one.one.one", Some("A"), Some("1.1.1.1"), 5).await;
        if res.is_up {
            assert!(res.error_msg.is_none());
            assert!(res.latency_ms >= 0);
        }
    }
}
