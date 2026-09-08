//! Alert message template interpolation.

use ratus_core::config::EndpointConfig;
use ratus_core::models::EndpointResult;

/// Interpolate placeholder tokens in an alert template string.
pub fn interpolate_alert_template(
    template: &str,
    endpoint: &EndpointConfig,
    result: &EndpointResult,
    custom_desc: Option<&str>,
) -> String {
    let mut output = template.to_string();

    output = output.replace("[ENDPOINT_NAME]", &endpoint.name);
    output = output.replace(
        "[ENDPOINT_GROUP]",
        endpoint.group.as_deref().unwrap_or("default"),
    );
    output = output.replace("[ENDPOINT_URL]", endpoint.url.as_deref().unwrap_or("N/A"));

    let desc = custom_desc.unwrap_or("Health check failed");
    output = output.replace("[ALERT_DESCRIPTION]", desc);

    let errors_str = if result.errors.is_empty() {
        "None".to_string()
    } else {
        result.errors.join(", ")
    };
    output = output.replace("[ERRORS]", &errors_str);

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_template_interpolation() {
        let ep = EndpointConfig {
            name: "billing-api".to_string(),
            group: Some("financial".to_string()),
            url: Some("https://billing.internal".to_string()),
            method: "GET".to_string(),
            body: None,
            headers: None,
            interval: Duration::from_secs(30),
            conditions: vec![],
            alerts: None,
            client: None,
            ui: None,
            dns: None,
            ssh: None,
            enabled: true,
        };
        let res = EndpointResult::failure(500, Duration::from_millis(100), "Connection refused");

        let template = "Alert: [ENDPOINT_NAME] in [ENDPOINT_GROUP] failed: [ERRORS]";
        let rendered = interpolate_alert_template(template, &ep, &res, None);
        assert_eq!(
            rendered,
            "Alert: billing-api in financial failed: Connection refused"
        );
    }
}
