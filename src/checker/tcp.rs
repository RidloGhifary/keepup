use std::time::{Duration, Instant};

use crate::checker::http::CheckResult;

/// Parse target and optional port override into (host, port).
pub fn parse_host_and_port(
    target: &str,
    port_override: Option<i32>,
) -> Result<(String, u16), String> {
    let t = target.trim();
    let t = t
        .strip_prefix("tcp://")
        .or_else(|| t.strip_prefix("http://"))
        .or_else(|| t.strip_prefix("https://"))
        .unwrap_or(t);
    let t = t.split('/').next().unwrap_or(t);

    if let Some(port) = port_override {
        if !(1..=65535).contains(&port) {
            return Err(format!("Invalid port number: {port}"));
        }
        let host = t.split(':').next().unwrap_or(t).to_string();
        if host.is_empty() {
            return Err("Target host cannot be empty".to_string());
        }
        return Ok((host, port as u16));
    }

    // Bracketed IPv6 format [::1]:port
    if let Some(rest) = t.strip_prefix('[') {
        if let Some((h, p_str)) = rest.split_once(']') {
            let port_str = p_str.strip_prefix(':').unwrap_or("");
            if let Ok(p) = port_str.parse::<u16>() {
                if !h.is_empty() {
                    return Ok((h.to_string(), p));
                }
            }
        }
        return Err("Invalid bracketed IPv6 TCP target".to_string());
    }

    // Standard host:port
    if let Some((h, p_str)) = t.rsplit_once(':') {
        if let Ok(p) = p_str.parse::<u16>() {
            let host = h.trim_matches('[').trim_matches(']').to_string();
            if host.is_empty() {
                return Err("Target host cannot be empty".to_string());
            }
            return Ok((host, p));
        }
    }

    Err("TCP check requires a target port (specified via port field or host:port)".to_string())
}

/// Probe TCP port connectivity on target host with a socket connection attempt.
pub async fn ping_tcp(target: &str, port_override: Option<i32>, timeout_secs: u64) -> CheckResult {
    ping_tcp_checked(target, port_override, timeout_secs, true).await
}

/// Probe a TCP target with SSRF net_guard check.
pub async fn ping_tcp_checked(
    target: &str,
    port_override: Option<i32>,
    timeout_secs: u64,
    allow_private: bool,
) -> CheckResult {
    let start = Instant::now();

    let (host, port) = match parse_host_and_port(target, port_override) {
        Ok(hp) => hp,
        Err(e) => {
            return CheckResult {
                status_code: None,
                latency_ms: 0,
                is_up: false,
                is_degraded: false,
                error_msg: Some(e),
            };
        }
    };

    let addrs = match crate::net_guard::resolve_checked(&host, port, allow_private).await {
        Ok(addrs) => addrs,
        Err(e) => {
            return CheckResult {
                status_code: None,
                latency_ms: start.elapsed().as_millis() as i64,
                is_up: false,
                is_degraded: false,
                error_msg: Some(format!("TCP target resolution failed: {e}")),
            };
        }
    };

    let connect_res = tokio::time::timeout(
        Duration::from_secs(timeout_secs),
        tokio::net::TcpStream::connect(&addrs[..]),
    )
    .await;

    let latency_ms = start.elapsed().as_millis() as i64;

    match connect_res {
        Ok(Ok(_stream)) => CheckResult {
            status_code: Some(port as i32),
            latency_ms,
            is_up: true,
            is_degraded: false,
            error_msg: None,
        },
        Ok(Err(e)) => CheckResult {
            status_code: None,
            latency_ms,
            is_up: false,
            is_degraded: false,
            error_msg: Some(format!("TCP connection error: {e}")),
        },
        Err(_) => CheckResult {
            status_code: None,
            latency_ms,
            is_up: false,
            is_degraded: false,
            error_msg: Some("TCP connection timed out".to_string()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_host_and_port() {
        assert_eq!(
            parse_host_and_port("127.0.0.1", Some(8080)).unwrap(),
            ("127.0.0.1".to_string(), 8080)
        );
        assert_eq!(
            parse_host_and_port("example.com:443", None).unwrap(),
            ("example.com".to_string(), 443)
        );
        assert_eq!(
            parse_host_and_port("tcp://localhost:9000", None).unwrap(),
            ("localhost".to_string(), 9000)
        );
        assert!(parse_host_and_port("127.0.0.1", None).is_err());
        assert!(parse_host_and_port("127.0.0.1", Some(70000)).is_err());
    }

    #[tokio::test]
    async fn test_ping_tcp_success_closed_and_private_guard() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let target_url = format!("tcp://127.0.0.1:{port}");

        tokio::spawn(async move {
            let _ = listener.accept().await;
        });

        let res = ping_tcp_checked(&target_url, None, 3, true).await;
        assert!(res.is_up, "Open TCP port must succeed");
        assert_eq!(res.status_code, Some(port as i32));
        assert!(res.error_msg.is_none());

        let res_blocked = ping_tcp_checked(&target_url, None, 3, false).await;
        assert!(!res_blocked.is_up, "Loopback TCP probe must be blocked when allow_private is false");
        assert!(res_blocked.error_msg.unwrap().to_lowercase().contains("blocked"));

        let closed_res = ping_tcp_checked("tcp://127.0.0.1:1", None, 1, true).await;
        assert!(!closed_res.is_up, "Closed TCP port must fail");

        let res_v6 = ping_tcp_checked("tcp://[::1]:9999", None, 1, false).await;
        assert!(!res_v6.is_up);
        let err_v6 = res_v6.error_msg.unwrap().to_lowercase();
        assert!(
            err_v6.contains("blocked") || err_v6.contains("private"),
            "Loopback IPv6 without allow_private must be blocked by net_guard, not fail format"
        );
    }

    #[tokio::test]
    async fn test_ping_tcp_local_listener() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port() as i32;

        let res = ping_tcp("127.0.0.1", Some(port), 2).await;
        assert!(res.is_up);
        assert!(res.error_msg.is_none());
        assert!(res.latency_ms >= 0);
    }

    #[tokio::test]
    async fn test_ping_tcp_connection_refused() {
        let res = ping_tcp("127.0.0.1", Some(1), 1).await;
        assert!(!res.is_up);
        assert!(res.error_msg.is_some());
    }
}
