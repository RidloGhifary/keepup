use std::time::{Duration, Instant};

use super::http::CheckResult;

/// Probe TCP port connectivity on target host with a socket connection attempt.
pub async fn ping_tcp(target: &str, port_override: Option<i32>, timeout_secs: u64) -> CheckResult {
    let start = Instant::now();

    let (host, port) = match parse_host_and_port(target, port_override) {
        Ok(hp) => hp,
        Err(e) => {
            return CheckResult {
                status_code: None,
                latency_ms: 0,
                is_up: false,
                error_msg: Some(e),
            };
        }
    };

    let connect_fut = tokio::net::TcpStream::connect((host.as_str(), port));
    let result = tokio::time::timeout(Duration::from_secs(timeout_secs), connect_fut).await;
    let latency_ms = start.elapsed().as_millis() as i64;

    match result {
        Ok(Ok(_stream)) => CheckResult {
            status_code: None,
            latency_ms,
            is_up: true,
            error_msg: None,
        },
        Ok(Err(err)) => CheckResult {
            status_code: None,
            latency_ms,
            is_up: false,
            error_msg: Some(format!("TCP connection failed: {err}")),
        },
        Err(_) => CheckResult {
            status_code: None,
            latency_ms: (timeout_secs * 1000) as i64,
            is_up: false,
            error_msg: Some(format!(
                "TCP connection to {host}:{port} timed out after {timeout_secs}s"
            )),
        },
    }
}

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

    // Attempt extracting host:port
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
        // Port 1 is reserved and typically not open
        let res = ping_tcp("127.0.0.1", Some(1), 1).await;
        assert!(!res.is_up);
        assert!(res.error_msg.is_some());
    }
}
