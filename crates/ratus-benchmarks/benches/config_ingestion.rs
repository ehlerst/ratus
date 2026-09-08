use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ratus_benchmarks::fixtures::generate_yaml_config;
use ratus_core::Config;

fn bench_config_ingestion(c: &mut Criterion) {
    let mut group = c.benchmark_group("config_ingestion");

    for count in [10, 100, 1000] {
        let yaml = generate_yaml_config(count);
        group.bench_with_input(BenchmarkId::new("endpoints", count), &yaml, |b, data| {
            b.iter(|| {
                let cfg = Config::from_yaml_str(black_box(data)).expect("Valid config");
                black_box(cfg);
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_config_ingestion);
criterion_main!(benches);
