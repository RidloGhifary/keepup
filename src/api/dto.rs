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
            if u.is_empty() || (!u.starts_with("http://") && !u.starts_with("https://")) {
                return Err(AppError::BadRequest(
                    "Monitor URL must be a valid http or https URL".to_string(),
                ));
            }
        }
        if let Some(interval) = self.check_interval_secs {
            if interval < 1 {
                return Err(AppError::BadRequest(
                    "check_interval_secs must be at least 1 second".to_string(),
                ));
            }
        }
        if let Some(timeout) = self.timeout_secs {
            if timeout < 1 {
                return Err(AppError::BadRequest(
                    "timeout_secs must be at least 1 second".to_string(),
                ));
            }
        }
        if let Some(retries) = self.max_retries {
            if retries < 1 {
                return Err(AppError::BadRequest(
                    "max_retries must be at least 1".to_string(),
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

// ── Webhook DTOs ─────────────────────────────────────────────────────────────

/// Payload for registering a new alert webhook.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CreateWebhookRequest {
    pub name: String,
    pub provider: String,
    pub url: String,
    #[serde(default = "default_true")]
    pub is_enabled: bool,
    #[serde(default = "default_true")]
    pub use_default_template: bool,
    #[serde(default = "default_true")]
    pub notify_on_resolve: bool,
    pub message_template: Option<String>,
}

pub type CreateWebhookDto = CreateWebhookRequest;

impl CreateWebhookRequest {
    pub fn validate(&self) -> Result<(), AppError> {
        if self.name.trim().is_empty() {
            return Err(AppError::BadRequest(
                "Webhook name cannot be empty".to_string(),
            ));
        }

        let provider = self.provider.to_lowercase();
        if provider != "discord" && provider != "slack" && provider != "generic" && provider != "generic_webhook" {
            return Err(AppError::BadRequest(format!(
                "Invalid provider '{}'. Allowed values are: discord, slack, generic",
                self.provider
            )));
        }

        let u = self.url.trim();
        if u.is_empty() || (!u.starts_with("http://") && !u.starts_with("https://")) {
            return Err(AppError::BadRequest(
                "Webhook URL must be a valid http or https URL".to_string(),
            ));
        }

        Ok(())
    }
}

/// Payload for updating an existing alert webhook.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UpdateWebhookRequest {
    pub name: Option<String>,
    pub provider: Option<String>,
    pub url: Option<String>,
    pub is_enabled: Option<bool>,
    pub use_default_template: Option<bool>,
    pub notify_on_resolve: Option<bool>,
    pub message_template: Option<String>,
}

pub type UpdateWebhookDto = UpdateWebhookRequest;

impl UpdateWebhookRequest {
    pub fn validate(&self) -> Result<(), AppError> {
        if let Some(ref name) = self.name {
            if name.trim().is_empty() {
                return Err(AppError::BadRequest(
                    "Webhook name cannot be empty".to_string(),
                ));
            }
        }

        if let Some(ref provider) = self.provider {
            let p = provider.to_lowercase();
            if p != "discord" && p != "slack" && p != "generic" && p != "generic_webhook" {
                return Err(AppError::BadRequest(format!(
                    "Invalid provider '{}'. Allowed values are: discord, slack, generic",
                    provider
                )));
            }
        }

        if let Some(ref url) = self.url {
            let u = url.trim();
            if u.is_empty() || (!u.starts_with("http://") && !u.starts_with("https://")) {
                return Err(AppError::BadRequest(
                    "Webhook URL must be a valid http or https URL".to_string(),
                ));
            }
        }

        Ok(())
    }
}

/// Payload for testing an alert webhook endpoint on-the-fly.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TestWebhookRequest {
    pub url: String,
    pub provider: String,
    #[serde(default = "default_true")]
    pub use_default_template: bool,
    pub message_template: Option<String>,
}

impl TestWebhookRequest {
    pub fn validate(&self) -> Result<(), AppError> {
        let provider = self.provider.to_lowercase();
        if provider != "discord" && provider != "slack" && provider != "generic" && provider != "generic_webhook" {
            return Err(AppError::BadRequest(format!(
                "Invalid provider '{}'. Allowed values are: discord, slack, generic",
                self.provider
            )));
        }

        let u = self.url.trim();
        if u.is_empty() || (!u.starts_with("http://") && !u.starts_with("https://")) {
            return Err(AppError::BadRequest(
                "Webhook URL must be a valid http or https URL".to_string(),
            ));
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
