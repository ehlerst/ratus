use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ratus_eval::{parse_condition, EvaluationContext};
use std::time::Duration;

fn bench_condition_eval(c: &mut Criterion) {
    let mut group = c.benchmark_group("condition_eval");

    // 1. Scalar Status Check
    let status_cond = parse_condition("[STATUS] == 200").unwrap();
    let ctx_status = EvaluationContext::new(200, Duration::from_millis(50));
    group.bench_function("scalar_status_check", |b| {
        b.iter(|| {
            black_box(status_cond.evaluate(black_box(&ctx_status)));
        });
    });

    // 2. Response Time Check
    let rt_cond = parse_condition("[RESPONSE_TIME] < 250").unwrap();
    let ctx_rt = EvaluationContext::new(200, Duration::from_millis(150));
    group.bench_function("response_time_check", |b| {
        b.iter(|| {
            black_box(rt_cond.evaluate(black_box(&ctx_rt)));
        });
    });

    // 3. Deep JSONPath Check
    let json_bytes = br#"{"status": "UP", "data": {"code": 42, "items": [1, 2, 3]}}"#;
    let json_cond = parse_condition("[BODY].data.code == 42").unwrap();
    group.bench_function("jsonpath_body_check", |b| {
        let ctx_json = EvaluationContext::new(200, Duration::from_millis(50)).with_body(json_bytes);
        b.iter(|| {
            black_box(json_cond.evaluate(black_box(&ctx_json)));
        });
    });

    // 4. Function Has Check
    let has_cond = parse_condition("has([BODY], \"UP\") == true").unwrap();
    group.bench_function("has_substring_check", |b| {
        let ctx_has = EvaluationContext::new(200, Duration::from_millis(50)).with_body(json_bytes);
        b.iter(|| {
            black_box(has_cond.evaluate(black_box(&ctx_has)));
        });
    });

    group.finish();
}

criterion_group!(benches, bench_condition_eval);
criterion_main!(benches);
