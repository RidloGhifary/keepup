use std::time::{Duration, Instant};

use crate::checker::http::CheckResult;

/// Probe a TCP target (`tcp://host:port`) by checking whether a TCP connection can be established.
pub async fn ping_tcp(url: &str, timeout_secs: u64, allow_private: bool) -> CheckResult {
    let raw = url.strip_prefix("tcp://").unwrap_or(url);
    let parts: Vec<&str> = raw.split(':').collect();
    if parts.len() != 2 {
        return CheckResult {
            status_code: None,
            latency_ms: 0,
            is_up: false,
            is_degraded: false,
            error_msg: Some("Invalid TCP target: format must be tcp://host:port".to_string()),
        };
    }

    let host = parts[0];
    let port: u16 = match parts[1].parse() {
        Ok(p) => p,
        Err(_) => {
            return CheckResult {
                status_code: None,
                latency_ms: 0,
                is_up: false,
                is_degraded: false,
                error_msg: Some("Invalid TCP target port".to_string()),
            };
        }
    };

    let start = Instant::now();
    let addrs = match crate::net_guard::resolve_checked(host, port, allow_private).await {
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

    #[tokio::test]
    async fn test_ping_tcp_success_closed_and_private_guard() {
        // 1. Open an ephemeral TCP listener on 127.0.0.1
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let target_url = format!("tcp://127.0.0.1:{port}");

        // Spawn a background task accepting 1 connection
        tokio::spawn(async move {
            let _ = listener.accept().await;
        });

        // Test with allow_private = true: should succeed
        let res = ping_tcp(&target_url, 3, true).await;
        assert!(res.is_up, "Open TCP port must succeed");
        assert_eq!(res.status_code, Some(port as i32));
        assert!(res.error_msg.is_none());

        // Test with allow_private = false: must be blocked by net_guard
        let res_blocked = ping_tcp(&target_url, 3, false).await;
        assert!(!res_blocked.is_up, "Loopback TCP probe must be blocked when allow_private is false");
        assert!(res_blocked.error_msg.unwrap().to_lowercase().contains("blocked"));

        // Test closed port
        let closed_res = ping_tcp("tcp://127.0.0.1:1", 1, true).await;
        assert!(!closed_res.is_up, "Closed TCP port must fail");
    }
}
