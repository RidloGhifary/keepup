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
        if let Some(ref url) = self.url {
            let u = url.trim();
            if u.is_empty()
                || (!u.starts_with("http://")
                    && !u.starts_with("https://")
                    && !u.starts_with("tcp://")
                    && !u.starts_with("dns://")
                    && !u.starts_with("push://"))
            {
                return Err(AppError::BadRequest(
                    "Monitor URL must start with http://, https://, tcp://, dns://, or push://"
                        .to_string(),
                ));
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
}
