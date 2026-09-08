//! Domain models for health check results, statuses, and uptime metrics.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Evaluation outcome for an individual condition expression.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConditionResult {
    /// The condition string evaluated, e.g. "[STATUS] == 200".
    pub condition: String,
    /// Whether the condition passed.
    pub success: bool,
}

/// The result of a single probe execution for an endpoint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EndpointResult {
    /// Timestamp when the probe occurred.
    pub timestamp: DateTime<Utc>,
    /// Whether all conditions passed and the probe succeeded.
    pub success: bool,
    /// HTTP status code or protocol exit code (0 if not applicable).
    pub status_code: u16,
    /// Total duration elapsed during the probe.
    #[serde(with = "crate::duration")]
    pub duration: Duration,
    /// Error messages encountered during probing or condition evaluation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
    /// Individual results for each evaluated condition.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub condition_results: Vec<ConditionResult>,
    /// Resolved IP address if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    /// Hostname probed if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
}

impl EndpointResult {
    /// Create a successful result with given status code and duration.
    pub fn success(status_code: u16, duration: Duration) -> Self {
        Self {
            timestamp: Utc::now(),
            success: true,
            status_code,
            duration,
            errors: Vec::new(),
            condition_results: Vec::new(),
            ip: None,
            hostname: None,
        }
    }

    /// Create a failed result with given error message and status code.
    pub fn failure(status_code: u16, duration: Duration, error: impl Into<String>) -> Self {
        Self {
            timestamp: Utc::now(),
            success: false,
            status_code,
            duration,
            errors: vec![error.into()],
            condition_results: Vec::new(),
            ip: None,
            hostname: None,
        }
    }
}

/// Live status representation of an endpoint for dashboard and API delivery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointStatus {
    /// Endpoint name.
    pub name: String,
    /// Optional grouping category.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// Computed URL-safe unique key (e.g. "group_name" or "name").
    pub key: String,
    /// Recent probe results.
    pub results: Vec<EndpointResult>,
}

impl EndpointStatus {
    /// Create a new status container for an endpoint.
    pub fn new(name: impl Into<String>, group: Option<String>) -> Self {
        let name = name.into();
        let key = compute_endpoint_key(&name, group.as_deref());
        Self {
            name,
            group,
            key,
            results: Vec::new(),
        }
    }

    /// Compute rolling uptime percentage over all recorded results (0.0 to 100.0).
    pub fn uptime_percentage(&self) -> f64 {
        if self.results.is_empty() {
            return 100.0;
        }
        let successful = self.results.iter().filter(|r| r.success).count();
        (successful as f64 / self.results.len() as f64) * 100.0
    }

    /// Compute average response time in milliseconds.
    pub fn average_duration_ms(&self) -> f64 {
        if self.results.is_empty() {
            return 0.0;
        }
        let total_ms: u128 = self.results.iter().map(|r| r.duration.as_millis()).sum();
        total_ms as f64 / self.results.len() as f64
    }

    /// Latest result recorded, if any.
    pub fn latest_result(&self) -> Option<&EndpointResult> {
        self.results.last()
    }
}

/// Helper function to compute a sanitized, URL-safe endpoint key from name and group.
pub fn compute_endpoint_key(name: &str, group: Option<&str>) -> String {
    let raw = match group {
        Some(g) if !g.trim().is_empty() => format!("{}_{}", g.trim(), name.trim()),
        _ => name.trim().to_string(),
    };
    raw.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_endpoint_key() {
        assert_eq!(
            compute_endpoint_key("api-service", Some("core")),
            "core_api-service"
        );
        assert_eq!(compute_endpoint_key("Auth Service", None), "auth_service");
        assert_eq!(
            compute_endpoint_key("db:primary", Some("infra")),
            "infra_db_primary"
        );
    }

    #[test]
    fn test_uptime_percentage() {
        let mut status = EndpointStatus::new("test", None);
        assert_eq!(status.uptime_percentage(), 100.0);

        status
            .results
            .push(EndpointResult::success(200, Duration::from_millis(50)));
        status.results.push(EndpointResult::failure(
            500,
            Duration::from_millis(50),
            "Server error",
        ));
        assert_eq!(status.uptime_percentage(), 50.0);
    }
}
