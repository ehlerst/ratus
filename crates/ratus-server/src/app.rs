//! Axum application router construction.

use crate::api::{
    dump_state, get_all_statuses, get_chaos_rules, get_endpoint_badge, get_endpoint_status,
    get_metrics, health, inject_chaos_rule, load_state, push_external_result, reset_chaos_rules,
    reset_state, serve_ui, AppState,
};
use axum::extract::Request;
use axum::http::header::{AUTHORIZATION, WWW_AUTHENTICATE};
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use base64::prelude::*;
use ratus_core::config::{BasicAuthConfig, SecurityConfig};
use ratus_prober::ChaosEngine;
use ratus_storage::MemoryStorage;
use std::sync::Arc;

async fn auth_middleware(basic_cfg: Arc<BasicAuthConfig>, req: Request, next: Next) -> Response {
    let path = req.uri().path();
    if path == "/health" {
        return next.run(req).await;
    }

    if let Some(auth_header) = req.headers().get(AUTHORIZATION) {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(encoded) = auth_str.strip_prefix("Basic ") {
                if let Ok(decoded_bytes) = BASE64_STANDARD.decode(encoded.trim()) {
                    if let Ok(decoded_str) = String::from_utf8(decoded_bytes) {
                        if let Some((user, pass)) = decoded_str.split_once(':') {
                            if user == basic_cfg.username && pass == basic_cfg.password {
                                return next.run(req).await;
                            }
                        }
                    }
                }
            }
        }
    }

    (
        StatusCode::UNAUTHORIZED,
        [(
            WWW_AUTHENTICATE,
            HeaderValue::from_static("Basic realm=\"Ratus\""),
        )],
        "Unauthorized",
    )
        .into_response()
}

/// Construct the complete Axum router with all Gatus API routes and test hooks.
pub fn create_router(storage: Arc<MemoryStorage>) -> Router {
    create_router_with_options(storage, Arc::new(ChaosEngine::new()), None)
}

/// Construct the Axum router with optional HTTP Basic authentication enforcement.
pub fn create_router_with_security(
    storage: Arc<MemoryStorage>,
    security: Option<SecurityConfig>,
) -> Router {
    create_router_with_options(storage, Arc::new(ChaosEngine::new()), security)
}

/// Construct the Axum router with custom chaos engine and security configuration.
pub fn create_router_with_options(
    storage: Arc<MemoryStorage>,
    chaos: Arc<ChaosEngine>,
    security: Option<SecurityConfig>,
) -> Router {
    let state = AppState { storage, chaos };

    let mut router = Router::new()
        // Embedded UI and Health
        .route("/", get(serve_ui))
        .route("/health", get(health))
        .route("/metrics", get(get_metrics))
        // Gatus v1 Endpoints API
        .route("/api/v1/endpoints/statuses", get(get_all_statuses))
        .route("/api/v1/endpoints/{key}/statuses", get(get_endpoint_status))
        .route("/api/v1/endpoints/{key}/badge.svg", get(get_endpoint_badge))
        .route(
            "/api/v1/endpoints/{key}/external",
            post(push_external_result),
        )
        // Deterministic State Lifecycle API (Playbook Section 5)
        .route("/_ratus/state/reset", post(reset_state))
        .route("/_ratus/state/dump", get(dump_state))
        .route("/_ratus/state/load", post(load_state))
        // Embedded Chaos Engineering API (Playbook Section 5)
        .route("/_ratus/chaos/inject", post(inject_chaos_rule))
        .route(
            "/_ratus/chaos/rules",
            get(get_chaos_rules)
                .post(inject_chaos_rule)
                .delete(reset_chaos_rules),
        )
        .route("/_ratus/chaos/reset", post(reset_chaos_rules))
        .with_state(state);

    if let Some(sec) = security {
        if let Some(basic) = sec.basic {
            let basic_arc = Arc::new(basic);
            router = router.layer(middleware::from_fn(move |req, next| {
                let cfg = basic_arc.clone();
                auth_middleware(cfg, req, next)
            }));
        }
    }

    router
}
