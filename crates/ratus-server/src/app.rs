//! Axum application router construction.

use crate::api::{
    dump_state, get_all_statuses, get_endpoint_badge, get_endpoint_status, get_metrics, health,
    load_state, reset_state, serve_ui, AppState,
};
use axum::routing::{get, post};
use axum::Router;
use ratus_storage::MemoryStorage;
use std::sync::Arc;

/// Construct the complete Axum router with all Gatus API routes and test hooks.
pub fn create_router(storage: Arc<MemoryStorage>) -> Router {
    let state = AppState { storage };

    Router::new()
        // Embedded UI and Health
        .route("/", get(serve_ui))
        .route("/health", get(health))
        .route("/metrics", get(get_metrics))
        // Gatus v1 Endpoints API
        .route("/api/v1/endpoints/statuses", get(get_all_statuses))
        .route("/api/v1/endpoints/{key}/statuses", get(get_endpoint_status))
        .route("/api/v1/endpoints/{key}/badge.svg", get(get_endpoint_badge))
        // Deterministic State Lifecycle API (Playbook Section 5)
        .route("/_ratus/state/reset", post(reset_state))
        .route("/_ratus/state/dump", get(dump_state))
        .route("/_ratus/state/load", post(load_state))
        .with_state(state)
}
