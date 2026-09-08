//! Asynchronous alert dispatcher and notification worker.

use crate::state::{AlertAction, AlertState};
use crate::template::interpolate_alert_template;
use parking_lot::RwLock;
use ratus_core::config::{AlertingConfig, EndpointAlertConfig, EndpointConfig};
use ratus_core::models::EndpointResult;
use reqwest::Client;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info};

/// Structured notification payload sent to alert provider workers.
#[derive(Debug, Clone)]
pub struct AlertNotification {
    /// Provider destination type ("slack", "discord", "custom").
    pub provider_type: String,
    /// Destination webhook URL or endpoint.
    pub target_url: String,
    /// Notification message text.
    pub message: String,
    /// Incident state: true = triggered, false = resolved.
    pub is_incident: bool,
}

/// Central alert dispatcher coordinating state machines and outbound notifications.
#[derive(Clone)]
pub struct AlertDispatcher {
    alert_states: Arc<RwLock<HashMap<String, AlertState>>>,
    alerting_config: Arc<AlertingConfig>,
    tx: mpsc::Sender<AlertNotification>,
}

impl AlertDispatcher {
    /// Create a new alert dispatcher with background dispatch worker.
    pub fn new(config: AlertingConfig) -> Self {
        let (tx, mut rx) = mpsc::channel::<AlertNotification>(1000);
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| Client::new());

        let worker_client = client;
        tokio::spawn(async move {
            while let Some(notif) = rx.recv().await {
                let cl = worker_client.clone();
                tokio::spawn(async move {
                    Self::dispatch_notification(&cl, &notif).await;
                });
            }
        });

        Self {
            alert_states: Arc::new(RwLock::new(HashMap::new())),
            alerting_config: Arc::new(config),
            tx,
        }
    }

    /// Process a probe result against all alerts declared on the endpoint.
    pub async fn process_result(&self, endpoint: &EndpointConfig, result: &EndpointResult) {
        let alerts = match &endpoint.alerts {
            Some(a) if !a.is_empty() => a,
            _ => return,
        };

        for alert_cfg in alerts {
            let state_key = format!("{}_{}", endpoint.key(), alert_cfg.alert_type);
            let action = {
                let mut states = self.alert_states.write();
                let state = states
                    .entry(state_key)
                    .or_insert_with(|| AlertState::new(alert_cfg.clone()));
                state.update(result.success)
            };

            match action {
                AlertAction::TriggerIncident => {
                    self.enqueue_alert(endpoint, result, alert_cfg, true).await;
                }
                AlertAction::TriggerResolved => {
                    self.enqueue_alert(endpoint, result, alert_cfg, false).await;
                }
                AlertAction::NoOp => {}
            }
        }
    }

    async fn enqueue_alert(
        &self,
        endpoint: &EndpointConfig,
        result: &EndpointResult,
        alert_cfg: &EndpointAlertConfig,
        is_incident: bool,
    ) {
        let default_tpl = if is_incident {
            "🔴 **Incident**: [ENDPOINT_NAME] ([ENDPOINT_GROUP]) is DOWN!\nDescription: [ALERT_DESCRIPTION]\nErrors: [ERRORS]"
        } else {
            "🟢 **Resolved**: [ENDPOINT_NAME] ([ENDPOINT_GROUP]) has recovered and is now HEALTHY."
        };

        let message = interpolate_alert_template(
            default_tpl,
            endpoint,
            result,
            alert_cfg.description.as_deref(),
        );

        let (target_url, p_type) = match alert_cfg.alert_type.as_str() {
            "slack" => {
                if let Some(ref slack) = self.alerting_config.slack {
                    (slack.webhook_url.clone(), "slack".to_string())
                } else {
                    return;
                }
            }
            "discord" => {
                if let Some(ref discord) = self.alerting_config.discord {
                    (discord.webhook_url.clone(), "discord".to_string())
                } else {
                    return;
                }
            }
            "custom" => {
                if let Some(ref custom) = self.alerting_config.custom {
                    (custom.url.clone(), "custom".to_string())
                } else {
                    return;
                }
            }
            _ => return,
        };

        let notification = AlertNotification {
            provider_type: p_type,
            target_url,
            message,
            is_incident,
        };

        if let Err(e) = self.tx.send(notification).await {
            error!("Failed to enqueue alert notification: {e}");
        }
    }

    async fn dispatch_notification(client: &Client, notif: &AlertNotification) {
        info!(
            "Dispatching {} alert notification to {}",
            notif.provider_type, notif.target_url
        );

        let payload = match notif.provider_type.as_str() {
            "slack" => json!({ "text": notif.message }),
            "discord" => json!({ "content": notif.message }),
            _ => {
                json!({ "text": notif.message, "status": if notif.is_incident { "down" } else { "up" } })
            }
        };

        match client.post(&notif.target_url).json(&payload).send().await {
            Ok(resp) => {
                if !resp.status().is_success() {
                    error!(
                        "Alert provider returned non-success status: {}",
                        resp.status()
                    );
                }
            }
            Err(err) => {
                error!("Failed to send alert to {}: {err}", notif.target_url);
            }
        }
    }
}
