//! Central probe dispatcher routing endpoints to protocol-specific engines.

use crate::dns::DnsProber;
use crate::http::HttpProber;
use crate::tcp::TcpProber;
use ratus_core::config::EndpointConfig;
use ratus_core::models::EndpointResult;
use std::sync::Arc;

/// Protocol dispatcher selecting the appropriate probe implementation for an endpoint.
#[derive(Clone)]
pub struct ProbeDispatcher {
    http: Arc<HttpProber>,
    tcp: Arc<TcpProber>,
    dns: Arc<DnsProber>,
}

impl Default for ProbeDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl ProbeDispatcher {
    /// Create a new probe dispatcher instance.
    pub fn new() -> Self {
        Self {
            http: Arc::new(HttpProber::new()),
            tcp: Arc::new(TcpProber::new()),
            dns: Arc::new(DnsProber::new()),
        }
    }

    /// Dispatch a probe for an endpoint according to its target protocol.
    pub async fn probe(&self, endpoint: &EndpointConfig) -> EndpointResult {
        if endpoint.dns.is_some() {
            return self.dns.probe(endpoint).await;
        }

        if let Some(ref url) = endpoint.url {
            let lower = url.to_ascii_lowercase();
            if lower.starts_with("tcp://") {
                return self.tcp.probe(endpoint).await;
            }
            if lower.starts_with("dns://") {
                return self.dns.probe(endpoint).await;
            }
        }

        // Default to HTTP/HTTPS
        self.http.probe(endpoint).await
    }
}
