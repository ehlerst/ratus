//! Alert state machine tracking failure/success thresholds and resolution triggers.

use ratus_core::config::EndpointAlertConfig;
use serde::{Deserialize, Serialize};

/// Current incident status of an alert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertStatus {
    /// Service is healthy; no active incident.
    Resolved,
    /// Failure threshold reached; incident is actively triggered.
    Triggered,
}

/// Tracking state for an individual alert on an endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertState {
    /// Alert configuration definition.
    pub config: EndpointAlertConfig,
    /// Current incident status.
    pub status: AlertStatus,
    /// Number of consecutive probe failures observed.
    pub consecutive_failures: u32,
    /// Number of consecutive probe successes observed.
    pub consecutive_successes: u32,
    /// Total number of incident alerts sent.
    pub total_alerts_sent: u64,
}

/// Action determined by the state machine after evaluating a probe result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlertAction {
    /// Do not dispatch any notification.
    NoOp,
    /// Dispatch an incident notification (threshold reached).
    TriggerIncident,
    /// Dispatch a resolution notification (recovery threshold reached).
    TriggerResolved,
}

impl AlertState {
    /// Create a new alert state tracker from config.
    pub fn new(config: EndpointAlertConfig) -> Self {
        Self {
            config,
            status: AlertStatus::Resolved,
            consecutive_failures: 0,
            consecutive_successes: 0,
            total_alerts_sent: 0,
        }
    }

    /// Update the state machine with a probe outcome and determine if an alert must be sent.
    pub fn update(&mut self, success: bool) -> AlertAction {
        if !self.config.enabled {
            return AlertAction::NoOp;
        }

        if success {
            self.consecutive_successes += 1;
            self.consecutive_failures = 0;

            if self.status == AlertStatus::Triggered
                && self.consecutive_successes >= self.config.success_threshold
            {
                self.status = AlertStatus::Resolved;
                if self.config.send_on_resolved {
                    return AlertAction::TriggerResolved;
                }
            }
            AlertAction::NoOp
        } else {
            self.consecutive_failures += 1;
            self.consecutive_successes = 0;

            if self.status == AlertStatus::Resolved
                && self.consecutive_failures >= self.config.failure_threshold
            {
                self.status = AlertStatus::Triggered;
                self.total_alerts_sent += 1;
                return AlertAction::TriggerIncident;
            }
            AlertAction::NoOp
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alert_lifecycle() {
        let config = EndpointAlertConfig {
            alert_type: "slack".to_string(),
            failure_threshold: 3,
            success_threshold: 2,
            send_on_resolved: true,
            description: Some("Service down".to_string()),
            enabled: true,
        };

        let mut state = AlertState::new(config);

        // 1st failure: no alert yet
        assert_eq!(state.update(false), AlertAction::NoOp);
        // 2nd failure: no alert yet
        assert_eq!(state.update(false), AlertAction::NoOp);
        // 3rd failure: threshold reached!
        assert_eq!(state.update(false), AlertAction::TriggerIncident);
        assert_eq!(state.status, AlertStatus::Triggered);

        // 4th failure: already triggered, no duplicate alert
        assert_eq!(state.update(false), AlertAction::NoOp);

        // 1st recovery: not resolved yet
        assert_eq!(state.update(true), AlertAction::NoOp);
        // 2nd recovery: success threshold reached!
        assert_eq!(state.update(true), AlertAction::TriggerResolved);
        assert_eq!(state.status, AlertStatus::Resolved);
    }
}
