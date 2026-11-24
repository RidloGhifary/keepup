use serde::{Deserialize, Serialize};

use crate::error::AppError;

fn default_true() -> bool {
    true
}

// ── Monitor DTOs ─────────────────────────────────────────────────────────────

/// Payload for updating an existing health check monitor.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UpdateMonitorDto {
    pub name: Option<String>,
    pub url: Option<String>,
    pub check_interval_secs: Option<i32>,
    pub timeout_secs: Option<i32>,
    pub max_retries: Option<i32>,
    pub is_active: Option<bool>,
    pub is_public: Option<bool>,
    pub check_domain_expiry: Option<bool>,
    pub domain_warning_days: Option<i32>,
    pub check_ssl: Option<bool>,
    pub ssl_warning_days: Option<i32>,
    pub channel_ids: Option<Vec<String>>,
    pub check_type: Option<String>,
    pub port: Option<i32>,
    pub dns_record_type: Option<String>,
    pub dns_expected_value: Option<String>,
    pub http_method: Option<String>,
    pub expected_status: Option<String>,
    pub request_headers: Option<String>,
    pub body_keyword: Option<String>,
    pub json_pointer: Option<String>,
    pub json_expected: Option<String>,
    pub degraded_latency_ms: Option<i64>,
    pub dns_expected: Option<String>,
    pub resend_interval_mins: Option<i32>,
    pub grace_secs: Option<i32>,
}

impl UpdateMonitorDto {
    pub fn validate(&self) -> Result<(), AppError> {
        if let Some(ref name) = self.name {
            if name.trim().is_empty() {
                return Err(AppError::BadRequest(
                    "Monitor name cannot be empty".to_string(),
                ));
            }
        }
        if let Some(ref ctype) = self.check_type {
            let t = ctype.trim().to_lowercase();
            if t != "http" && t != "tcp" && t != "dns" && t != "ping" {
                return Err(AppError::BadRequest(format!(
                    "Invalid check_type '{ctype}'. Allowed values: http, tcp, dns, ping"
                )));
            }
        }
        if let Some(ref url) = self.url {
            let u = url.trim();
            if u.is_empty() {
                return Err(AppError::BadRequest(
                    "Target URL/host cannot be empty".to_string(),
                ));
            }
            let is_http = self
                .check_type
                .as_deref()
                .map(|t| t.eq_ignore_ascii_case("http"))
                .unwrap_or(u.starts_with("http://") || u.starts_with("https://"));
            if is_http && !u.starts_with("http://") && !u.starts_with("https://") {
                return Err(AppError::BadRequest(
                    "HTTP Monitor URL must start with http:// or https://".to_string(),
                ));
            }
        }
        if let Some(port) = self.port {
            if !(1..=65535).contains(&port) {
                return Err(AppError::BadRequest(
                    "TCP port must be between 1 and 65535".to_string(),
                ));
            }
        }
        if let Some(ref rtype) = self.dns_record_type {
            let rt = rtype.trim().to_uppercase();
            if rt != "A" && rt != "AAAA" && rt != "CNAME" && rt != "TXT" && rt != "MX" {
                return Err(AppError::BadRequest(format!(
                    "Invalid dns_record_type '{rtype}'. Allowed: A, AAAA, CNAME, TXT, MX"
                )));
            }
        }
        if let Some(interval) = self.check_interval_secs {
            if !(5..=86400).contains(&interval) {
                return Err(AppError::BadRequest(
                    "check_interval_secs must be between 5 and 86400".to_string(),
                ));
            }
        }
        if let Some(timeout) = self.timeout_secs {
            if !(1..=60).contains(&timeout) {
                return Err(AppError::BadRequest(
                    "timeout_secs must be between 1 and 60".to_string(),
                ));
            }
        }
        if let Some(retries) = self.max_retries {
            if !(1..=10).contains(&retries) {
                return Err(AppError::BadRequest(
                    "max_retries must be between 1 and 10".to_string(),
                ));
            }
        }
        if let Some(domain_days) = self.domain_warning_days {
            if !(1..=365).contains(&domain_days) {
                return Err(AppError::BadRequest(
                    "domain_warning_days must be between 1 and 365".to_string(),
                ));
            }
        }
        if let Some(ssl_days) = self.ssl_warning_days {
            if !(1..=365).contains(&ssl_days) {
                return Err(AppError::BadRequest(
                    "ssl_warning_days must be between 1 and 365".to_string(),
                ));
            }
        }
        if let Some(days) = self.domain_warning_days {
            if days < 1 {
                return Err(AppError::BadRequest(
                    "domain_warning_days must be at least 1 day".to_string(),
                ));
            }
        }
        if let Some(days) = self.ssl_warning_days {
            if days < 1 {
                return Err(AppError::BadRequest(
                    "ssl_warning_days must be at least 1 day".to_string(),
                ));
            }
        }
        Ok(())
    }
}

// ── Status Page DTOs ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CreateStatusPageDto {
    pub slug: String,
    pub title: String,
    pub description: Option<String>,
    #[serde(default = "default_true")]
    pub is_public: bool,
    pub monitors: Option<Vec<StatusPageMonitorInput>>,
    #[serde(default = "default_true")]
    pub allow_indexing: bool,
    #[serde(default)]
    pub meta_title: Option<String>,
    #[serde(default)]
    pub meta_description: Option<String>,
    #[serde(default)]
    pub og_image_url: Option<String>,
    #[serde(default)]
    pub canonical_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StatusPageMonitorInput {
    pub monitor_id: String,
    #[serde(default = "default_services_group")]
    pub group_name: String,
    #[serde(default)]
    pub display_order: i64,
}

fn default_services_group() -> String {
    "Services".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UpdateStatusPageDto {
    pub title: String,
    pub description: Option<String>,
    #[serde(default = "default_true")]
    pub is_public: bool,
    pub monitors: Option<Vec<StatusPageMonitorInput>>,
    #[serde(default = "default_true")]
    pub allow_indexing: bool,
    #[serde(default)]
    pub meta_title: Option<String>,
    #[serde(default)]
    pub meta_description: Option<String>,
    #[serde(default)]
    pub og_image_url: Option<String>,
    #[serde(default)]
    pub canonical_url: Option<String>,
}
