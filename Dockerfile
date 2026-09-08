# Stage 1: Build binary
FROM rust:1-bookworm AS builder

WORKDIR /usr/src/ratus

# Copy workspace manifests and sources
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/

# Build optimized release binary
RUN cargo build --release --bin ratus

# Stage 2: Minimal distroless runtime
FROM gcr.io/distroless/cc-debian12:latest

# Copy binary from builder
COPY --from=builder /usr/src/ratus/target/release/ratus /usr/local/bin/ratus

# Set working directory and expose default port
WORKDIR /app
EXPOSE 8080

ENTRYPOINT ["/usr/local/bin/ratus"]
CMD ["start", "-c", "/app/config.yaml"]
