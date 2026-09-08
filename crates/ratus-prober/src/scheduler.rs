//! Background asynchronous scheduler triggering probes on periodic intervals.

use crate::dispatcher::ProbeDispatcher;
use ratus_core::config::{Config, EndpointConfig};
use ratus_core::models::EndpointResult;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::time::sleep;
use tracing::{error, info, trace};

/// Notification payload emitted whenever an endpoint probe completes.
#[derive(Debug, Clone)]
pub struct ProbeEvent {
    /// Endpoint configuration snapshot.
    pub endpoint: EndpointConfig,
    /// Probe execution result.
    pub result: EndpointResult,
}

/// Periodic probe scheduler running background tasks per endpoint.
pub struct Scheduler {
    dispatcher: ProbeDispatcher,
    event_tx: mpsc::Sender<ProbeEvent>,
    shutdown_tx: broadcast::Sender<()>,
}

impl Scheduler {
    /// Create a new scheduler with an event sender.
    pub fn new(event_tx: mpsc::Sender<ProbeEvent>) -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);
        Self {
            dispatcher: ProbeDispatcher::new(),
            event_tx,
            shutdown_tx,
        }
    }

    /// Attach a custom shared chaos engine instance to this scheduler.
    pub fn with_chaos(mut self, chaos: std::sync::Arc<crate::chaos::ChaosEngine>) -> Self {
        self.dispatcher = self.dispatcher.with_chaos(chaos);
        self
    }

    /// Access the underlying chaos engine.
    pub fn chaos(&self) -> std::sync::Arc<crate::chaos::ChaosEngine> {
        self.dispatcher.chaos()
    }

    /// Start polling all enabled endpoints in the background.
    pub fn start(&self, config: &Config) {
        info!(
            "Starting probe scheduler for {} endpoints",
            config.endpoints.len()
        );

        for endpoint in &config.endpoints {
            if !endpoint.enabled {
                continue;
            }

            let ep = endpoint.clone();
            let dispatcher = self.dispatcher.clone();
            let tx = self.event_tx.clone();
            let mut shutdown_rx = self.shutdown_tx.subscribe();

            tokio::spawn(async move {
                let interval = ep.interval;
                trace!(
                    "Spawned prober worker for '{}' with interval {:?}",
                    ep.name,
                    interval
                );

                loop {
                    // Execute probe
                    let result = dispatcher.probe(&ep).await;

                    if tx
                        .send(ProbeEvent {
                            endpoint: ep.clone(),
                            result,
                        })
                        .await
                        .is_err()
                    {
                        trace!(
                            "Scheduler event receiver closed, stopping worker for '{}'",
                            ep.name
                        );
                        break;
                    }

                    // Wait for interval or shutdown signal
                    tokio::select! {
                        _ = sleep(interval) => {}
                        _ = shutdown_rx.recv() => {
                            info!("Prober worker for '{}' received shutdown signal", ep.name);
                            break;
                        }
                    }
                }
            });
        }
    }

    /// Trigger graceful shutdown of all prober workers.
    pub fn shutdown(&self) {
        if let Err(e) = self.shutdown_tx.send(()) {
            error!("Failed to broadcast shutdown to probers: {e}");
        }
    }
}
