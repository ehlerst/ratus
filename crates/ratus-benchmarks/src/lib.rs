//! Benchmark fixtures and utilities for Ratus.

#![deny(missing_docs)]
#![deny(clippy::all)]

/// Helper module to generate synthetic YAML configurations for benchmarking.
pub mod fixtures {
    /// Generate a valid YAML string containing `endpoint_count` endpoints.
    pub fn generate_yaml_config(endpoint_count: usize) -> String {
        let mut yaml = String::with_capacity(endpoint_count * 250);
        yaml.push_str("metrics: true\n");
        yaml.push_str("ui:\n  title: \"Benchmark Dashboard\"\n");
        yaml.push_str("endpoints:\n");

        for i in 0..endpoint_count {
            yaml.push_str(&format!(
                "  - name: service-{i}\n    group: cluster-{}\n    url: \"https://api.internal/v1/service-{i}/health\"\n    interval: 30s\n    conditions:\n      - \"[STATUS] == 200\"\n      - \"[RESPONSE_TIME] < 250\"\n",
                i % 10
            ));
        }

        yaml
    }
}
