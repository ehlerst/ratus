//! High-performance asynchronous HTTP and HTTPS prober.

use bytes::Bytes;
use chrono::Utc;
use ratus_core::config::EndpointConfig;
use ratus_core::models::{ConditionResult, EndpointResult};
use ratus_eval::{parse_condition, EvaluationContext};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::{Client, Method};
use std::collections::HashMap;
use std::str::FromStr;
use std::time::{Duration, Instant};

/// Asynchronous HTTP probe engine with connection pooling and condition evaluation.
pub struct HttpProber {
    client: Client,
    insecure_client: Client,
}

impl Default for HttpProber {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpProber {
    /// Create a new HTTP prober with pre-configured connection pools.
    pub fn new() -> Self {
        let client = Client::builder()
            .pool_idle_timeout(Duration::from_secs(90))
            .pool_max_idle_per_host(10)
            .tcp_nodelay(true)
            .build()
            .unwrap_or_else(|_| Client::new());

        let insecure_client = Client::builder()
            .danger_accept_invalid_certs(true)
            .pool_idle_timeout(Duration::from_secs(90))
            .pool_max_idle_per_host(10)
            .tcp_nodelay(true)
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            client,
            insecure_client,
        }
    }

    /// Execute an HTTP/HTTPS health probe against an endpoint.
    pub async fn probe(&self, endpoint: &EndpointConfig) -> EndpointResult {
        let url_str = match &endpoint.url {
            Some(u) => u,
            None => {
                return EndpointResult::failure(
                    0,
                    Duration::ZERO,
                    format!("Endpoint '{}' has no URL configured", endpoint.name),
                )
            }
        };

        let client = if endpoint.client.as_ref().is_some_and(|c| c.insecure) {
            &self.insecure_client
        } else {
            &self.client
        };

        let method = Method::from_str(&endpoint.method).unwrap_or(Method::GET);
        let mut req = client.request(method, url_str);

        // Apply timeout
        if let Some(timeout) = endpoint.client.as_ref().and_then(|c| c.timeout) {
            req = req.timeout(timeout);
        } else {
            req = req.timeout(Duration::from_secs(10));
        }

        // Apply headers
        if let Some(ref headers_map) = endpoint.headers {
            let mut header_map = HeaderMap::new();
            for (k, v) in headers_map {
                if let (Ok(h_name), Ok(h_val)) = (HeaderName::from_str(k), HeaderValue::from_str(v))
                {
                    header_map.insert(h_name, h_val);
                }
            }
            req = req.headers(header_map);
        }

        // Apply body
        if let Some(ref body_content) = endpoint.body {
            req = req.body(body_content.clone());
        }

        let start = Instant::now();
        let resp_result = req.send().await;
        let duration = start.elapsed();

        match resp_result {
            Ok(resp) => {
                let status_code = resp.status().as_u16();

                // Extract headers into a lowercase map
                let mut resp_headers = HashMap::new();
                for (name, val) in resp.headers() {
                    if let Ok(val_str) = val.to_str() {
                        resp_headers
                            .insert(name.as_str().to_ascii_lowercase(), val_str.to_string());
                    }
                }

                // Read body bytes
                let body_bytes: Bytes = resp.bytes().await.unwrap_or_default();

                // Build evaluation context
                let ctx = EvaluationContext::new(status_code, duration)
                    .with_body(&body_bytes)
                    .with_headers(&resp_headers)
                    .with_connected(true);

                // Evaluate all configured conditions
                let mut all_success = true;
                let mut condition_results = Vec::with_capacity(endpoint.conditions.len());
                let mut errors = Vec::new();

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
                    ip: None,
                    hostname: None,
                }
            }
            Err(err) => {
                let mut condition_results = Vec::new();
                for cond_str in &endpoint.conditions {
                    condition_results.push(ConditionResult {
                        condition: cond_str.clone(),
                        success: false,
                    });
                }
                EndpointResult {
                    timestamp: Utc::now(),
                    success: false,
                    status_code: 0,
                    duration,
                    errors: vec![err.to_string()],
                    condition_results,
                    ip: None,
                    hostname: None,
                }
            }
        }
    }
}
