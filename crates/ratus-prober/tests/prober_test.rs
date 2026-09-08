use ratus_core::config::EndpointConfig;
use ratus_prober::dispatcher::ProbeDispatcher;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_http_probe_success() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/healthz"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "UP",
            "code": 42
        })))
        .mount(&mock_server)
        .await;

    let mut endpoint = EndpointConfig {
        name: "test-service".to_string(),
        group: Some("core".to_string()),
        url: Some(format!("{}/healthz", mock_server.uri())),
        method: "GET".to_string(),
        body: None,
        headers: None,
        interval: std::time::Duration::from_secs(30),
        conditions: vec![
            "[STATUS] == 200".to_string(),
            "[BODY].status == UP".to_string(),
            "[BODY].code == 42".to_string(),
        ],
        alerts: None,
        client: None,
        ui: None,
        dns: None,
        ssh: None,
        enabled: true,
    };

    let dispatcher = ProbeDispatcher::new();
    let result = dispatcher.probe(&endpoint).await;

    assert!(result.success);
    assert_eq!(result.status_code, 200);
    assert_eq!(result.condition_results.len(), 3);
    assert!(result.condition_results.iter().all(|c| c.success));

    // Test condition failure
    endpoint.conditions.push("[STATUS] == 500".to_string());
    let failed_result = dispatcher.probe(&endpoint).await;
    assert!(!failed_result.success);
}

#[tokio::test]
async fn test_tcp_probe_local() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let local_addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        if let Ok((_socket, _)) = listener.accept().await {
            // connection established
        }
    });

    let endpoint = EndpointConfig {
        name: "tcp-service".to_string(),
        group: None,
        url: Some(format!("tcp://{}", local_addr)),
        method: "GET".to_string(),
        body: None,
        headers: None,
        interval: std::time::Duration::from_secs(30),
        conditions: vec!["[CONNECTED] == true".to_string()],
        alerts: None,
        client: None,
        ui: None,
        dns: None,
        ssh: None,
        enabled: true,
    };

    let dispatcher = ProbeDispatcher::new();
    let result = dispatcher.probe(&endpoint).await;

    assert!(result.success);
    assert_eq!(result.status_code, 200);
}
