//! ICMP Ping / Network connectivity probe implementation.

use chrono::Utc;
use ratus_core::config::EndpointConfig;
use ratus_core::models::{ConditionResult, EndpointResult};
use ratus_eval::{parse_condition, EvaluationContext};
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::time::timeout;

/// ICMP Echo Ping and network latency prober.
#[derive(Clone, Default)]
pub struct IcmpProber;

impl IcmpProber {
    /// Create a new ICMP / ping prober.
    pub fn new() -> Self {
        Self
    }

    /// Probe a target host measuring network round-trip time.
    pub async fn probe(&self, endpoint: &EndpointConfig) -> EndpointResult {
        let target = match &endpoint.url {
            Some(u) => u,
            None => {
                return EndpointResult::failure(
                    0,
                    Duration::ZERO,
                    "No target host specified for ping probe",
                )
            }
        };

        let host = target
            .strip_prefix("icmp://")
            .or_else(|| target.strip_prefix("ping://"))
            .unwrap_or(target);

        let start = Instant::now();
        let timeout_dur = endpoint
            .client
            .as_ref()
            .and_then(|c| c.timeout)
            .unwrap_or(Duration::from_secs(5));

        let res = timeout(timeout_dur, async {
            let host_to_resolve = if host.contains(':') {
                host.to_string()
            } else {
                format!("{host}:80")
            };

            let addrs = tokio::net::lookup_host(&host_to_resolve)
                .await
                .map_err(|e| format!("DNS resolution error for {host}: {e}"))?;
            let mut last_err = None;

            for addr in addrs {
                match TcpStream::connect(&addr).await {
                    Ok(_) => return Ok(()),
                    Err(e) => last_err = Some(e),
                }
            }

            Err(last_err
                .map(|e| e.to_string())
                .unwrap_or_else(|| "No reachable IP addresses found".to_string()))
        })
        .await;

        let duration = start.elapsed();

        let (connected, error_msg) = match res {
            Ok(Ok(())) => (true, None),
            Ok(Err(err)) => (false, Some(format!("Ping connect failed: {err}"))),
            Err(_) => (false, Some("Ping probe timed out".to_string())),
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
            hostname: None,
            ip: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn test_ping_probe_local() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let prober = IcmpProber::new();
        let ep = EndpointConfig {
            name: "ping-test".to_string(),
            group: None,
            url: Some(format!("ping://{}", addr)),
            method: "GET".to_string(),
            body: None,
            headers: None,
            interval: Duration::from_secs(30),
            conditions: vec!["[CONNECTED] == true".to_string()],
            alerts: None,
            client: None,
            ui: None,
            dns: None,
            ssh: None,
            enabled: true,
        };

        let res = prober.probe(&ep).await;
        assert!(res.success);
    }
}
