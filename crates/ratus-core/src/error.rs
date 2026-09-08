//! Core error models for Ratus.

use thiserror::Error;

/// Core error enum for Ratus operations.
#[derive(Error, Debug)]
pub enum RatusError {
    /// Configuration parse or deserialization failure.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Validation error in parsed configuration.
    #[error("Configuration validation error: {0}")]
    Validation(String),

    /// Condition expression parsing or evaluation failure.
    #[error("Condition evaluation error: {0}")]
    Evaluation(String),

    /// Probing failure across network protocols.
    #[error("Probe error: {0}")]
    Probe(String),

    /// Storage persistence or read failure.
    #[error("Storage error: {0}")]
    Storage(String),

    /// Alert dispatch or formatting failure.
    #[error("Alert error: {0}")]
    Alert(String),

    /// Underlying standard I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// YAML deserialization error.
    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Specialized Result type for Ratus operations.
pub type Result<T> = std::result::Result<T, RatusError>;
