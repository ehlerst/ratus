//! In-memory bounded ring-buffer storage implementation.

use crate::ring_buffer::RingBuffer;
use parking_lot::RwLock;
use ratus_core::config::EndpointConfig;
use ratus_core::error::{RatusError, Result};
use ratus_core::models::{EndpointResult, EndpointStatus};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Default historical samples kept per endpoint for UI rendering (e.g. 50 recent probes).
pub const DEFAULT_HISTORY_CAPACITY: usize = 50;

/// Internal state container for an individual endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointState {
    /// Endpoint human-readable name.
    pub name: String,
    /// Optional grouping category.
    pub group: Option<String>,
    /// Unique URL-safe identifier.
    pub key: String,
    /// Bounded ring buffer of recent probe results.
    pub results: RingBuffer<EndpointResult>,
    /// Lifetime total probe runs.
    pub total_runs: u64,
    /// Lifetime successful probe runs.
    pub successful_runs: u64,
    /// Consecutive successes count for resolving alerts.
    pub consecutive_successes: u32,
    /// Consecutive failures count for triggering alerts.
    pub consecutive_failures: u32,
}

impl EndpointState {
    /// Create a new endpoint state container.
    pub fn new(name: String, group: Option<String>, key: String, capacity: usize) -> Self {
        Self {
            name,
            group,
            key,
            results: RingBuffer::new(capacity),
            total_runs: 0,
            successful_runs: 0,
            consecutive_successes: 0,
            consecutive_failures: 0,
        }
    }

    /// Record a probe result and update counters.
    pub fn record_result(&mut self, result: EndpointResult) {
        self.total_runs += 1;
        if result.success {
            self.successful_runs += 1;
            self.consecutive_successes += 1;
            self.consecutive_failures = 0;
        } else {
            self.consecutive_failures += 1;
            self.consecutive_successes = 0;
        }
        self.results.push(result);
    }

    /// Convert to public `EndpointStatus` for API/UI delivery.
    pub fn to_status(&self) -> EndpointStatus {
        EndpointStatus {
            name: self.name.clone(),
            group: self.group.clone(),
            key: self.key.clone(),
            results: self.results.to_vec(),
        }
    }
}

/// In-memory storage backed by fine-grained read-write locks.
#[derive(Clone)]
pub struct MemoryStorage {
    endpoints: Arc<RwLock<HashMap<String, EndpointState>>>,
    capacity: usize,
}

impl Default for MemoryStorage {
    fn default() -> Self {
        Self::new(DEFAULT_HISTORY_CAPACITY)
    }
}

impl MemoryStorage {
    /// Create a new in-memory storage engine.
    pub fn new(capacity: usize) -> Self {
        Self {
            endpoints: Arc::new(RwLock::new(HashMap::new())),
            capacity,
        }
    }

    /// Save a probe result for an endpoint.
    pub fn save_result(&self, endpoint: &EndpointConfig, result: EndpointResult) {
        let key = endpoint.key();
        let mut map = self.endpoints.write();
        let state = map.entry(key.clone()).or_insert_with(|| {
            EndpointState::new(
                endpoint.name.clone(),
                endpoint.group.clone(),
                key,
                self.capacity,
            )
        });
        state.record_result(result);
    }

    /// Get current live status for an endpoint key.
    pub fn get_status(&self, key: &str) -> Option<EndpointStatus> {
        let map = self.endpoints.read();
        map.get(key).map(|s| s.to_status())
    }

    /// Get all endpoint statuses for dashboard and API.
    pub fn get_all_statuses(&self) -> Vec<EndpointStatus> {
        let map = self.endpoints.read();
        map.values().map(|s| s.to_status()).collect()
    }

    /// Get raw internal state for alert evaluation.
    pub fn get_endpoint_state(&self, key: &str) -> Option<EndpointState> {
        let map = self.endpoints.read();
        map.get(key).cloned()
    }

    /// Reset all endpoint states (Playbook Section 5: Deterministic test lifecycle).
    pub fn reset(&self) {
        let mut map = self.endpoints.write();
        map.clear();
    }

    /// Export complete cluster state snapshot to JSON string.
    pub fn export_state(&self) -> Result<String> {
        let map = self.endpoints.read();
        serde_json::to_string(&*map).map_err(RatusError::Json)
    }

    /// Hydrate complete service state from a snapshot JSON string.
    pub fn import_state(&self, json: &str) -> Result<()> {
        let imported: HashMap<String, EndpointState> =
            serde_json::from_str(json).map_err(RatusError::Json)?;
        let mut map = self.endpoints.write();
        *map = imported;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_memory_storage_lifecycle() {
        let storage = MemoryStorage::new(5);
        let endpoint = EndpointConfig {
            name: "auth-svc".to_string(),
            group: Some("core".to_string()),
            url: Some("https://auth.internal".to_string()),
            method: "GET".to_string(),
            body: None,
            headers: None,
            interval: Duration::from_secs(30),
            conditions: vec!["[STATUS] == 200".to_string()],
            alerts: None,
            client: None,
            ui: None,
            dns: None,
            ssh: None,
            enabled: true,
        };

        storage.save_result(
            &endpoint,
            EndpointResult::success(200, Duration::from_millis(50)),
        );
        storage.save_result(
            &endpoint,
            EndpointResult::failure(500, Duration::from_millis(50), "Error"),
        );

        let status = storage
            .get_status(&endpoint.key())
            .expect("Should have status");
        assert_eq!(status.name, "auth-svc");
        assert_eq!(status.results.len(), 2);
        assert_eq!(status.uptime_percentage(), 50.0);

        // Test export and import
        let exported = storage.export_state().expect("Should export JSON");
        storage.reset();
        assert!(storage.get_status(&endpoint.key()).is_none());

        storage.import_state(&exported).expect("Should import JSON");
        assert!(storage.get_status(&endpoint.key()).is_some());
    }
}
