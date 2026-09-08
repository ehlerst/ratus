use ratus_alert::AlertDispatcher;
use ratus_core::config::{AlertingConfig, EndpointAlertConfig, EndpointConfig, SlackConfig};
use ratus_core::models::EndpointResult;
use std::time::Duration;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_alert_dispatcher_full_flow() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/slack"))
        .respond_with(ResponseTemplate::new(200))
        .expect(2) // 1 incident + 1 resolved
        .mount(&mock_server)
        .await;

    let alerting = AlertingConfig {
        slack: Some(SlackConfig {
            webhook_url: format!("{}/slack", mock_server.uri()),
        }),
        discord: None,
        telegram: None,
        pagerduty: None,
        custom: None,
    };

    let dispatcher = AlertDispatcher::new(alerting);

    let endpoint = EndpointConfig {
        name: "payment-service".to_string(),
        group: Some("billing".to_string()),
        url: Some("https://pay.internal".to_string()),
        method: "GET".to_string(),
        body: None,
        headers: None,
        interval: Duration::from_secs(30),
        conditions: vec!["[STATUS] == 200".to_string()],
        alerts: Some(vec![EndpointAlertConfig {
            alert_type: "slack".to_string(),
            failure_threshold: 3,
            success_threshold: 2,
            send_on_resolved: true,
            description: Some("Payments failing".to_string()),
            enabled: true,
        }]),
        client: None,
        ui: None,
        dns: None,
        ssh: None,
        enabled: true,
    };

    let fail_res = EndpointResult::failure(500, Duration::from_millis(50), "Internal Error");
    let succ_res = EndpointResult::success(200, Duration::from_millis(30));

    // 2 failures: no alert
    dispatcher.process_result(&endpoint, &fail_res).await;
    dispatcher.process_result(&endpoint, &fail_res).await;

    // 3rd failure: triggers incident!
    dispatcher.process_result(&endpoint, &fail_res).await;

    // 4th failure: duplicate, should not re-trigger
    dispatcher.process_result(&endpoint, &fail_res).await;

    // 1st success: not resolved yet
    dispatcher.process_result(&endpoint, &succ_res).await;

    // 2nd success: resolved! triggers resolution notification
    dispatcher.process_result(&endpoint, &succ_res).await;

    // Allow async worker to dispatch
    tokio::time::sleep(Duration::from_millis(150)).await;

    // MockServer will verify that exactly 2 notifications were sent!
}
