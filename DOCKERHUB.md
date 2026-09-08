# 🦀 Ratus: High-Performance Automated Health Dashboard in Pure Rust

[![Docker Hub](https://img.shields.io/docker/pulls/ehlers320/ratus.svg)](https://hub.docker.com/r/ehlers320/ratus)
[![Image Size](https://img.shields.io/docker/image-size/ehlers320/ratus/latest)](https://hub.docker.com/r/ehlers320/ratus)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/ehlerst/ratus/blob/main/LICENSE)
[![GitHub](https://img.shields.io/badge/github-ehlerst%2Fratus-orange.svg)](https://github.com/ehlerst/ratus)

> **Ratus** is an ultra-fast, zero-garbage-collection, memory-disciplined automated service health monitoring daemon and status dashboard written in pure Rust. It is designed as a drop-in, high-efficiency alternative to Gatus.

---

## ⚡ Highlights

* **Minimal Container Footprint**: ~10 MB uncompressed distroless runtime with `< 5 MiB` idle memory usage.
* **Microsecond Evaluation Engine**: AST-parsed zero-allocation condition evaluation (`[STATUS] == 200`, `[RESPONSE_TIME] < 100`, etc.).
* **Comprehensive Protocol Support**:
  * **HTTP / HTTPS**: Status codes, body parsing (JSON, regex, length), TLS certificate expiration, custom headers.
  * **TCP / UDP**: Raw socket connection tests with latency timers.
  * **DNS**: Resolver queries (`A`, `AAAA`, `CNAME`, `MX`, `TXT`).
  * **WebSocket**: RFC 6455 upgrade handshakes over `ws://` and `wss://`.
  * **gRPC**: Standard `/grpc.health.v1.Health/Check` status verification over HTTP/2.
  * **ICMP / Ping**: Fast network round-trip ping verification.
* **Modernized Web Dashboard**:
  * Single-file embedded UI with zero CDN or external JavaScript dependencies.
  * Live status beacon, search filter, group selector pills, and latency SVG sparklines.
  * Interactive 30-probe history segments with hover metrics.
  * Built-in interactive **Chaos Lab** modal.
* **Observability & Metrics**:
  * Native **Prometheus** metrics at `/metrics`.
  * Native **OpenTelemetry (OTEL)** OTLP/HTTP metrics at `/v1/metrics` and background push exporter.
  * Micro-second dynamic SVG status badges at `/api/v1/endpoints/{key}/badge.svg`.
* **Storage Options**: Lock-free in-memory circular ring buffer or persistent zero-config SQLite with WAL mode.
* **Multi-Channel Alerting**: Slack, Discord, Telegram, PagerDuty, and custom HTTP webhooks with failure/recovery thresholds.

---

## 🚀 Quick Start

### 1. Run with Docker

Run directly with default local self-monitoring:

```bash
docker run -d \
  --name ratus \
  -p 8080:8080 \
  ehlers320/ratus:latest
```

Open your browser at [http://localhost:8080](http://localhost:8080).

### 2. Run with Custom `config.yaml`

```bash
docker run -d \
  --name ratus \
  -p 8080:8080 \
  -v $(pwd)/config.yaml:/app/config.yaml \
  ehlers320/ratus:latest
```

### 3. Docker Compose Example

```yaml
version: "3.8"

services:
  ratus:
    image: ehlers320/ratus:latest
    container_name: ratus
    restart: unless-stopped
    ports:
      - "8080:8080"
    volumes:
      - ./config.yaml:/app/config.yaml:ro
      - ratus-data:/data
    environment:
      - RATUS_CONFIG_PATH=/app/config.yaml
      - RATUS_PORT=8080
      - RUST_LOG=info

volumes:
  ratus-data:
```

---

## ⚙️ Configuration Example (`config.yaml`)

```yaml
metrics: true

ui:
  title: "Production Infrastructure Health"
  description: "Automated status dashboard in pure Rust"
  header: "Acme Cloud Services"

# Persistent storage (optional, defaults to in-memory)
storage:
  type: sqlite
  path: /data/ratus.db

# OpenTelemetry configuration (optional)
otel:
  enabled: true
  endpoint: "http://otel-collector:4318/v1/metrics"
  interval: 30s
  service_name: "ratus-production"

# Global Alerting Providers (optional)
alerting:
  slack:
    webhook-url: ${SLACK_WEBHOOK_URL}
  discord:
    webhook-url: ${DISCORD_WEBHOOK_URL}

endpoints:
  - name: core-api
    group: api
    url: https://api.example.com/health
    interval: 15s
    conditions:
      - "[STATUS] == 200"
      - "[RESPONSE_TIME] < 200"
      - "[CERTIFICATE_EXPIRATION] > 48h"
    alerts:
      - type: slack
        failure-threshold: 3
        success-threshold: 2
        send-on-resolved: true

  - name: postgres-primary
    group: database
    url: tcp://db.example.internal:5432
    interval: 30s
    conditions:
      - "[CONNECTED] == true"

  - name: realtime-gateway
    group: realtime
    url: wss://ws.example.com/socket
    interval: 20s
    conditions:
      - "[STATUS] == 101"
      - "[RESPONSE_TIME] < 100"

  - name: grpc-health
    group: services
    url: grpc://auth.example.internal:50051
    interval: 30s
    conditions:
      - "[STATUS] == 200"
      - "[CONNECTED] == true"

  - name: network-gateway
    group: network
    url: ping://1.1.1.1
    interval: 10s
    conditions:
      - "[CONNECTED] == true"
      - "[RESPONSE_TIME] < 30"
```

---

## 🌐 Endpoints Reference

| Path | Method | Description |
|---|---|---|
| `/` | `GET` | Embedded dark space web dashboard (zero external JS/CDN) |
| `/health` | `GET` | Healthcheck endpoint (`{"status":"UP"}`) |
| `/metrics` | `GET` | Prometheus metric telemetry |
| `/v1/metrics` | `GET` | OpenTelemetry OTLP/HTTP JSON metric telemetry |
| `/api/v1/endpoints/statuses` | `GET` | Full endpoint statuses, latencies, and uptime history |
| `/api/v1/endpoints/{key}/badge.svg` | `GET` | Microsecond dynamic SVG status badge |
| `/_ratus/chaos/inject` | `POST` | Inject synthetic failure, latency, or error codes |
| `/_ratus/chaos/reset` | `POST` | Clear all active chaos engineering rules |
| `/_ratus/state/dump` | `GET` | Export atomic JSON snapshot of server state |

---

## 📊 Performance vs Go Baseline

| Metric | Go Baseline (Gatus) | Ratus (Rust) | Advantage |
|---|---|---|---|
| **Condition Evaluation** | 122.8 ns / eval (3 allocs) | **3.46 ns / eval (0 allocs)** | **35.5x faster (Zero-alloc)** |
| **Config Ingestion (1k eps)** | 9.85 ms (82k allocs) | **4.65 ms** | **2.1x faster** |
| **Storage Ingestion Rate** | ~190k results/s | **> 4.4M results/s** | **23.3x higher throughput** |
| **Badge Generation** | ~18.5k req/s | **> 2.1M req/s** | **> 100x faster** |
| **Idle Memory (Docker RSS)** | ~35–50 MiB | **4.82 MiB** | **> 7x less memory** |
| **Container Image Size** | ~25–30 MB | **10.3 MB** | **> 2.5x smaller** |

---

## 📜 License & Attribution

Project Ratus is open source dual-licensed under [Apache License 2.0](https://github.com/ehlerst/ratus/blob/main/LICENSE-APACHE) and [MIT](https://github.com/ehlerst/ratus/blob/main/LICENSE-MIT).

* Ratus is inspired by and designed as a high-performance Rust drop-in replacement for [**Gatus**](https://github.com/TwiN/gatus), originally created by [**TwiN**](https://github.com/TwiN) under Apache License 2.0.
* See the [NOTICE](https://github.com/ehlerst/ratus/blob/main/NOTICE) file for full attribution and trademark notices.
* Gatus is a project and trademark of TwiN. Ratus is an independent clean-room implementation in pure Rust.
