# ==============================================================================
# 🦀 Project Ratus: Makefile
# High-Performance Automated Health Dashboard & Monitoring Daemon in Pure Rust
# ==============================================================================

# Toolchain & paths
CARGO       ?= cargo
PREFIX      ?= /usr/local
DESTDIR     ?=
BINDIR      ?= $(PREFIX)/bin

# Runtime & Local Testing defaults
CONFIG      ?= config.yaml
PORT        ?= 8080
RUST_LOG    ?= info
TARGET      ?= http://127.0.0.1:$(PORT)/health

# Binary paths
TARGET_BIN  := target/release/ratus
DEBUG_BIN   := target/debug/ratus

.DEFAULT_GOAL := help

.PHONY: all help build build-release build-all install uninstall cargo-install \
        run start dev run-release validate check test test-all lint clippy \
        fmt fmt-check bench clean docker-build docker-run

# ------------------------------------------------------------------------------
# Help & Overview
# ------------------------------------------------------------------------------
help:
	@echo "================================================================================"
	@echo "🦀 Ratus - Build, Install & Local Development Makefile"
	@echo "================================================================================"
	@echo "Usage: make [TARGET] [VARIABLE=value]"
	@echo ""
	@echo "🚀 Local Testing & Running:"
	@echo "  make run             Build and start Ratus daemon & UI locally (CONFIG=$(CONFIG), PORT=$(PORT))"
	@echo "  make start           Alias for 'make run'"
	@echo "  make dev             Alias for 'make run'"
	@echo "  make run-release     Build and run optimized release binary locally"
	@echo "  make validate        Validate YAML config syntax and endpoint rules"
	@echo "  make check           Ad-hoc single probe check (TARGET=$(TARGET))"
	@echo ""
	@echo "🔨 Build Steps:"
	@echo "  make build           Compile all crates in debug mode"
	@echo "  make build-release   Compile optimized release binary (target/release/ratus)"
	@echo "  make build-all       Compile workspace targets including tests and benchmarks"
	@echo ""
	@echo "📦 Installation Steps:"
	@echo "  make install         Build and install 'ratus' to \$$BINDIR ($(BINDIR))"
	@echo "  make uninstall       Remove installed 'ratus' binary from \$$BINDIR"
	@echo "  make cargo-install   Install 'ratus' directly into ~/.cargo/bin via Cargo"
	@echo ""
	@echo "🧪 Testing & Code Quality:"
	@echo "  make test            Run fast pure in-memory workspace unit tests"
	@echo "  make test-all        Run workspace tests plus Docker testcontainers suite"
	@echo "  make lint            Run clippy with strict zero-warning enforcement"
	@echo "  make clippy          Alias for 'make lint'"
	@echo "  make fmt             Format code across all workspace crates"
	@echo "  make fmt-check       Verify code formatting without modifying files"
	@echo "  make bench           Run criterion micro-benchmarks"
	@echo "  make clean           Clean cargo build artifacts and cache"
	@echo ""
	@echo "🐳 Docker Operations:"
	@echo "  make docker-build    Build minimal distroless Docker image (ratus:latest)"
	@echo "  make docker-run      Run Docker image exposing port $(PORT)"
	@echo "================================================================================"

all: build-release

# ------------------------------------------------------------------------------
# Build Steps
# ------------------------------------------------------------------------------
build:
	@echo "Building Ratus workspace (debug)..."
	$(CARGO) build --workspace

build-release:
	@echo "Building Ratus binary (release)..."
	$(CARGO) build --release --bin ratus

build-all:
	@echo "Building all workspace targets and test suites..."
	$(CARGO) build --workspace --all-targets --all-features

# ------------------------------------------------------------------------------
# Local Testing & Running
# ------------------------------------------------------------------------------
run: build
	@echo "Starting Ratus locally with config='$(CONFIG)' on port $(PORT)..."
	RUST_LOG=$(RUST_LOG) $(DEBUG_BIN) start -c $(CONFIG) -p $(PORT)

start: run
dev: run

run-release: build-release
	@echo "Starting Ratus release binary with config='$(CONFIG)' on port $(PORT)..."
	RUST_LOG=$(RUST_LOG) $(TARGET_BIN) start -c $(CONFIG) -p $(PORT)

validate:
	@echo "Validating configuration file '$(CONFIG)'..."
	$(CARGO) run --bin ratus -- validate -c $(CONFIG)

check:
	@echo "Executing ad-hoc probe check against '$(TARGET)'..."
	$(CARGO) run --bin ratus -- check $(TARGET)

# ------------------------------------------------------------------------------
# Installation Steps
# ------------------------------------------------------------------------------
install: build-release
	@echo "Installing ratus binary to $(DESTDIR)$(BINDIR)/ratus..."
	install -d $(DESTDIR)$(BINDIR)
	install -m 755 $(TARGET_BIN) $(DESTDIR)$(BINDIR)/ratus
	@echo "Installed ratus successfully: $(DESTDIR)$(BINDIR)/ratus"

uninstall:
	@echo "Removing ratus binary from $(DESTDIR)$(BINDIR)/ratus..."
	rm -f $(DESTDIR)$(BINDIR)/ratus
	@echo "Uninstalled ratus."

cargo-install:
	@echo "Installing ratus into Cargo binary path via cargo install..."
	$(CARGO) install --path crates/ratus-server --force

# ------------------------------------------------------------------------------
# Testing & Code Quality
# ------------------------------------------------------------------------------
test:
	@echo "Running fast in-memory workspace tests..."
	$(CARGO) test --workspace

test-all: test
	@echo "Running integration testcontainers test suite..."
	$(CARGO) test --manifest-path Testcontainers/rust-testcontainers/Cargo.toml

lint:
	@echo "Running clippy with zero-warning discipline..."
	RUSTFLAGS="-D warnings" $(CARGO) clippy --workspace --all-targets --all-features

clippy: lint

fmt:
	@echo "Formatting Rust source code..."
	$(CARGO) fmt --all

fmt-check:
	@echo "Checking formatting compliance..."
	$(CARGO) fmt --all -- --check

bench:
	@echo "Running Criterion micro-benchmarks..."
	$(CARGO) bench -p ratus-benchmarks

clean:
	@echo "Cleaning workspace artifacts..."
	$(CARGO) clean
	rm -rf Testcontainers/rust-testcontainers/target

# ------------------------------------------------------------------------------
# Docker Operations
# ------------------------------------------------------------------------------
docker-build:
	@echo "Building Docker container image 'ratus:latest'..."
	docker build -t ratus:latest .

docker-run:
	@echo "Running Docker container on http://localhost:$(PORT)..."
	docker run --rm -it -p $(PORT):8080 ratus:latest
