//! gRPC Health Checking Protocol (grpc.health.v1.Health/Check) implementation.

use chrono::Utc;
use ratus_core::config::EndpointConfig;
use ratus_core::models::{ConditionResult, EndpointResult};
use ratus_eval::{parse_condition, EvaluationContext};
use reqwest::header::{HeaderMap, HeaderValue};
use std::time::{Duration, Instant};
use tokio::time::timeout;

/// gRPC prober querying `grpc.health.v1.Health/Check`.
#[derive(Clone)]
pub struct GrpcProber {
    client: reqwest::Client,
}

impl Default for GrpcProber {
    fn default() -> Self {
        Self::new()
    }
}

impl GrpcProber {
    /// Create a new gRPC health check prober.
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// Probe a gRPC endpoint.
    pub async fn probe(&self, endpoint: &EndpointConfig) -> EndpointResult {
        let url_str = match &endpoint.url {
            Some(u) => u,
            None => {
                return EndpointResult::failure(
                    0,
                    Duration::ZERO,
                    "No target URL specified for gRPC probe",
                )
            }
        };

        // Format target into HTTP/2 URL
        let http_url = if let Some(stripped) = url_str.strip_prefix("grpc://") {
            format!("http://{stripped}/grpc.health.v1.Health/Check")
        } else if let Some(stripped) = url_str.strip_prefix("grpcs://") {
            format!("https://{stripped}/grpc.health.v1.Health/Check")
        } else {
            format!("{url_str}/grpc.health.v1.Health/Check")
        };

        let start = Instant::now();
        let timeout_dur = endpoint
            .client
            .as_ref()
            .and_then(|c| c.timeout)
            .unwrap_or(Duration::from_secs(10));

        // 5-byte gRPC frame: 1 byte compressed flag (0) + 4 bytes big-endian length (0 for empty HealthCheckRequest)
        let grpc_body: [u8; 5] = [0x00, 0x00, 0x00, 0x00, 0x00];

        let mut headers = HeaderMap::new();
        headers.insert("content-type", HeaderValue::from_static("application/grpc"));
        headers.insert("te", HeaderValue::from_static("trailers"));

        let req_fut = self
            .client
            .post(&http_url)
            .headers(headers)
            .body(grpc_body.to_vec())
            .send();
        let res = timeout(timeout_dur, req_fut).await;
        let duration = start.elapsed();

        let (status_code, error_msg, connected) = match res {
            Ok(Ok(resp)) => {
                let status = resp.status().as_u16();
                let grpc_status = resp
                    .headers()
                    .get("grpc-status")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<i32>().ok());

                let healthy = status == 200 && grpc_status.unwrap_or(0) == 0;
                let err = if !healthy {
                    Some(format!(
                        "gRPC status check failed: code={status}, grpc-status={grpc_status:?}"
                    ))
                } else {
                    None
                };
                (status, err, healthy)
            }
            Ok(Err(err)) => (0, Some(format!("gRPC request error: {err}")), false),
            Err(_) => (0, Some("gRPC check timed out".to_string()), false),
        };

        let ctx = EvaluationContext::new(status_code, duration).with_connected(connected);

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
            status_code,
            duration,
            errors,
            condition_results,
            hostname: None,
            ip: None,
        }
    }
}
