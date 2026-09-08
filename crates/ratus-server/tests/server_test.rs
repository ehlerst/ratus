use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use ratus_core::config::EndpointConfig;
use ratus_core::models::EndpointResult;
use ratus_server::create_router;
use ratus_storage::MemoryStorage;
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;

#[tokio::test]
async fn test_server_routes_in_memory() {
    let storage = Arc::new(MemoryStorage::new(50));
    let endpoint = EndpointConfig {
        name: "test-service".to_string(),
        group: Some("core".to_string()),
        url: Some("https://example.com".to_string()),
        method: "GET".to_string(),
        body: None,
        headers: None,
        interval: Duration::from_secs(30),
        conditions: vec!["[STATUS] == 200".to_string()],
        alerts: None,
        client: None,
        ui: None,
        dns: None,
        ssh: None,
        enabled: true,
    };

    storage.save_result(
        &endpoint,
        EndpointResult::success(200, Duration::from_millis(45)),
    );

    let app = create_router(storage);

    // 1. Health check
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 2. Embedded UI HTML
    let resp = app
        .clone()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 3. Statuses JSON
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/endpoints/statuses")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json.as_array().unwrap().len(), 1);

    // 4. SVG Badge
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/endpoints/core_test-service/badge.svg")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers().get("content-type").unwrap(), "image/svg+xml");

    // 5. Prometheus metrics
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 6. Push external result
    let push_body = serde_json::json!({
        "success": true,
        "status": 204,
        "duration": 25,
        "errors": []
    });
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/endpoints/cron_worker/external")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&push_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Verify pushed endpoint exists
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/endpoints/cron_worker/statuses")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 7. Reset state
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/_ratus/state/reset")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_basic_auth_protection() {
    use ratus_core::config::{BasicAuthConfig, SecurityConfig};
    use ratus_server::create_router_with_security;

    let storage = Arc::new(MemoryStorage::new(50));
    let security = SecurityConfig {
        basic: Some(BasicAuthConfig {
            username: "admin".to_string(),
            password: "supersecretpassword".to_string(),
        }),
    };

    let app = create_router_with_security(storage, Some(security));

    // 1. Health endpoint should always bypass auth
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 2. Status API without credentials should return 401 Unauthorized
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/endpoints/statuses")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // 3. Status API with invalid credentials should return 401 Unauthorized
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/endpoints/statuses")
                .header("authorization", "Basic d3Jvbmc6cGFzc3dvcmQ=")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // 4. Status API with valid credentials should succeed
    // admin:supersecretpassword in base64 is "YWRtaW46c3VwZXJzZWNyZXRwYXNzd29yZA=="
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/endpoints/statuses")
                .header(
                    "authorization",
                    "Basic YWRtaW46c3VwZXJzZWNyZXRwYXNzd29yZA==",
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_chaos_api_endpoints() {
    use ratus_prober::{ChaosEngine, ChaosRule};
    use ratus_server::create_router_with_options;

    let storage = Arc::new(MemoryStorage::new(50));
    let chaos = Arc::new(ChaosEngine::new());
    let app = create_router_with_options(storage, chaos.clone(), None);

    // 1. Initial rules list should be empty
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/_ratus/chaos/rules")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let rules: Vec<ChaosRule> = serde_json::from_slice(&bytes).unwrap();
    assert!(rules.is_empty());

    // 2. Inject a chaos rule
    let inject_payload = serde_json::json!({
        "endpoint_key": "api_test",
        "latency_ms": 100,
        "jitter_ms": 20,
        "force_status": 503,
        "force_error": "Fault Injected",
        "limit_times": 3
    });

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/_ratus/chaos/inject")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&inject_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 3. Rules list should have 1 rule matching injected values
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/_ratus/chaos/rules")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let rules: Vec<ChaosRule> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].endpoint_key, "api_test");
    assert_eq!(rules[0].force_status, Some(503));
    assert_eq!(rules[0].limit_times, Some(3));

    // 4. Reset chaos rules
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/_ratus/chaos/reset")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 5. Rules should be empty again
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/_ratus/chaos/rules")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let rules: Vec<ChaosRule> = serde_json::from_slice(&bytes).unwrap();
    assert!(rules.is_empty());
}
