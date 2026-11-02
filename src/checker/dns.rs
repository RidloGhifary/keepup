use std::net::IpAddr;
use std::time::{Duration, Instant};

use crate::checker::http::CheckResult;

/// Probe a DNS target (`dns://host`), verifying that resolution succeeds and optionally matches `expected`.
pub async fn ping_dns(
    url: &str,
    record_type: Option<&str>,
    expected: Option<&str>,
    timeout_secs: u64,
) -> CheckResult {
    let host = url.strip_prefix("dns://").unwrap_or(url).trim();
    if host.is_empty() {
        return CheckResult {
            status_code: None,
            latency_ms: 0,
            is_up: false,
            is_degraded: false,
            error_msg: Some("Invalid DNS target: host cannot be empty".to_string()),
        };
    }

    let rec_type = record_type.unwrap_or("A").to_uppercase();
    let start = Instant::now();

    // Use tokio::net::lookup_host to resolve host:0
    let resolve_res = tokio::time::timeout(
        Duration::from_secs(timeout_secs),
        tokio::net::lookup_host(format!("{host}:0")),
    )
    .await;

    let latency_ms = start.elapsed().as_millis() as i64;

    let addrs = match resolve_res {
        Ok(Ok(iter)) => iter.collect::<Vec<_>>(),
        Ok(Err(e)) => {
            return CheckResult {
                status_code: None,
                latency_ms,
                is_up: false,
                is_degraded: false,
                error_msg: Some(format!("DNS resolution failed: {e}")),
            };
        }
        Err(_) => {
            return CheckResult {
                status_code: None,
                latency_ms,
                is_up: false,
                is_degraded: false,
                error_msg: Some("DNS resolution timed out".to_string()),
            };
        }
    };

    let resolved_ips: Vec<IpAddr> = addrs
        .into_iter()
        .map(|addr| addr.ip())
        .filter(|ip| match rec_type.as_str() {
            "AAAA" => ip.is_ipv6(),
            _ => ip.is_ipv4(),
        })
        .collect();

    if resolved_ips.is_empty() {
        return CheckResult {
            status_code: None,
            latency_ms,
            is_up: false,
            is_degraded: false,
            error_msg: Some(format!("No {rec_type} records found for host '{host}'")),
        };
    }

    if let Some(expected_val) = expected {
        let expected_trimmed = expected_val.trim();
        let matches = resolved_ips
            .iter()
            .any(|ip| ip.to_string() == expected_trimmed);

        if !matches {
            return CheckResult {
                status_code: None,
                latency_ms,
                is_up: false,
                is_degraded: false,
                error_msg: Some(format!(
                    "DNS drift detected: expected '{expected_trimmed}', found {:?}",
                    resolved_ips
                )),
            };
        }
    }

    CheckResult {
        status_code: Some(resolved_ips.len() as i32),
        latency_ms,
        is_up: true,
        is_degraded: false,
        error_msg: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ping_dns_resolution_and_drift_detection() {
        // 1. Resolve localhost A record matching 127.0.0.1
        let res = ping_dns("dns://localhost", Some("A"), Some("127.0.0.1"), 3).await;
        assert!(res.is_up, "Resolving localhost matching 127.0.0.1 must succeed");
        assert!(res.status_code.unwrap() >= 1);

        // 2. Resolve localhost expecting non-matching IP (drift detection)
        let drift_res = ping_dns("dns://localhost", Some("A"), Some("192.0.2.1"), 3).await;
        assert!(!drift_res.is_up, "DNS drift detection must report down");
        assert!(drift_res.error_msg.unwrap().contains("DNS drift detected"));

        // 3. Resolve non-existent host
        let nonexistent = ping_dns("dns://this-domain-does-not-exist-xyz-12345.invalid", Some("A"), None, 2).await;
        assert!(!nonexistent.is_up, "Non-existent domain must fail resolution");
    }
}
