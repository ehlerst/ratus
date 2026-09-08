//! Ratus Core: Fundamental domain types, configuration schemas, error handling, and models.

#![deny(missing_docs)]
#![deny(clippy::all)]

pub mod config;
pub mod duration;
pub mod env;
pub mod error;
pub mod models;

pub use config::{Config, EndpointConfig};
pub use duration::{format_duration, parse_duration};
pub use env::interpolate_env_vars;
pub use error::{RatusError, Result};
pub use models::{compute_endpoint_key, ConditionResult, EndpointResult, EndpointStatus};
