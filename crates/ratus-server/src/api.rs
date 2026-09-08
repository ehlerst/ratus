//! Axum HTTP API handlers compatible with Gatus and Prometheus.

use crate::badge::generate_badge;
use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::Json;
use ratus_core::models::EndpointStatus;
use ratus_storage::MemoryStorage;
use serde_json::json;
use std::sync::Arc;

/// Shared application state injected into Axum routes.
#[derive(Clone)]
pub struct AppState {
    /// In-memory storage handle.
    pub storage: Arc<MemoryStorage>,
}

const UI_HTML: &str = include_str!("ui/index.html");

/// Handler for the embedded Web dashboard.
pub async fn serve_ui() -> Html<&'static str> {
    Html(UI_HTML)
}

/// Standard health check handler.
pub async fn health() -> Json<serde_json::Value> {
    Json(json!({ "status": "UP" }))
}

/// Handler returning all endpoint statuses for dashboard and external clients.
pub async fn get_all_statuses(State(state): State<AppState>) -> Json<Vec<EndpointStatus>> {
    Json(state.storage.get_all_statuses())
}

/// Handler returning a single endpoint's status.
pub async fn get_endpoint_status(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Json<EndpointStatus>, StatusCode> {
    match state.storage.get_status(&key) {
        Some(status) => Ok(Json(status)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// Handler generating dynamic SVG health/uptime badge.
pub async fn get_endpoint_badge(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Response {
    let status_opt = state.storage.get_status(&key);
    let (label, value, is_success) = match status_opt {
        Some(status) => {
            let uptime = status.uptime_percentage();
            let is_up = match status.latest_result() {
                Some(r) => r.success,
                None => true,
            };
            ("uptime".to_string(), format!("{uptime:.1}%"), is_up)
        }
        None => ("health".to_string(), "unknown".to_string(), false),
    };

    let svg = generate_badge(&label, &value, is_success);

    (
        [
            (header::CONTENT_TYPE, "image/svg+xml"),
            (header::CACHE_CONTROL, "no-cache, no-store, must-revalidate"),
        ],
        svg,
    )
        .into_response()
}

/// Handler exporting Prometheus metrics.
pub async fn get_metrics(State(state): State<AppState>) -> Response {
    let statuses = state.storage.get_all_statuses();
    let mut output = String::new();

    output.push_str("# HELP ratus_results_total Total health check results\n");
    output.push_str("# TYPE ratus_results_total counter\n");
    for s in &statuses {
        let successes = s.results.iter().filter(|r| r.success).count();
        let failures = s.results.len() - successes;
        output.push_str(&format!(
            "ratus_results_total{{key=\"{}\",success=\"true\"}} {}\n",
            s.key, successes
        ));
        output.push_str(&format!(
            "ratus_results_total{{key=\"{}\",success=\"false\"}} {}\n",
            s.key, failures
        ));
    }

    output.push_str("\n# HELP ratus_uptime_percent Rolling uptime percentage\n");
    output.push_str("# TYPE ratus_uptime_percent gauge\n");
    for s in &statuses {
        output.push_str(&format!(
            "ratus_uptime_percent{{key=\"{}\"}} {:.2}\n",
            s.key,
            s.uptime_percentage()
        ));
    }

    (
        [(header::CONTENT_TYPE, "text/plain; version=0.0.4")],
        output,
    )
        .into_response()
}

/// Deterministic test lifecycle: reset state.
pub async fn reset_state(State(state): State<AppState>) -> Json<serde_json::Value> {
    state.storage.reset();
    Json(json!({ "status": "reset_success" }))
}

/// Deterministic test lifecycle: dump state.
pub async fn dump_state(State(state): State<AppState>) -> Response {
    match state.storage.export_state() {
        Ok(json_str) => ([(header::CONTENT_TYPE, "application/json")], json_str).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Export failed: {e}"),
        )
            .into_response(),
    }
}

/// Deterministic test lifecycle: load state.
pub async fn load_state(
    State(state): State<AppState>,
    body: String,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match state.storage.import_state(&body) {
        Ok(_) => Ok(Json(json!({ "status": "load_success" }))),
        Err(_) => Err(StatusCode::BAD_REQUEST),
    }
}
