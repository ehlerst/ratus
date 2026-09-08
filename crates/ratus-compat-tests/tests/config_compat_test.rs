use ratus_core::Config;
use std::time::Duration;

const REAL_WORLD_GATUS_YAML: &str = r#"
metrics: true
ui:
  title: "Service Health Dashboard"
  description: "Live status of production microservices"
  header: "Acme Production Operations"
  buttons:
    - name: "Status Page"
      link: "https://status.example.com"

storage:
  type: memory

alerting:
  slack:
    webhook-url: "https://hooks.slack.com/services/T000/B000/XXXX"
  discord:
    webhook-url: "https://discord.com/api/webhooks/000/XXXX"

endpoints:
  - name: core-auth-api
    group: authentication
    url: "https://auth.example.com/healthz"
    interval: 15s
    conditions:
      - "[STATUS] == 200"
      - "[RESPONSE_TIME] < 200"
      - "[CERTIFICATE_EXPIRATION] > 48h"
    alerts:
      - type: slack
        failure-threshold: 3
        success-threshold: 2
        send-on-resolved: true
        description: "Auth service is experiencing errors"

  - name: billing-service
    group: financial
    url: "https://billing.example.com/api/v1/ping"
    interval: 1m
    conditions:
      - "[STATUS] == 200"
      - "[BODY].status == OK"

  - name: database-primary
    group: data-tier
    url: "tcp://db.example.internal:5432"
    interval: 30s
    conditions:
      - "[CONNECTED] == true"
"#;

#[test]
fn test_parse_real_world_gatus_config() {
    let config = Config::from_yaml_str(REAL_WORLD_GATUS_YAML)
        .expect("Should parse real-world Gatus YAML successfully");

    assert!(config.metrics);
    assert_eq!(config.endpoints.len(), 3);

    // Verify first endpoint
    let ep1 = &config.endpoints[0];
    assert_eq!(ep1.name, "core-auth-api");
    assert_eq!(ep1.group.as_deref(), Some("authentication"));
    assert_eq!(ep1.interval, Duration::from_secs(15));
    assert_eq!(ep1.conditions.len(), 3);
    assert_eq!(ep1.key(), "authentication_core-auth-api");

    let alerts = ep1.alerts.as_ref().unwrap();
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].alert_type, "slack");
    assert_eq!(alerts[0].failure_threshold, 3);
    assert_eq!(alerts[0].success_threshold, 2);
    assert!(alerts[0].send_on_resolved);

    // Verify UI
    let ui = config.ui.as_ref().unwrap();
    assert_eq!(ui.title.as_deref(), Some("Service Health Dashboard"));
    assert_eq!(ui.buttons.as_ref().unwrap().len(), 1);
}

#[test]
fn test_endpoint_validation_failures() {
    // Missing conditions
    let invalid_yaml = r#"
endpoints:
  - name: bad-endpoint
    url: "https://example.com"
"#;
    assert!(Config::from_yaml_str(invalid_yaml).is_err());

    // Zero interval
    let zero_interval_yaml = r#"
endpoints:
  - name: bad-endpoint
    url: "https://example.com"
    interval: 0s
    conditions: ["[STATUS] == 200"]
"#;
    assert!(Config::from_yaml_str(zero_interval_yaml).is_err());
}
