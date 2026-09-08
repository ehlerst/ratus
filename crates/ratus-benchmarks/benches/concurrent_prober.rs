use criterion::{criterion_group, criterion_main, Criterion};
use ratus_core::config::EndpointConfig;
use ratus_prober::dispatcher::ProbeDispatcher;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

fn bench_concurrent_probes(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mock_server = rt.block_on(async {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string("{\"status\":\"UP\"}"))
            .mount(&server)
            .await;
        server
    });

    let uri = mock_server.uri();
    let dispatcher = Arc::new(ProbeDispatcher::new());

    let mut group = c.benchmark_group("concurrent_prober");

    for count in [50, 200] {
        let mut endpoints = Vec::with_capacity(count);
        for i in 0..count {
            endpoints.push(EndpointConfig {
                name: format!("ep-{i}"),
                group: Some("cluster".to_string()),
                url: Some(format!("{uri}/ping")),
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
            });
        }

        let endpoints = Arc::new(endpoints);
        let disp = dispatcher.clone();

        group.bench_function(format!("concurrent_{count}_probes"), |b| {
            b.to_async(&rt).iter(|| {
                let eps = endpoints.clone();
                let d = disp.clone();
                async move {
                    let mut handles = Vec::with_capacity(eps.len());
                    for ep in eps.iter() {
                        let ep_cloned = ep.clone();
                        let d_cloned = d.clone();
                        handles.push(tokio::spawn(
                            async move { d_cloned.probe(&ep_cloned).await },
                        ));
                    }
                    for h in handles {
                        let _ = h.await;
                    }
                }
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_concurrent_probes);
criterion_main!(benches);
