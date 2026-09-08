#!/usr/bin/env bash
set -euo pipefail

echo "================================================================================"
echo "🦀 Project Ratus vs TwiN/Gatus: Head-to-Head Benchmark Suite"
echo "================================================================================"

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "[1/4] Compiling Ratus in release mode..."
cargo build --release --workspace

echo ""
echo "[2/4] Running Pure Rust Micro-Benchmarks (Criterion)..."
cargo bench -p ratus-benchmarks --bench config_ingestion -- --sample-size 10
cargo bench -p ratus-benchmarks --bench condition_eval -- --sample-size 10
cargo bench -p ratus-benchmarks --bench server_badge -- --sample-size 10

echo ""
echo "[3/4] Running Head-to-Head Config Ingestion & Startup Comparison..."
if command -v go >/dev/null 2>&1; then
    echo "--- Go Baseline (yaml_bench_test.go) ---"
    (cd Testcontainers/go-testcontainers && go test -bench=BenchmarkYAML -benchmem)
fi

echo "--- Ratus Release Binary Cold Startup ---"
/usr/bin/time -v ./target/release/ratus validate -c tests/fixtures/sample_config.yaml 2>&1 | grep -E 'Elapsed|Maximum resident'

echo ""
echo "[4/4] Docker Container Footprint Verification..."
if command -v docker >/dev/null 2>&1; then
    echo "Inspecting Docker image sizes..."
    docker images | grep -E 'ratus|gatus' || true
fi

echo "================================================================================"
echo "✅ Head-to-head benchmark run completed successfully."
echo "================================================================================"
