//! Configuration data structures, deserialization, and validation for Ratus.

use crate::duration::{deserialize_duration, deserialize_optional_duration, serialize_duration};
use crate::env::interpolate_env_vars;
use crate::error::{RatusError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::Duration;

/// Default check interval if not specified (60 seconds).
fn default_interval() -> Duration {
    Duration::from_secs(60)
}

/// Root configuration matching Gatus `config.yaml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    /// Whether Prometheus metrics are enabled.
    #[serde(default)]
    pub metrics: bool,

    /// Storage persistence configuration.
    #[serde(default)]
    pub storage: Option<StorageConfig>,

    /// Web UI appearance and links configuration.
    #[serde(default)]
    pub ui: Option<UiConfig>,

    /// Global alerting provider settings.
    #[serde(default)]
    pub alerting: Option<AlertingConfig>,

    /// Monitored endpoints list.
    #[serde(default)]
    pub endpoints: Vec<EndpointConfig>,

    /// Maintenance window configuration.
    #[serde(default)]
    pub maintenance: Option<MaintenanceConfig>,

    /// Remote push monitoring endpoints.
    #[serde(default)]
    pub remote: Option<RemoteConfig>,

    /// Enable verbose debug logging.
    #[serde(default)]
    pub debug: bool,
}

impl Config {
    /// Parse and validate configuration from a YAML string with environment variable interpolation.
    pub fn from_yaml_str(raw: &str) -> Result<Self> {
        let interpolated = interpolate_env_vars(raw);
        let config: Config = serde_yaml::from_str(&interpolated)?;
        config.validate()?;
        Ok(config)
    }

    /// Load and validate configuration from a file path.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path_ref = path.as_ref();
        let content = fs::read_to_string(path_ref).map_err(|e| {
            RatusError::Config(format!(
                "Failed to read config file '{}': {e}",
                path_ref.display()
            ))
        })?;
        Self::from_yaml_str(&content)
    }

    /// Perform comprehensive semantic validation across endpoints and alerts.
    pub fn validate(&self) -> Result<()> {
        if self.endpoints.is_empty() {
            return Err(RatusError::Validation(
                "Configuration must declare at least one endpoint in 'endpoints'".to_string(),
            ));
        }

        let mut seen_keys = HashMap::new();
        for (idx, ep) in self.endpoints.iter().enumerate() {
            ep.validate(idx)?;
            let key = ep.key();
            if let Some(prev_idx) = seen_keys.insert(key.clone(), idx) {
                return Err(RatusError::Validation(format!(
                    "Duplicate endpoint identifier '{key}' found at endpoint index {idx} (first defined at index {prev_idx})"
                )));
            }
        }

        Ok(())
    }
}

/// Endpoint definition to probe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct EndpointConfig {
    /// Human-readable endpoint name.
    pub name: String,

    /// Optional grouping category (e.g. "core", "infrastructure").
    #[serde(default)]
    pub group: Option<String>,

    /// Target URL (for HTTP/HTTPS/WebSocket) or hostname/IP.
    #[serde(default)]
    pub url: Option<String>,

    /// HTTP method (GET, POST, PUT, DELETE, etc.). Defaults to "GET".
    #[serde(default = "default_method")]
    pub method: String,

    /// Request body payload.
    #[serde(default)]
    pub body: Option<String>,

    /// Custom request headers.
    #[serde(default)]
    pub headers: Option<HashMap<String, String>>,

    /// Probe interval (e.g. "30s", "1m"). Defaults to 60s.
    #[serde(
        default = "default_interval",
        deserialize_with = "deserialize_duration",
        serialize_with = "serialize_duration"
    )]
    pub interval: Duration,

    /// Conditions that must evaluate to true for the endpoint to be considered healthy.
    #[serde(default)]
    pub conditions: Vec<String>,

    /// Specific alert configurations for this endpoint.
    #[serde(default)]
    pub alerts: Option<Vec<EndpointAlertConfig>>,

    /// Custom HTTP/TLS client settings.
    #[serde(default)]
    pub client: Option<ClientConfig>,

    /// UI presentation overrides for this endpoint.
    #[serde(default)]
    pub ui: Option<EndpointUiConfig>,

    /// DNS probing configuration.
    #[serde(default)]
    pub dns: Option<DnsConfig>,

    /// SSH probing configuration.
    #[serde(default)]
    pub ssh: Option<SshConfig>,

    /// Whether this endpoint is enabled for probing.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_method() -> String {
    "GET".to_string()
}

fn default_true() -> bool {
    true
}

impl EndpointConfig {
    /// Compute the URL-safe unique key for this endpoint.
    pub fn key(&self) -> String {
        crate::models::compute_endpoint_key(&self.name, self.group.as_deref())
    }

    /// Validate individual endpoint configuration.
    pub fn validate(&self, idx: usize) -> Result<()> {
        if self.name.trim().is_empty() {
            return Err(RatusError::Validation(format!(
                "Endpoint at index {idx} has an empty 'name'"
            )));
        }

        if self.url.is_none() && self.dns.is_none() && self.ssh.is_none() {
            return Err(RatusError::Validation(format!(
                "Endpoint '{}' must specify at least one target ('url', 'dns', or 'ssh')",
                self.name
            )));
        }

        if self.conditions.is_empty() {
            return Err(RatusError::Validation(format!(
                "Endpoint '{}' must define at least one health check in 'conditions'",
                self.name
            )));
        }

        if self.interval.as_millis() == 0 {
            return Err(RatusError::Validation(format!(
                "Endpoint '{}' has an invalid zero interval",
                self.name
            )));
        }

        Ok(())
    }
}

/// Custom HTTP/TLS client configuration per endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct ClientConfig {
    /// Whether to skip TLS certificate verification.
    #[serde(default)]
    pub insecure: bool,

    /// Request timeout (e.g. "5s", "10s").
    #[serde(default, deserialize_with = "deserialize_optional_duration")]
    pub timeout: Option<Duration>,

    /// Custom DNS resolver IP or host.
    #[serde(default)]
    pub dns_resolver: Option<String>,

    /// Whether to ignore HTTP redirects.
    #[serde(default)]
    pub ignore_redirect: bool,
}

/// Alert trigger definition attached to an endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct EndpointAlertConfig {
    /// Alert provider type (slack, discord, telegram, teams, pagerduty, custom, email, etc.).
    #[serde(rename = "type")]
    pub alert_type: String,

    /// Number of consecutive failures before triggering alert.
    #[serde(default = "default_failure_threshold")]
    pub failure_threshold: u32,

    /// Number of consecutive successes before resolving alert.
    #[serde(default = "default_success_threshold")]
    pub success_threshold: u32,

    /// Whether to dispatch notification when incident resolves.
    #[serde(default)]
    pub send_on_resolved: bool,

    /// Custom description or message template.
    #[serde(default)]
    pub description: Option<String>,

    /// Whether this alert is active.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_failure_threshold() -> u32 {
    3
}

fn default_success_threshold() -> u32 {
    2
}

/// UI presentation overrides per endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct EndpointUiConfig {
    /// Hide target URL in the dashboard.
    #[serde(default)]
    pub hide_url: bool,

    /// Hide target hostname in the dashboard.
    #[serde(default)]
    pub hide_hostname: bool,

    /// Do not automatically resolve failed run markers.
    #[serde(default)]
    pub dont_resolve_failed_runs: bool,
}

/// DNS probing configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct DnsConfig {
    /// DNS query type (A, AAAA, CNAME, MX, TXT).
    pub query_type: String,

    /// Target query domain name.
    pub query_name: String,
}

/// SSH probing configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SshConfig {
    /// Target SSH port (default 22).
    #[serde(default = "default_ssh_port")]
    pub port: u16,

    /// SSH username.
    #[serde(default)]
    pub username: Option<String>,
}

fn default_ssh_port() -> u16 {
    22
}

/// Storage engine configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct StorageConfig {
    /// Storage type: "memory", "sqlite", or "postgres".
    #[serde(rename = "type")]
    pub storage_type: String,

    /// Storage database file path or connection string.
    #[serde(default)]
    pub path: Option<String>,
}

/// Web UI customization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct UiConfig {
    /// Page title.
    #[serde(default)]
    pub title: Option<String>,

    /// Meta description.
    #[serde(default)]
    pub description: Option<String>,

    /// Top header text.
    #[serde(default)]
    pub header: Option<String>,

    /// Logo image URL.
    #[serde(default)]
    pub logo: Option<String>,

    /// Main external link URL.
    #[serde(default)]
    pub link: Option<String>,

    /// Top navigation buttons.
    #[serde(default)]
    pub buttons: Option<Vec<UiButtonConfig>>,
}

/// Top navigation button in Web UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct UiButtonConfig {
    /// Button label.
    pub name: String,

    /// Button link target URL.
    pub link: String,
}

/// Alerting providers configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct AlertingConfig {
    /// Slack webhook configuration.
    #[serde(default)]
    pub slack: Option<SlackConfig>,

    /// Discord webhook configuration.
    #[serde(default)]
    pub discord: Option<DiscordConfig>,

    /// Telegram bot configuration.
    #[serde(default)]
    pub telegram: Option<TelegramConfig>,

    /// PagerDuty configuration.
    #[serde(default)]
    pub pagerduty: Option<PagerDutyConfig>,

    /// Generic custom webhook configuration.
    #[serde(default)]
    pub custom: Option<CustomAlertConfig>,
}

/// Slack alert configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SlackConfig {
    /// Slack incoming webhook URL.
    pub webhook_url: String,
}

/// Discord alert configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct DiscordConfig {
    /// Discord webhook URL.
    pub webhook_url: String,
}

/// Telegram alert configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct TelegramConfig {
    /// Telegram bot token.
    pub token: String,
    /// Telegram chat ID.
    pub chat_id: String,
}

/// PagerDuty alert configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PagerDutyConfig {
    /// Integration key.
    pub integration_key: String,
}

/// Custom webhook alert configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CustomAlertConfig {
    /// Destination URL.
    pub url: String,
    /// HTTP method (default POST).
    #[serde(default = "default_post")]
    pub method: String,
    /// Request payload template.
    #[serde(default)]
    pub body: Option<String>,
    /// Custom HTTP headers.
    #[serde(default)]
    pub headers: Option<HashMap<String, String>>,
}

fn default_post() -> String {
    "POST".to_string()
}

/// Maintenance window configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MaintenanceConfig {
    /// Start timestamp or cron expression.
    #[serde(default)]
    pub start: Option<String>,
    /// Duration of maintenance.
    #[serde(default, deserialize_with = "deserialize_optional_duration")]
    pub duration: Option<Duration>,
}

/// Remote endpoints configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct RemoteConfig {
    /// Remote token or secret.
    #[serde(default)]
    pub token: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_gatus_yaml() {
        let yaml = r#"
endpoints:
  - name: website
    url: "https://twin.sh/health"
    interval: 30s
    conditions:
      - "[STATUS] == 200"
      - "[RESPONSE_TIME] < 300"
"#;
        let config = Config::from_yaml_str(yaml).expect("Failed to parse config");
        assert_eq!(config.endpoints.len(), 1);
        let ep = &config.endpoints[0];
        assert_eq!(ep.name, "website");
        assert_eq!(ep.interval, Duration::from_secs(30));
        assert_eq!(ep.conditions.len(), 2);
    }

    #[test]
    fn test_parse_with_env_interpolation() {
        std::env::set_var("RATUS_TEST_HOST", "api.example.com");
        let yaml = r#"
endpoints:
  - name: api
    url: "https://${RATUS_TEST_HOST:localhost}/status"
    interval: 10s
    conditions:
      - "[STATUS] == 200"
"#;
        let config = Config::from_yaml_str(yaml).expect("Failed to parse config");
        assert_eq!(
            config.endpoints[0].url.as_deref(),
            Some("https://api.example.com/status")
        );
        std::env::remove_var("RATUS_TEST_HOST");
    }

    #[test]
    fn test_validation_duplicate_keys() {
        let yaml = r#"
endpoints:
  - name: health
    group: core
    url: "https://example.com"
    conditions: ["[STATUS] == 200"]
  - name: health
    group: core
    url: "https://example.org"
    conditions: ["[STATUS] == 200"]
"#;
        let res = Config::from_yaml_str(yaml);
        assert!(res.is_err());
    }
}
