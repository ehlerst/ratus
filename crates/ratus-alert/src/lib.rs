//! Resilient alerting engine and notification providers for Ratus.

#![deny(missing_docs)]
#![deny(clippy::all)]

pub mod dispatcher;
pub mod state;
pub mod template;

pub use dispatcher::{AlertDispatcher, AlertNotification};
pub use state::{AlertAction, AlertState, AlertStatus};
pub use template::interpolate_alert_template;
