use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use ratus_alert::AlertDispatcher;
use ratus_core::config::{AlertingConfig, EndpointConfig, SlackConfig};
use ratus_prober::{ProbeDispatcher, ProbeEvent};
use ratus_server::create_router;
use ratus_storage::MemoryStorage;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tower::ServiceExt;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_full_stack_end_to_end_in_memory() {
    // 1. Setup mock upstream HTTP service
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/health"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "UP",
            "version": "1.0.0"
        })))
        .mount(&mock_server)
        .await;

    // 2. Setup mock Slack webhook
    let slack_server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&slack_server)
        .await;

    // 3. Initialize storage and alerting
    let storage = Arc::new(MemoryStorage::new(50));
    let alerting = AlertingConfig {
        slack: Some(SlackConfig {
            webhook_url: format!("{}/slack", slack_server.uri()),
        }),
        discord: None,
        telegram: None,
        pagerduty: None,
        custom: None,
    };
    let alerts = AlertDispatcher::new(alerting);

    // 4. Create probe event pipeline
    let (tx, mut rx) = mpsc::channel::<ProbeEvent>(100);
    let loop_storage = storage.clone();
    let loop_alerts = alerts.clone();
    tokio::spawn(async move {
        while let Some(ev) = rx.recv().await {
            loop_storage.save_result(&ev.endpoint, ev.result.clone());
            loop_alerts.process_result(&ev.endpoint, &ev.result).await;
        }
    });

    // 5. Define endpoints
    let healthy_ep = EndpointConfig {
        name: "upstream-api".to_string(),
        group: Some("microservices".to_string()),
        url: Some(format!("{}/api/health", mock_server.uri())),
        method: "GET".to_string(),
        body: None,
        headers: None,
        interval: Duration::from_secs(10),
        conditions: vec![
            "[STATUS] == 200".to_string(),
            "[BODY].status == UP".to_string(),
            "[RESPONSE_TIME] < 500".to_string(),
        ],
        alerts: None,
        client: None,
        ui: None,
        dns: None,
        ssh: None,
        enabled: true,
    };

    // 6. Execute probe
    let dispatcher = ProbeDispatcher::new();
    let result = dispatcher.probe(&healthy_ep).await;
    assert!(result.success);
    assert_eq!(result.status_code, 200);

    tx.send(ProbeEvent {
        endpoint: healthy_ep.clone(),
        result,
    })
    .await
    .unwrap();

    // Allow event queue to settle
    tokio::time::sleep(Duration::from_millis(50)).await;

    // 7. Verify API routes
    let app = create_router(storage);

    // Query all statuses
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
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["name"], "upstream-api");

    // Query SVG badge
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/endpoints/microservices_upstream-api/badge.svg")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let svg_bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let svg = String::from_utf8(svg_bytes.to_vec()).unwrap();
    assert!(svg.contains("uptime"));
    assert!(svg.contains("100.0%"));

    // Verify state export
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/_ratus/state/dump")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Verify state reset
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
