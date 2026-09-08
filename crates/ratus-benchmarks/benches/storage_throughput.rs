use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ratus_core::config::EndpointConfig;
use ratus_core::models::EndpointResult;
use ratus_storage::MemoryStorage;
use std::time::Duration;

fn bench_storage_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("storage_throughput");

    let endpoint = EndpointConfig {
        name: "bench-service".to_string(),
        group: Some("core".to_string()),
        url: Some("https://bench.internal/health".to_string()),
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

    let storage = MemoryStorage::new(100);
    let sample_result = EndpointResult::success(200, Duration::from_millis(45));

    // 1. Result Ingestion Rate
    group.bench_function("result_ingestion", |b| {
        b.iter(|| {
            storage.save_result(black_box(&endpoint), black_box(sample_result.clone()));
        });
    });

    // Seed storage with 100 results for query benchmarking
    for _ in 0..100 {
        storage.save_result(&endpoint, sample_result.clone());
    }

    // 2. Analytical Status & Uptime Query
    let key = endpoint.key();
    group.bench_function("query_uptime_stats", |b| {
        b.iter(|| {
            let status = storage.get_status(black_box(&key)).expect("Status present");
            black_box(status.uptime_percentage());
        });
    });

    group.finish();
}

criterion_group!(benches, bench_storage_throughput);
criterion_main!(benches);
