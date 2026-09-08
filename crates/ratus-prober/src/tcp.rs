//! Direct TCP socket connection prober.

use chrono::Utc;
use ratus_core::config::EndpointConfig;
use ratus_core::models::{ConditionResult, EndpointResult};
use ratus_eval::{parse_condition, EvaluationContext};
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::time::timeout;

/// Asynchronous TCP connection prober.
pub struct TcpProber;

impl Default for TcpProber {
    fn default() -> Self {
        Self::new()
    }
}

impl TcpProber {
    /// Create a new TCP prober.
    pub fn new() -> Self {
        Self
    }

    /// Execute a TCP connection probe against an endpoint.
    pub async fn probe(&self, endpoint: &EndpointConfig) -> EndpointResult {
        let target_str = match &endpoint.url {
            Some(u) => u.trim(),
            None => {
                return EndpointResult::failure(
                    0,
                    Duration::ZERO,
                    format!("Endpoint '{}' has no TCP target configured", endpoint.name),
                )
            }
        };

        // Normalize target: strip "tcp://" prefix if present
        let addr_str = target_str.strip_prefix("tcp://").unwrap_or(target_str);

        let probe_timeout = endpoint
            .client
            .as_ref()
            .and_then(|c| c.timeout)
            .unwrap_or(Duration::from_secs(5));

        let start = Instant::now();
        let connect_fut = TcpStream::connect(addr_str);
        let result = timeout(probe_timeout, connect_fut).await;
        let duration = start.elapsed();

        let (connected, error_msg) = match result {
            Ok(Ok(_stream)) => (true, None),
            Ok(Err(e)) => (false, Some(format!("TCP connect error: {e}"))),
            Err(_) => (
                false,
                Some(format!("TCP connection timed out after {probe_timeout:?}")),
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
            ip: None,
            hostname: Some(addr_str.to_string()),
        }
    }
}
