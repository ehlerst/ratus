use testcontainers::core::{ContainerPort, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::GenericImage;

#[tokio::test]
async fn test_ratus_container_lifecycle_and_apis() -> anyhow::Result<()> {
    // Start Ratus container using the compiled release Docker image
    let image = GenericImage::new("ehlers320/ratus", "latest")
        .with_wait_for(WaitFor::message_on_stdout("Ratus is operational"))
        .with_exposed_port(ContainerPort::Tcp(8080));

    let container = image.start().await?;
    let host_port = container
        .get_host_port_ipv4(ContainerPort::Tcp(8080))
        .await?;
    let base_url = format!("http://127.0.0.1:{host_port}");

    let client = reqwest::Client::new();

    // 1. Validate /health endpoint
    let health_resp = client.get(format!("{base_url}/health")).send().await?;
    assert!(health_resp.status().is_success());
    let health_body: serde_json::Value = health_resp.json().await?;
    assert_eq!(health_body["status"], "UP");

    // 2. Validate /metrics endpoint
    let metrics_resp = client.get(format!("{base_url}/metrics")).send().await?;
    assert!(metrics_resp.status().is_success());
    let metrics_text = metrics_resp.text().await?;
    assert!(metrics_text.contains("ratus_results_total"));

    // 3. Validate /api/v1/endpoints/statuses
    let statuses_resp = client
        .get(format!("{base_url}/api/v1/endpoints/statuses"))
        .send()
        .await?;
    assert!(statuses_resp.status().is_success());

    // 4. Validate Chaos Injection API in live container
    let chaos_payload = serde_json::json!({
        "endpoint_key": "payment_service",
        "latency_ms": 50,
        "force_status": 503,
        "limit_times": 5
    });
    let chaos_resp = client
        .post(format!("{base_url}/_ratus/chaos/inject"))
        .json(&chaos_payload)
        .send()
        .await?;
    assert!(chaos_resp.status().is_success());

    // 5. Query active chaos rules from container
    let rules_resp = client
        .get(format!("{base_url}/_ratus/chaos/rules"))
        .send()
        .await?;
    assert!(rules_resp.status().is_success());
    let rules: Vec<serde_json::Value> = rules_resp.json().await?;
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0]["endpoint_key"], "payment_service");

    // 6. Reset state
    let reset_resp = client
        .post(format!("{base_url}/_ratus/state/reset"))
        .send()
        .await?;
    assert!(reset_resp.status().is_success());

    println!("✅ Tier 2 Rust Testcontainers passed: container boot, health, metrics, chaos API, and reset all verified.");
    Ok(())
}
