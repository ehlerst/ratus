//! Ratus binary CLI entrypoint.

use clap::{Parser, Subcommand};
use ratus_core::Config;
use std::path::PathBuf;
use std::process::ExitCode;
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
        #[arg(short = 'c', long = "config", default_value = "config.yaml")]
        config: PathBuf,
    },

    /// Start the Ratus monitoring daemon and web dashboard
    Start {
        /// Path to the configuration YAML file
        #[arg(short = 'c', long = "config", default_value = "config.yaml")]
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
}

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Validate { config } => {
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
            info!("Loading configuration from '{}'...", config.display());
            let cfg = match Config::from_file(&config) {
                Ok(c) => c,
                Err(err) => {
                    eprintln!("Failed to load configuration: {err}");
                    return ExitCode::FAILURE;
                }
            };
            let bind_port = port.unwrap_or(8080);
            println!(
                "Starting Ratus daemon on port {bind_port} with {} endpoints...",
                cfg.endpoints.len()
            );
            ExitCode::SUCCESS
        }
        Commands::Check { target } => {
            println!("Probing target: {target}...");
            ExitCode::SUCCESS
        }
    }
}
