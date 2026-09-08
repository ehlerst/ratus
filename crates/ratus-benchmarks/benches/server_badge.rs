use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ratus_server::generate_badge;

fn bench_badge_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("badge_generation");

    // 1. Dynamic SVG Badge Generation Rate
    group.bench_function("generate_svg_badge", |b| {
        b.iter(|| {
            let svg = generate_badge(black_box("uptime"), black_box("99.98%"), black_box(true));
            black_box(svg);
        });
    });

    group.finish();
}

criterion_group!(benches, bench_badge_generation);
criterion_main!(benches);
