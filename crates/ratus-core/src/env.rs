//! Environment variable interpolation for configuration strings.

use regex::Regex;
use std::env;
use std::sync::LazyLock;

static ENV_VAR_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    // Matches ${VAR} or ${VAR:default_value} or $VAR
    Regex::new(r"\$\{([a-zA-Z_][a-zA-Z0-9_]*(?::[^}]*)?)\}").expect("Invalid regex")
});

/// Interpolate environment variables in a template string.
///
/// Supports:
/// - `${VAR}`: Replaced by the environment variable `VAR` (or empty string if unset)
/// - `${VAR:default}`: Replaced by `VAR` if set, otherwise `default`
pub fn interpolate_env_vars(input: &str) -> String {
    ENV_VAR_REGEX
        .replace_all(input, |caps: &regex::Captures| {
            let inner = &caps[1];
            if let Some((var_name, default_val)) = inner.split_once(':') {
                env::var(var_name).unwrap_or_else(|_| default_val.to_string())
            } else {
                env::var(inner).unwrap_or_default()
            }
        })
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpolation_with_default() {
        let template = "url: http://${TEST_HOST_VAR:localhost}:8080/health";
        let result = interpolate_env_vars(template);
        assert_eq!(result, "url: http://localhost:8080/health");
    }

    #[test]
    fn test_interpolation_with_env_set() {
        std::env::set_var("RATUS_TEST_PORT", "9090");
        let template = "port: ${RATUS_TEST_PORT:8080}";
        let result = interpolate_env_vars(template);
        assert_eq!(result, "port: 9090");
        std::env::remove_var("RATUS_TEST_PORT");
    }

    #[test]
    fn test_interpolation_empty_default() {
        let template = "token: ${UNSET_TOKEN_XYZ}";
        let result = interpolate_env_vars(template);
        assert_eq!(result, "token: ");
    }
}
