//! Axum HTTP API handlers compatible with Gatus and Prometheus.

use crate::badge::generate_badge;
use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::Json;
use ratus_core::models::EndpointStatus;
use ratus_prober::{ChaosEngine, ChaosRule};
use ratus_storage::MemoryStorage;
use serde_json::json;
use std::sync::Arc;

/// Shared application state injected into Axum routes.
#[derive(Clone)]
pub struct AppState {
    /// In-memory storage handle.
    pub storage: Arc<MemoryStorage>,
    /// Chaos engine handle for fault injection testing.
    pub chaos: Arc<ChaosEngine>,
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

/// Payload accepted by the external probe push endpoint (`POST /api/v1/endpoints/{key}/external`).
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ExternalPushPayload {
    /// Whether the external probe succeeded.
    pub success: bool,
    /// HTTP status code or exit code.
    #[serde(default)]
    pub status: u16,
    /// Probe execution duration in milliseconds or nanoseconds.
    #[serde(default)]
    pub duration: u64,
    /// Optional errors encountered during probe.
    #[serde(default)]
    pub errors: Vec<String>,
}

/// Handler for external probe push ingestion (`POST /api/v1/endpoints/{key}/external`).
pub async fn push_external_result(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(payload): Json<ExternalPushPayload>,
) -> (StatusCode, Json<serde_json::Value>) {
    let dur = if payload.duration > 1_000_000 {
        std::time::Duration::from_nanos(payload.duration)
    } else {
        std::time::Duration::from_millis(payload.duration)
    };

    let result = ratus_core::models::EndpointResult {
        timestamp: chrono::Utc::now(),
        success: payload.success,
        status_code: payload.status,
        duration: dur,
        errors: payload.errors,
        condition_results: Vec::new(),
        ip: None,
        hostname: None,
    };

    let (group, name) = if let Some((g, n)) = key.split_once('_') {
        (Some(g), n)
    } else {
        (None, key.as_str())
    };

    state.storage.save_result_by_key(&key, name, group, result);

    (StatusCode::OK, Json(json!({ "status": "success" })))
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

/// Helper function generating standard OpenTelemetry (OTLP/HTTP) JSON metric payload.
pub fn generate_otlp_metrics(storage: &MemoryStorage, service_name: &str) -> serde_json::Value {
    let statuses = storage.get_all_statuses();
    let now_nano = chrono::Utc::now()
        .timestamp_nanos_opt()
        .unwrap_or(0)
        .to_string();

    let mut status_points = Vec::new();
    let mut duration_points = Vec::new();
    let mut uptime_points = Vec::new();

    for s in &statuses {
        let is_up = s.latest_result().map(|r| r.success).unwrap_or(true);
        let status_val = if is_up { 1 } else { 0 };
        let uptime = s.uptime_percentage();
        let last_dur_ms = s
            .latest_result()
            .map(|r| r.duration.as_secs_f64() * 1000.0)
            .unwrap_or(0.0);

        let attrs = json!([
            { "key": "endpoint.key", "value": { "stringValue": s.key } },
            { "key": "endpoint.name", "value": { "stringValue": s.name } },
            { "key": "endpoint.group", "value": { "stringValue": s.group.as_deref().unwrap_or("default") } }
        ]);

        status_points.push(json!({
            "timeUnixNano": now_nano,
            "asInt": status_val,
            "attributes": attrs
        }));

        duration_points.push(json!({
            "timeUnixNano": now_nano,
            "asDouble": last_dur_ms,
            "attributes": attrs
        }));

        uptime_points.push(json!({
            "timeUnixNano": now_nano,
            "asDouble": uptime,
            "attributes": attrs
        }));
    }

    json!({
        "resourceMetrics": [
            {
                "resource": {
                    "attributes": [
                        { "key": "service.name", "value": { "stringValue": service_name } },
                        { "key": "service.version", "value": { "stringValue": env!("CARGO_PKG_VERSION") } }
                    ]
                },
                "scopeMetrics": [
                    {
                        "scope": { "name": "ratus.prober", "version": env!("CARGO_PKG_VERSION") },
                        "metrics": [
                            {
                                "name": "ratus.endpoint.status",
                                "description": "Endpoint operational health status (1 = UP, 0 = DOWN)",
                                "unit": "1",
                                "gauge": { "dataPoints": status_points }
                            },
                            {
                                "name": "ratus.endpoint.duration_ms",
                                "description": "Latest endpoint probe response duration in milliseconds",
                                "unit": "ms",
                                "gauge": { "dataPoints": duration_points }
                            },
                            {
                                "name": "ratus.endpoint.uptime_percentage",
                                "description": "Rolling uptime availability percentage (0-100)",
                                "unit": "%",
                                "gauge": { "dataPoints": uptime_points }
                            }
                        ]
                    }
                ]
            }
        ]
    })
}

/// Handler exporting OpenTelemetry (OTLP/HTTP JSON) metrics.
pub async fn get_otel_metrics(State(state): State<AppState>) -> Response {
    let otlp = generate_otlp_metrics(&state.storage, "ratus");
    ([(header::CONTENT_TYPE, "application/json")], Json(otlp)).into_response()
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

/// Inject or update a chaos failure/latency simulation rule.
pub async fn inject_chaos_rule(
    State(state): State<AppState>,
    Json(rule): Json<ChaosRule>,
) -> Json<serde_json::Value> {
    let key = rule.endpoint_key.clone();
    state.chaos.add_rule(rule);
    Json(json!({ "status": "rule_added", "endpoint_key": key }))
}

/// Retrieve all active chaos simulation rules.
pub async fn get_chaos_rules(State(state): State<AppState>) -> Json<Vec<ChaosRule>> {
    Json(state.chaos.get_rules())
}

/// Reset and clear all active chaos simulation rules.
pub async fn reset_chaos_rules(State(state): State<AppState>) -> Json<serde_json::Value> {
    state.chaos.clear();
    Json(json!({ "status": "chaos_cleared" }))
}
