//! Central probe dispatcher routing endpoints to protocol-specific engines.

use crate::chaos::ChaosEngine;
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
    chaos: Arc<ChaosEngine>,
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
            chaos: Arc::new(ChaosEngine::new()),
        }
    }

    /// Attach a custom shared chaos engine instance.
    pub fn with_chaos(mut self, chaos: Arc<ChaosEngine>) -> Self {
        self.chaos = chaos;
        self
    }

    /// Access the underlying chaos engine.
    pub fn chaos(&self) -> Arc<ChaosEngine> {
        self.chaos.clone()
    }

    /// Dispatch a probe for an endpoint according to its target protocol.
    pub async fn probe(&self, endpoint: &EndpointConfig) -> EndpointResult {
        let mut result = if endpoint.dns.is_some() {
            self.dns.probe(endpoint).await
        } else if let Some(ref url) = endpoint.url {
            let lower = url.to_ascii_lowercase();
            if lower.starts_with("tcp://") {
                self.tcp.probe(endpoint).await
            } else if lower.starts_with("dns://") {
                self.dns.probe(endpoint).await
            } else {
                self.http.probe(endpoint).await
            }
        } else {
            self.http.probe(endpoint).await
        };

        self.chaos.apply(&endpoint.key(), &mut result).await;
        result
    }
}
