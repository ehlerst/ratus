//! Ratus binary CLI and daemon entrypoint.

use clap::{Parser, Subcommand};
use ratus_alert::AlertDispatcher;
use ratus_core::Config;
use ratus_prober::{ChaosEngine, ProbeDispatcher, ProbeEvent, Scheduler};
use ratus_server::create_router_with_options;
use ratus_storage::{MemoryStorage, SqliteStorage};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tracing::{error, info};

#[derive(Parser, Debug)]
#[command(name = "ratus", version, about = "High-performance automated service health dashboard in pure Rust", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Validate configuration file syntax, environment variables, and semantic consistency
    Validate {
        /// Path to the configuration YAML file
        #[arg(
            short = 'c',
            long = "config",
            default_value = "config.yaml",
            env = "RATUS_CONFIG_PATH"
        )]
        config: PathBuf,
    },

    /// Start the Ratus monitoring daemon and web dashboard
    Start {
        /// Path to the configuration YAML file
        #[arg(
            short = 'c',
            long = "config",
            default_value = "config.yaml",
            env = "RATUS_CONFIG_PATH"
        )]
        config: PathBuf,

        /// Port to bind the HTTP dashboard and metrics server
        #[arg(short = 'p', long = "port", env = "RATUS_PORT")]
        port: Option<u16>,
    },

    /// Ad-hoc single probe check of a target URL or host
    Check {
        /// URL or host target to probe
        target: String,
    },

    /// Manage or inspect Ratus server state (atomic export, reset, or load)
    State {
        #[command(subcommand)]
        action: StateAction,
    },

    /// Manage chaos simulation rules (fault injection, latency, and auto-recovery)
    Chaos {
        #[command(subcommand)]
        action: ChaosAction,
    },
}

#[derive(Subcommand, Debug)]
enum ChaosAction {
    /// Inject a chaos fault or latency rule
    Inject {
        /// Target endpoint key (e.g. "core_api", or "*" to match all endpoints)
        #[arg(short = 'e', long = "endpoint")]
        endpoint: String,

        /// Artificial latency in milliseconds
        #[arg(long, default_value_t = 0)]
        latency_ms: u64,

        /// Maximum additional jitter in milliseconds
        #[arg(long, default_value_t = 0)]
        jitter_ms: u64,

        /// Forced HTTP status code (e.g. 500, 502, 503)
        #[arg(long)]
        status: Option<u16>,

        /// Forced error message
        #[arg(long)]
        error: Option<String>,

        /// Number of times to fire before auto-recovering
        #[arg(long)]
        limit: Option<u32>,

        /// Base URL of running Ratus server
        #[arg(long, default_value = "http://127.0.0.1:8080")]
        url: String,
    },
    /// List all active chaos rules
    List {
        /// Base URL of running Ratus server
        #[arg(long, default_value = "http://127.0.0.1:8080")]
        url: String,
    },
    /// Clear all active chaos rules
    Reset {
        /// Base URL of running Ratus server
        #[arg(long, default_value = "http://127.0.0.1:8080")]
        url: String,
    },
}

#[derive(Subcommand, Debug)]
enum StateAction {
    /// Export atomic JSON snapshot of server state
    Dump {
        /// Base URL of Ratus server
        #[arg(long, default_value = "http://127.0.0.1:8080")]
        url: String,
    },
    /// Reset in-memory buffers and alert states
    Reset {
        /// Base URL of Ratus server
        #[arg(long, default_value = "http://127.0.0.1:8080")]
        url: String,
    },
    /// Load JSON snapshot into running server
    Load {
        /// Path to JSON state file
        #[arg(short = 'f', long = "file")]
        file: PathBuf,

        /// Base URL of Ratus server
        #[arg(long, default_value = "http://127.0.0.1:8080")]
        url: String,
    },
}

fn resolve_config_path(config: PathBuf) -> PathBuf {
    if config == std::path::Path::new("config.yaml") {
        if let Ok(p) = std::env::var("GATUS_CONFIG_PATH") {
            return PathBuf::from(p);
        }
    }
    config
}

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Validate { config } => {
            let config = resolve_config_path(config);
            info!("Validating configuration from '{}'...", config.display());
            match Config::from_file(&config) {
                Ok(cfg) => {
                    println!(
                        "Configuration is valid! Loaded {} endpoints.",
                        cfg.endpoints.len()
                    );
                    ExitCode::SUCCESS
                }
                Err(err) => {
                    error!("Validation failed: {err}");
                    eprintln!("Error: {err}");
                    ExitCode::FAILURE
                }
            }
        }
        Commands::Start { config, port } => {
            let config = resolve_config_path(config);
            info!("Loading configuration from '{}'...", config.display());
            let cfg = match Config::from_file(&config) {
                Ok(c) => c,
                Err(err) => {
                    if !config.exists() {
                        info!(
                            "Configuration file '{}' not found, initializing with default configuration.",
                            config.display()
                        );
                        Config::default()
                    } else {
                        eprintln!("Failed to load configuration: {err}");
                        return ExitCode::FAILURE;
                    }
                }
            };

            let storage = Arc::new(MemoryStorage::default());

            // Check if SQLite persistence is configured
            let sqlite_storage: Option<Arc<SqliteStorage>> = match &cfg.storage {
                Some(s_cfg) if s_cfg.storage_type == "sqlite" => {
                    let path = s_cfg.path.as_deref().unwrap_or("ratus.db");
                    info!("Initializing SQLite persistent storage at '{path}' (WAL mode)...");
                    match SqliteStorage::open(path) {
                        Ok(sqlite) => {
                            if let Ok(historical) = sqlite.load_all_recent(50) {
                                for (key, results) in historical {
                                    for r in results {
                                        let (group, name) =
                                            if let Some((g, n)) = key.split_once('_') {
                                                (Some(g), n)
                                            } else {
                                                (None, key.as_str())
                                            };
                                        storage.save_result_by_key(&key, name, group, r);
                                    }
                                }
                                info!("Restored historical results from SQLite persistence.");
                            }
                            Some(Arc::new(sqlite))
                        }
                        Err(e) => {
                            error!("Failed to open SQLite database at '{path}': {e}");
                            None
                        }
                    }
                }
                _ => None,
            };

            let alerting_cfg = cfg.alerting.clone().unwrap_or_default();
            let alert_dispatcher = AlertDispatcher::new(alerting_cfg);

            // Channel for probe result events
            let (event_tx, mut event_rx) = mpsc::channel::<ProbeEvent>(1000);

            // Background event loop routing probe outcomes to storage and alerting
            let loop_storage = storage.clone();
            let loop_alerts = alert_dispatcher.clone();
            let loop_sqlite = sqlite_storage;
            tokio::spawn(async move {
                while let Some(event) = event_rx.recv().await {
                    loop_storage.save_result(&event.endpoint, event.result.clone());
                    if let Some(ref sqlite) = loop_sqlite {
                        let _ = sqlite.save_result(&event.endpoint, &event.result);
                    }
                    loop_alerts
                        .process_result(&event.endpoint, &event.result)
                        .await;
                }
            });

            let chaos = Arc::new(ChaosEngine::new());

            // Start periodic probe scheduler
            let scheduler = Scheduler::new(event_tx).with_chaos(chaos.clone());
            scheduler.start(&cfg);

            // Optional OpenTelemetry (OTLP/HTTP) background push exporter
            let otel_cfg = cfg.otel.clone().unwrap_or_default();
            let otel_endpoint = otel_cfg
                .endpoint
                .or_else(|| std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok());

            if (otel_cfg.enabled || otel_endpoint.is_some()) && otel_endpoint.is_some() {
                if let Some(endpoint_url) = otel_endpoint {
                    let otel_storage = storage.clone();
                    let service_name = otel_cfg.service_name;
                    let push_interval = otel_cfg.interval;
                    info!(
                        "Starting OpenTelemetry OTLP background push exporter to '{endpoint_url}' (interval: {push_interval:?})..."
                    );
                    tokio::spawn(async move {
                        let client = reqwest::Client::new();
                        loop {
                            tokio::time::sleep(push_interval).await;
                            let payload = ratus_server::api::generate_otlp_metrics(
                                &otel_storage,
                                &service_name,
                            );
                            if let Err(e) = client.post(&endpoint_url).json(&payload).send().await {
                                tracing::warn!(
                                    "Failed to push OTLP metrics to '{endpoint_url}': {e}"
                                );
                            }
                        }
                    });
                }
            }

            let bind_port = port.unwrap_or(8080);
            let addr = format!("0.0.0.0:{bind_port}");
            info!("Binding HTTP server on {addr}...");

            let router = create_router_with_options(storage, chaos, cfg.security.clone());
            let listener = match TcpListener::bind(&addr).await {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("Failed to bind port {bind_port}: {e}");
                    return ExitCode::FAILURE;
                }
            };

            println!("🦀 Ratus is operational at http://{addr}");
            if let Err(e) = axum::serve(listener, router).await {
                eprintln!("Server error: {e}");
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
        Commands::Check { target } => {
            println!("Probing target: {target}...");
            let dummy_ep = ratus_core::config::EndpointConfig {
                name: "adhoc-check".to_string(),
                group: None,
                url: Some(target.clone()),
                method: "GET".to_string(),
                body: None,
                headers: None,
                interval: std::time::Duration::from_secs(30),
                conditions: vec!["[STATUS] == 200".to_string()],
                alerts: None,
                client: None,
                ui: None,
                dns: None,
                ssh: None,
                enabled: true,
            };

            let dispatcher = ProbeDispatcher::new();
            let result = dispatcher.probe(&dummy_ep).await;

            println!(
                "Probe result: success={}, status_code={}, duration={:?}",
                result.success, result.status_code, result.duration
            );
            if result.success {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Commands::State { action } => {
            let client = reqwest::Client::new();
            match action {
                StateAction::Dump { url } => {
                    let endpoint = format!("{}/_ratus/state/dump", url.trim_end_matches('/'));
                    match client.get(&endpoint).send().await {
                        Ok(resp) => {
                            if resp.status().is_success() {
                                let body = resp.text().await.unwrap_or_default();
                                println!("{body}");
                                ExitCode::SUCCESS
                            } else {
                                eprintln!("Failed to dump state: HTTP {}", resp.status());
                                ExitCode::FAILURE
                            }
                        }
                        Err(err) => {
                            eprintln!("Error connecting to Ratus server: {err}");
                            ExitCode::FAILURE
                        }
                    }
                }
                StateAction::Reset { url } => {
                    let endpoint = format!("{}/_ratus/state/reset", url.trim_end_matches('/'));
                    match client.post(&endpoint).send().await {
                        Ok(resp) => {
                            if resp.status().is_success() {
                                println!("Ratus state successfully reset.");
                                ExitCode::SUCCESS
                            } else {
                                eprintln!("Failed to reset state: HTTP {}", resp.status());
                                ExitCode::FAILURE
                            }
                        }
                        Err(err) => {
                            eprintln!("Error connecting to Ratus server: {err}");
                            ExitCode::FAILURE
                        }
                    }
                }
                StateAction::Load { file, url } => {
                    let content = match std::fs::read_to_string(&file) {
                        Ok(c) => c,
                        Err(err) => {
                            eprintln!("Failed to read state file '{}': {err}", file.display());
                            return ExitCode::FAILURE;
                        }
                    };
                    let parsed: serde_json::Value = match serde_json::from_str(&content) {
                        Ok(p) => p,
                        Err(err) => {
                            eprintln!("State file is not valid JSON: {err}");
                            return ExitCode::FAILURE;
                        }
                    };
                    let endpoint = format!("{}/_ratus/state/load", url.trim_end_matches('/'));
                    match client.post(&endpoint).json(&parsed).send().await {
                        Ok(resp) => {
                            if resp.status().is_success() {
                                println!(
                                    "Ratus state successfully loaded from '{}'.",
                                    file.display()
                                );
                                ExitCode::SUCCESS
                            } else {
                                eprintln!("Failed to load state: HTTP {}", resp.status());
                                ExitCode::FAILURE
                            }
                        }
                        Err(err) => {
                            eprintln!("Error connecting to Ratus server: {err}");
                            ExitCode::FAILURE
                        }
                    }
                }
            }
        }
        Commands::Chaos { action } => {
            let client = reqwest::Client::new();
            match action {
                ChaosAction::Inject {
                    endpoint,
                    latency_ms,
                    jitter_ms,
                    status,
                    error,
                    limit,
                    url,
                } => {
                    let rule = ratus_prober::ChaosRule {
                        endpoint_key: endpoint.clone(),
                        latency_ms,
                        jitter_ms,
                        force_status: status,
                        force_error: error,
                        limit_times: limit,
                        times_fired: 0,
                    };
                    let target_url = format!("{}/_ratus/chaos/inject", url.trim_end_matches('/'));
                    match client.post(&target_url).json(&rule).send().await {
                        Ok(resp) => {
                            if resp.status().is_success() {
                                println!(
                                    "Successfully injected chaos rule for endpoint '{endpoint}'."
                                );
                                ExitCode::SUCCESS
                            } else {
                                eprintln!("Failed to inject chaos rule: HTTP {}", resp.status());
                                ExitCode::FAILURE
                            }
                        }
                        Err(err) => {
                            eprintln!("Error connecting to Ratus server: {err}");
                            ExitCode::FAILURE
                        }
                    }
                }
                ChaosAction::List { url } => {
                    let target_url = format!("{}/_ratus/chaos/rules", url.trim_end_matches('/'));
                    match client.get(&target_url).send().await {
                        Ok(resp) => {
                            if resp.status().is_success() {
                                let body = resp.text().await.unwrap_or_default();
                                println!("{body}");
                                ExitCode::SUCCESS
                            } else {
                                eprintln!("Failed to retrieve chaos rules: HTTP {}", resp.status());
                                ExitCode::FAILURE
                            }
                        }
                        Err(err) => {
                            eprintln!("Error connecting to Ratus server: {err}");
                            ExitCode::FAILURE
                        }
                    }
                }
                ChaosAction::Reset { url } => {
                    let target_url = format!("{}/_ratus/chaos/reset", url.trim_end_matches('/'));
                    match client.post(&target_url).send().await {
                        Ok(resp) => {
                            if resp.status().is_success() {
                                println!("All chaos rules have been reset.");
                                ExitCode::SUCCESS
                            } else {
                                eprintln!("Failed to reset chaos rules: HTTP {}", resp.status());
                                ExitCode::FAILURE
                            }
                        }
                        Err(err) => {
                            eprintln!("Error connecting to Ratus server: {err}");
                            ExitCode::FAILURE
                        }
                    }
                }
            }
        }
    }
}
