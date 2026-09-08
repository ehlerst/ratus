//! Embedded Chaos Engine for simulating network latency, failures, and transient errors.

use parking_lot::RwLock;
use ratus_core::models::EndpointResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::info;

/// A chaos injection rule targeting an endpoint key or all endpoints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChaosRule {
    /// Target endpoint key (e.g. "core_api", or "*" to match all endpoints).
    #[serde(alias = "endpoint-key")]
    pub endpoint_key: String,
    /// Injected artificial latency in milliseconds.
    #[serde(default, alias = "latency-ms")]
    pub latency_ms: u64,
    /// Maximum additional random jitter in milliseconds.
    #[serde(default, alias = "jitter-ms")]
    pub jitter_ms: u64,
    /// Force a specific HTTP status code (e.g. 500, 502, 504).
    #[serde(default, alias = "force-status")]
    pub force_status: Option<u16>,
    /// Force an error message failure.
    #[serde(default, alias = "force-error")]
    pub force_error: Option<String>,
    /// Limit how many times this rule will trigger before auto-recovering.
    /// None means the rule fires indefinitely until manually removed.
    #[serde(default, alias = "limit-times")]
    pub limit_times: Option<u32>,
    /// Number of times this rule has fired so far.
    #[serde(default, alias = "times-fired")]
    pub times_fired: u32,
}

/// Thread-safe chaos engine managing failure simulation rules.
#[derive(Clone, Default)]
pub struct ChaosEngine {
    rules: Arc<RwLock<HashMap<String, ChaosRule>>>,
}

impl ChaosEngine {
    /// Create a new empty chaos engine.
    pub fn new() -> Self {
        Self {
            rules: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add or update a chaos injection rule.
    pub fn add_rule(&self, rule: ChaosRule) {
        let mut rules = self.rules.write();
        rules.insert(rule.endpoint_key.clone(), rule);
    }

    /// Get all currently active chaos rules.
    pub fn get_rules(&self) -> Vec<ChaosRule> {
        let rules = self.rules.read();
        rules.values().cloned().collect()
    }

    /// Remove a specific chaos rule by endpoint key.
    pub fn remove_rule(&self, endpoint_key: &str) -> bool {
        let mut rules = self.rules.write();
        rules.remove(endpoint_key).is_some()
    }

    /// Remove all active chaos rules (reset).
    pub fn clear(&self) {
        let mut rules = self.rules.write();
        rules.clear();
    }

    /// Apply active chaos rules against an endpoint probe result.
    pub async fn apply(&self, endpoint_key: &str, result: &mut EndpointResult) {
        let rule_opt = {
            let mut rules = self.rules.write();
            let key_to_check = if rules.contains_key(endpoint_key) {
                Some(endpoint_key.to_string())
            } else if rules.contains_key("*") {
                Some("*".to_string())
            } else {
                None
            };

            if let Some(ref k) = key_to_check {
                if let Some(rule) = rules.get_mut(k) {
                    rule.times_fired += 1;
                    let rule_clone = rule.clone();
                    if let Some(limit) = rule.limit_times {
                        if rule.times_fired >= limit {
                            rules.remove(k);
                            info!(
                                "Chaos rule for '{}' reached limit ({}) and was auto-removed.",
                                k, limit
                            );
                        }
                    }
                    Some(rule_clone)
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some(rule) = rule_opt {
            // 1. Latency injection
            let delay_ms = rule.latency_ms + (rule.jitter_ms / 2);
            if delay_ms > 0 {
                sleep(Duration::from_millis(delay_ms)).await;
                result.duration += Duration::from_millis(delay_ms);
            }

            // 2. Synthetic error status injection
            if let Some(code) = rule.force_status {
                result.status_code = code;
                if code >= 400 {
                    result.success = false;
                }
            }

            // 3. Synthetic error message injection
            if let Some(err) = rule.force_error {
                result.success = false;
                result.errors.push(err);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_chaos_failure_and_transient_recovery() {
        let chaos = ChaosEngine::new();
        let rule = ChaosRule {
            endpoint_key: "api-service".to_string(),
            latency_ms: 10,
            jitter_ms: 0,
            force_status: Some(503),
            force_error: Some("Service Unavailable (Chaos)".to_string()),
            limit_times: Some(2),
            times_fired: 0,
        };

        chaos.add_rule(rule);
        assert_eq!(chaos.get_rules().len(), 1);

        // Run 1: Should fail due to chaos
        let mut r1 = EndpointResult::success(200, Duration::from_millis(5));
        chaos.apply("api-service", &mut r1).await;
        assert!(!r1.success);
        assert_eq!(r1.status_code, 503);
        assert!(r1
            .errors
            .contains(&"Service Unavailable (Chaos)".to_string()));
        assert!(r1.duration >= Duration::from_millis(15));

        // Run 2: Second hit, reached limit of 2
        let mut r2 = EndpointResult::success(200, Duration::from_millis(5));
        chaos.apply("api-service", &mut r2).await;
        assert!(!r2.success);
        assert_eq!(r2.status_code, 503);

        // Run 3: Auto-recovered! Rule should be expired
        let mut r3 = EndpointResult::success(200, Duration::from_millis(5));
        chaos.apply("api-service", &mut r3).await;
        assert!(r3.success);
        assert_eq!(r3.status_code, 200);
        assert_eq!(chaos.get_rules().len(), 0);
    }
}
