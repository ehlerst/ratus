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

    // 6. Reset state
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
