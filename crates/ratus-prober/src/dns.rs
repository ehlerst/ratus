//! Asynchronous DNS resolution prober.

use chrono::Utc;
use ratus_core::config::EndpointConfig;
use ratus_core::models::{ConditionResult, EndpointResult};
use ratus_eval::{parse_condition, EvaluationContext};
use std::time::{Duration, Instant};
use tokio::net::lookup_host;
use tokio::time::timeout;

/// Asynchronous DNS resolution prober.
pub struct DnsProber;

impl Default for DnsProber {
    fn default() -> Self {
        Self::new()
    }
}

impl DnsProber {
    /// Create a new DNS prober.
    pub fn new() -> Self {
        Self
    }

    /// Execute a DNS probe against an endpoint.
    pub async fn probe(&self, endpoint: &EndpointConfig) -> EndpointResult {
        let (query_host, default_port) = match (&endpoint.dns, &endpoint.url) {
            (Some(dns), _) => (dns.query_name.as_str(), 53),
            (_, Some(url_str)) => {
                let stripped = url_str.strip_prefix("dns://").unwrap_or(url_str.as_str());
                (stripped, 53)
            }
            (None, None) => {
                return EndpointResult::failure(
                    0,
                    Duration::ZERO,
                    format!("Endpoint '{}' has no DNS target configured", endpoint.name),
                );
            }
        };

        let host_port = if query_host.contains(':') {
            query_host.to_string()
        } else {
            format!("{query_host}:{default_port}")
        };

        let probe_timeout = endpoint
            .client
            .as_ref()
            .and_then(|c| c.timeout)
            .unwrap_or(Duration::from_secs(5));

        let start = Instant::now();
        let lookup_fut = lookup_host(&host_port);
        let result = timeout(probe_timeout, lookup_fut).await;
        let duration = start.elapsed();

        let (connected, resolved_ip, error_msg) = match result {
            Ok(Ok(mut addrs)) => {
                if let Some(first) = addrs.next() {
                    (true, Some(first.ip().to_string()), None)
                } else {
                    (false, None, Some("No DNS records returned".to_string()))
                }
            }
            Ok(Err(e)) => (false, None, Some(format!("DNS resolution failed: {e}"))),
            Err(_) => (
                false,
                None,
                Some(format!("DNS query timed out after {probe_timeout:?}")),
            ),
        };

        let ctx = EvaluationContext::new(if connected { 200 } else { 0 }, duration)
            .with_connected(connected);

        let mut all_success = connected;
        let mut condition_results = Vec::with_capacity(endpoint.conditions.len());
        let mut errors = Vec::new();

        if let Some(err) = error_msg {
            errors.push(err);
        }

        for cond_str in &endpoint.conditions {
            match parse_condition(cond_str) {
                Ok(cond) => {
                    let passed = cond.evaluate(&ctx);
                    if !passed {
                        all_success = false;
                        errors.push(format!("Condition '{cond_str}' failed"));
                    }
                    condition_results.push(ConditionResult {
                        condition: cond_str.clone(),
                        success: passed,
                    });
                }
                Err(e) => {
                    all_success = false;
                    errors.push(format!("Invalid condition '{cond_str}': {e}"));
                    condition_results.push(ConditionResult {
                        condition: cond_str.clone(),
                        success: false,
                    });
                }
            }
        }

        EndpointResult {
            timestamp: Utc::now(),
            success: all_success,
            status_code: if connected { 200 } else { 0 },
            duration,
            errors,
            condition_results,
            ip: resolved_ip,
            hostname: Some(query_host.to_string()),
        }
    }
}
