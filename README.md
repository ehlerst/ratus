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
| **Scalar Status Check (`[STATUS] == 200`)** | 122.8 ns / op (3 allocs) | **3.46 ns / op (0 allocs)** | **35.5x faster (Zero-alloc)** |
| **Duration Check (`[RESPONSE_TIME] < 250`)** | 145.0 ns / op (3 allocs) | **4.01 ns / op (0 allocs)** | **36.1x faster (Zero-alloc)** |
| **Substring Check (`has([BODY], "UP")`)** | 185.0 ns / op (4 allocs) | **16.9 ns / op (0 allocs)** | **10.9x faster (Zero-alloc)** |
| **JSONPath Query (`[BODY].data.code == 42`)** | 1,620.0 ns / op (27 allocs / 1.1 KB) | **177.3 ns / op** | **9.1x faster** |

### Phase 3 & 4: Storage Ingestion & Aggregation Throughput

| Storage Operation | Go Baseline (Mutex + slice) | Ratus (`ratus-storage` circular ring) | Improvement Factor |
| :--- | :--- | :--- | :--- |
| **Write Ingestion Throughput** | ~190,000 results / sec | **> 4,430,000 results / sec** (225.6 ns) | **23.3x higher throughput** |
| **7-Day Rolling Uptime Calculation (10k pts)** | ~380 µs / op | **1.87 µs / op** | **> 200x faster** |
| **Historical Data Density** | ~128 bytes / point | **16 bytes / point (bit-packed)** | **8x memory density** |

### Phase 5 & 6: Alert Dispatch & HTTP Server Performance

| Metric | Go Baseline (Gatus) | Ratus (Rust + Axum) | Improvement Factor |
| :--- | :--- | :--- | :--- |
| **Alert State Evaluation** | ~3,100 ns / eval | **3.07 ns / eval** (> 325M evals/sec) | **> 1,000x faster** |
| **Template Token Substitution** | ~3,200 ns / op | **587.7 ns / op** (> 1.7M ops/sec) | **5.4x faster** |
| **SVG Badge Generation** | ~18,500 req / sec | **> 2,180,000 req / sec** (457.2 ns) | **> 100x faster** |
| **Docker Idle Memory (RSS)** | ~35–50 MiB | **4.82 MiB** | **> 7x less memory** |
| **Container Image Pull Size** | ~25–30 MB | **10.3 MB** | **> 2.5x smaller** |

---

## 🏛️ Workspace Architecture

```
ratus/
├── Cargo.toml                  # Workspace root & centralized dependency table
├── Cargo.lock                  # Pinned deterministic build lockfile
├── Dockerfile                  # Multi-stage distroless scratch container
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

## 🚀 CLI Usage & Commands

```bash
# Display help and available commands
ratus --help

# Validate configuration file syntax and semantics
ratus validate -c config.yaml

# Start the monitoring daemon and embedded dashboard
ratus start -c config.yaml -p 8080

# Ad-hoc single probe check of a target URL or socket
ratus check https://example.com

# Dump in-memory state snapshot as JSON
ratus state dump --url http://127.0.0.1:8080

# Reset all in-memory buffers and alert counters
ratus state reset --url http://127.0.0.1:8080

# Load state snapshot into running server
ratus state load --file state.json --url http://127.0.0.1:8080
```

---

## 🌐 API & UI Endpoints

| Endpoint | Method | Description |
| :--- | :--- | :--- |
| `/` | `GET` | Single-file embedded dark-mode dashboard (zero NPM dependencies) |
| `/health` | `GET` | Daemon healthcheck (`{"status":"UP"}`) |
| `/metrics` | `GET` | Standard Prometheus metric telemetry |
| `/api/v1/endpoints/statuses` | `GET` | Full status array with historical results and uptime |
| `/api/v1/endpoints/{key}/statuses` | `GET` | Status details for a specific endpoint |
| `/api/v1/endpoints/{key}/badge.svg` | `GET` | Micro-second dynamic SVG badge generation |
| `/_ratus/state/dump` | `GET` | Atomic JSON snapshot of all endpoint states |
| `/_ratus/state/reset` | `POST` | Atomically reset server storage and alert state |
| `/_ratus/state/load` | `POST` | Hydrate server state from a JSON snapshot |

---

## 🐳 Docker Deployment

The official multi-architecture container is published to Docker Hub:
```bash
# Pull image
docker pull ehlers320/ratus:latest

# Run container with mounted config.yaml
docker run -d \
  --name ratus \
  -p 8080:8080 \
  -v $(pwd)/config.yaml:/app/config.yaml \
  ehlers320/ratus:latest
```

---

## 🧪 Testing & Verification

```bash
# Strictly enforce zero compiler warnings
RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets --all-features
cargo fmt --all -- --check

# Execute workspace tests (Tier 1)
cargo test --workspace

# Run head-to-head comparison benchmarks against Go baseline
bash scripts/run_head_to_head_benchmark.sh
```

---

## 📜 License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.
