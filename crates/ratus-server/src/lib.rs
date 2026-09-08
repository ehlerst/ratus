//! Ratus server and API implementation.

#![deny(missing_docs)]
#![deny(clippy::all)]

pub mod api;
pub mod app;
pub mod badge;

pub use api::AppState;
pub use app::{create_router, create_router_with_options, create_router_with_security};
pub use badge::generate_badge;
