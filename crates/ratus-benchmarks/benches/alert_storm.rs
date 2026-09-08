use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ratus_alert::state::AlertState;
use ratus_alert::template::interpolate_alert_template;
use ratus_core::config::{EndpointAlertConfig, EndpointConfig};
use ratus_core::models::EndpointResult;
use std::time::Duration;

fn bench_alert_storm(c: &mut Criterion) {
    let mut group = c.benchmark_group("alert_storm");

    let alert_cfg = EndpointAlertConfig {
        alert_type: "slack".to_string(),
        failure_threshold: 3,
        success_threshold: 2,
        send_on_resolved: true,
        description: Some("Database cluster connection failure".to_string()),
        enabled: true,
    };

    // 1. State Machine Evaluation
    group.bench_function("state_machine_transitions", |b| {
        let mut state = AlertState::new(alert_cfg.clone());
        b.iter(|| {
            black_box(state.update(black_box(false)));
            black_box(state.update(black_box(true)));
        });
    });

    // 2. Alert Template Token Interpolation
    let ep = EndpointConfig {
        name: "orders-db".to_string(),
        group: Some("databases".to_string()),
        url: Some("tcp://orders-db:5432".to_string()),
        method: "GET".to_string(),
        body: None,
        headers: None,
        interval: Duration::from_secs(30),
        conditions: vec!["[CONNECTED] == true".to_string()],
        alerts: None,
        client: None,
        ui: None,
        dns: None,
        ssh: None,
        enabled: true,
    };

    let result = EndpointResult::failure(500, Duration::from_millis(120), "TCP connection timeout");
    let template =
        "Alert: [ENDPOINT_NAME] ([ENDPOINT_GROUP]) failed: [ERRORS]. Details: [ALERT_DESCRIPTION]";

    group.bench_function("template_interpolation", |b| {
        b.iter(|| {
            let rendered = interpolate_alert_template(
                black_box(template),
                black_box(&ep),
                black_box(&result),
                black_box(Some("Critical incident")),
            );
            black_box(rendered);
        });
    });

    group.finish();
}

criterion_group!(benches, bench_alert_storm);
criterion_main!(benches);
