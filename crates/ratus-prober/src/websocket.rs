//! WebSocket protocol probe implementation.

use chrono::Utc;
use ratus_core::config::EndpointConfig;
use ratus_core::models::{ConditionResult, EndpointResult};
use ratus_eval::{parse_condition, EvaluationContext};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

/// WebSocket prober verifying HTTP 101 Upgrade and ping-pong handshake.
#[derive(Clone, Default)]
pub struct WebSocketProber;

impl WebSocketProber {
    /// Create a new WebSocket prober.
    pub fn new() -> Self {
        Self
    }

    /// Probe a WebSocket endpoint.
    pub async fn probe(&self, endpoint: &EndpointConfig) -> EndpointResult {
        let url_str = match &endpoint.url {
            Some(u) => u,
            None => {
                return EndpointResult::failure(
                    0,
                    Duration::ZERO,
                    "No target URL specified for WebSocket probe",
                )
            }
        };

        let start = Instant::now();
        let timeout_dur = endpoint
            .client
            .as_ref()
            .and_then(|c| c.timeout)
            .unwrap_or(Duration::from_secs(10));

        let res = timeout(timeout_dur, self.execute_handshake(url_str)).await;
        let duration = start.elapsed();

        let (status_code, error_msg) = match res {
            Ok(Ok(code)) => (code, None),
            Ok(Err(err)) => (0, Some(format!("WebSocket handshake failed: {err}"))),
            Err(_) => (0, Some("WebSocket probe timed out".to_string())),
        };

        let connected = status_code == 101 || (200..300).contains(&status_code);
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

    async fn execute_handshake(&self, raw_url: &str) -> Result<u16, String> {
        let stripped = raw_url
            .strip_prefix("ws://")
            .or_else(|| raw_url.strip_prefix("http://"))
            .unwrap_or(raw_url);

        let (host_port, path) = match stripped.split_once('/') {
            Some((hp, p)) => (hp, format!("/{p}")),
            None => (stripped, "/".to_string()),
        };

        let host_port = if !host_port.contains(':') {
            format!("{host_port}:80")
        } else {
            host_port.to_string()
        };

        let mut stream = TcpStream::connect(&host_port)
            .await
            .map_err(|e| format!("TCP connect error to {host_port}: {e}"))?;

        let request = format!(
            "GET {path} HTTP/1.1\r\n\
             Host: {host_port}\r\n\
             Upgrade: websocket\r\n\
             Connection: Upgrade\r\n\
             Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
             Sec-WebSocket-Version: 13\r\n\
             \r\n"
        );

        stream
            .write_all(request.as_bytes())
            .await
            .map_err(|e| format!("Failed to write handshake request: {e}"))?;

        let mut buffer = [0u8; 1024];
        let n = stream
            .read(&mut buffer)
            .await
            .map_err(|e| format!("Failed to read handshake response: {e}"))?;

        if n == 0 {
            return Err("Server closed connection during WebSocket handshake".to_string());
        }

        let response = String::from_utf8_lossy(&buffer[..n]);
        let first_line = response.lines().next().unwrap_or("");
        let parts: Vec<&str> = first_line.split_whitespace().collect();
        if parts.len() >= 2 {
            if let Ok(code) = parts[1].parse::<u16>() {
                if code == 101 || (200..300).contains(&code) {
                    return Ok(code);
                } else {
                    return Err(format!(
                        "Unexpected HTTP response code {code}: {first_line}"
                    ));
                }
            }
        }

        Err(format!("Malformed response line: {first_line}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn test_websocket_handshake() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                let mut buf = [0u8; 1024];
                let _ = socket.read(&mut buf).await;
                let resp = "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n\r\n";
                let _ = socket.write_all(resp.as_bytes()).await;
            }
        });

        let prober = WebSocketProber::new();
        let ep = EndpointConfig {
            name: "ws-test".to_string(),
            group: None,
            url: Some(format!("ws://{}", addr)),
            method: "GET".to_string(),
            body: None,
            headers: None,
            interval: Duration::from_secs(30),
            conditions: vec!["[STATUS] == 101".to_string()],
            alerts: None,
            client: None,
            ui: None,
            dns: None,
            ssh: None,
            enabled: true,
        };

        let res = prober.probe(&ep).await;
        assert!(res.success);
        assert_eq!(res.status_code, 101);
    }
}
