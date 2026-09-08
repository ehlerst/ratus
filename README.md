# 🦀 Ratus: High-Performance Automated Service Health Dashboard in Pure Rust

[![CI](https://github.com/ehlerst/ratus/actions/workflows/ci.yml/badge.svg)](https://github.com/ehlerst/ratus/actions/workflows/ci.yml)
[![Docker Hub](https://img.shields.io/docker/pulls/ehlers320/ratus.svg)](https://hub.docker.com/r/ehlers320/ratus)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.89%2B-orange.svg)](https://www.rust-lang.org)

> **Ratus** is an ultra-fast, zero-GC, memory-disciplined Rust drop-in replacement for [TwiN/gatus](https://github.com/TwiN/gatus). Engineered strictly according to the **Ehlerst Rust Engineering Standard**, targeting `< 3 MiB` idle memory, sub-millisecond cold boot, and microsecond evaluation latencies.

---

## ⚡ Performance Highlights (Empirical Benchmarks)

### Phase 1: Configuration Ingestion & Deserialization (AMD Ryzen 7 5800H)

| Benchmark Scenario | Go Baseline (`gopkg.in/yaml.v3`) | Ratus (`serde_yaml` in Rust) | Improvement Factor |
| :--- | :--- | :--- | :--- |
| **10 Endpoints Config Ingestion** | 97.6 µs / op (881 allocs) | **51.6 µs / op** | **1.89x faster** |
| **100 Endpoints Config Ingestion** | 922.2 µs / op (8,264 allocs) | **482.9 µs / op** | **1.91x faster** |
| **1,000 Endpoints Config Ingestion** | 9.85 ms / op (82,072 allocs / 3.84 MB) | **4.65 ms / op** | **> 2.1x faster** |
| **Cold CLI Validation Startup** | ~28 ms | **< 1.0 ms** | **> 28x faster** |
| **Stripped Release Binary Size** | ~25 MB (Go static) | **3.0 MB** | **8.3x smaller** |

### Phase 2: Condition Evaluation Micro-Engine (AMD Ryzen 7 5800H)

| Condition Type | Go Baseline (Gatus reflection/replace) | Ratus (`ratus-eval` typed AST) | Improvement Factor |
| :--- | :--- | :--- | :--- |
| **Scalar Status Check (`[STATUS] == 200`)** | 130.1 ns / op (3 allocs) | **3.46 ns / op (0 allocs)** | **37.6x faster (Zero-alloc)** |
| **Duration Check (`[RESPONSE_TIME] < 250`)** | 145.0 ns / op (3 allocs) | **4.01 ns / op (0 allocs)** | **36.1x faster (Zero-alloc)** |
| **Substring Check (`has([BODY], "UP")`)** | 185.0 ns / op (4 allocs) | **16.9 ns / op (0 allocs)** | **10.9x faster (Zero-alloc)** |
| **JSONPath Query (`[BODY].data.code == 42`)** | 1,702.0 ns / op (27 allocs / 1.1 KB) | **177.3 ns / op** | **9.6x faster** |

---

## 🏛️ Workspace Architecture

```
ratus/
├── Cargo.toml                  # Workspace root & centralized dependency table
├── Cargo.lock                  # Pinned deterministic build lockfile
├── PLAN.md                     # Comprehensive multi-phase roadmap & benchmark specifications
├── README.md                   # Documentation & benchmark matrix
├── crates/
│   ├── ratus-core/             # Core models, duration parsing, env interpolation, config schema
│   ├── ratus-eval/             # Zero-allocation condition lexer, AST parser & evaluator
│   ├── ratus-prober/           # Tokio async probing engine (HTTP, TCP, UDP, ICMP, DNS, TLS)
│   ├── ratus-storage/          # Lock-free in-memory ring buffers & persistent backends
│   ├── ratus-alert/            # Alert state machine, deduplication, debounce & webhook dispatch
│   ├── ratus-server/           # Axum HTTP API, SVG badge engine, embedded UI, CLI companion
│   ├── ratus-benchmarks/       # Criterion benchmark suites
│   └── ratus-compat-tests/     # Pure Rust in-memory integration & Gatus parity tests
└── Testcontainers/
    ├── rust-testcontainers/    # Rust Testcontainers end-to-end container test suite
    └── go-testcontainers/      # Cross-language verification & Go benchmark harness
```

---

## 🚀 Quickstart

### 1. Build and Test
```bash
# Verify compiler discipline (zero warnings allowed)
RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets --all-features
cargo fmt --all -- --check

# Run test suite
cargo test --workspace

# Build optimized binary
cargo build --release --bin ratus
```

### 2. Validate Configuration
```bash
./target/release/ratus validate -c tests/fixtures/sample_config.yaml
```

### 3. Run Ingestion Benchmarks
```bash
cargo bench -p ratus-benchmarks --bench config_ingestion
```

---

## 🐳 Docker Deployment

The official multi-architecture container is published to Docker Hub:
```bash
docker pull ehlers320/ratus:latest
```

---

## 📜 License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.
