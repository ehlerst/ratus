//! Evaluation context containing response data borrowed directly from the prober.

use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Duration;

/// Borrowed execution context passed to condition evaluators.
pub struct EvaluationContext<'a> {
    /// HTTP status code or protocol exit code (0 if not applicable).
    pub status_code: u16,

    /// Response time duration.
    pub response_time: Duration,

    /// Raw response body bytes.
    pub body: Option<&'a [u8]>,

    /// Response headers map (keys should be lowercase).
    pub headers: Option<&'a HashMap<String, String>>,

    /// Time remaining until TLS certificate expiration.
    pub certificate_expiration: Option<Duration>,

    /// Socket connection status.
    pub connected: bool,

    /// Resolved IP address string.
    pub ip: Option<&'a str>,

    /// DNS response code (e.g. "NOERROR", "NXDOMAIN").
    pub dns_rcode: Option<&'a str>,

    /// Lazily cached parsed JSON value.
    json_cache: RefCell<Option<Option<serde_json::Value>>>,
}

impl<'a> EvaluationContext<'a> {
    /// Create a new evaluation context.
    pub fn new(status_code: u16, response_time: Duration) -> Self {
        Self {
            status_code,
            response_time,
            body: None,
            headers: None,
            certificate_expiration: None,
            connected: true,
            ip: None,
            dns_rcode: None,
            json_cache: RefCell::new(None),
        }
    }

    /// Attach body payload.
    pub fn with_body(mut self, body: &'a [u8]) -> Self {
        self.body = Some(body);
        self
    }

    /// Attach response headers.
    pub fn with_headers(mut self, headers: &'a HashMap<String, String>) -> Self {
        self.headers = Some(headers);
        self
    }

    /// Attach TLS certificate expiration.
    pub fn with_certificate_expiration(mut self, exp: Duration) -> Self {
        self.certificate_expiration = Some(exp);
        self
    }

    /// Attach connected status.
    pub fn with_connected(mut self, connected: bool) -> Self {
        self.connected = connected;
        self
    }

    /// Retrieve or lazily parse the JSON body.
    pub fn json(&self) -> Option<serde_json::Value> {
        let mut cache = self.json_cache.borrow_mut();
        if let Some(ref val) = *cache {
            return val.clone();
        }

        let parsed = self
            .body
            .and_then(|bytes| serde_json::from_slice(bytes).ok());
        *cache = Some(parsed.clone());
        parsed
    }

    /// Get body as UTF-8 string.
    pub fn body_str(&self) -> Option<&str> {
        self.body.and_then(|b| std::str::from_utf8(b).ok())
    }
}
