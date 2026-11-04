pub mod alerting;
pub mod dns;
pub mod domain;
pub mod http;
pub mod ping;
pub mod scheduler;
pub mod tcp;
pub mod tls;
pub mod whois;

use reqwest::Client;
pub use http::CheckResult;

/// Unified dispatcher to execute health checks across protocols.
pub async fn execute_probe(
    client: &Client,
    check_type: &str,
    target: &str,
    port: Option<i32>,
    dns_record_type: Option<&str>,
    dns_expected_value: Option<&str>,
    timeout_secs: u64,
) -> CheckResult {
    match check_type.to_lowercase().as_str() {
        "tcp" => tcp::ping_tcp(target, port, timeout_secs).await,
        "dns" => dns::ping_dns(target, dns_record_type, dns_expected_value, timeout_secs).await,
        "ping" => ping::ping_icmp(target, timeout_secs).await,
        _ => http::ping_http(client, target, timeout_secs).await,
    }
}
