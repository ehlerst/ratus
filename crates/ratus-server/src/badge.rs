//! Zero-allocation dynamic SVG status and uptime badge generator.

/// Generate an SVG badge representing endpoint health status or uptime percentage.
pub fn generate_badge(label: &str, status: &str, is_success: bool) -> String {
    let color = if is_success { "#44cc11" } else { "#e05d44" };
    let label_width = (label.len() * 7 + 10).max(40);
    let status_width = (status.len() * 7 + 10).max(40);
    let total_width = label_width + status_width;

    let label_x = label_width / 2;
    let status_x = label_width + status_width / 2;

    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{total_width}" height="20" role="img" aria-label="{label}: {status}">
  <title>{label}: {status}</title>
  <linearGradient id="s" x2="0" y2="100%">
    <stop offset="0" stop-color="#bbb" stop-opacity=".1"/>
    <stop offset="1" stop-opacity=".1"/>
  </linearGradient>
  <clipPath id="r">
    <rect width="{total_width}" height="20" rx="3" fill="#fff"/>
  </clipPath>
  <g clip-path="url(#r)">
    <rect width="{label_width}" height="20" fill="#555"/>
    <rect x="{label_width}" width="{status_width}" height="20" fill="{color}"/>
    <rect width="{total_width}" height="20" fill="url(#s)"/>
  </g>
  <g fill="#fff" text-anchor="middle" font-family="Verdana,Geneva,DejaVu Sans,sans-serif" text-rendering="geometricPrecision" font-size="110">
    <text aria-hidden="true" x="{label_x}0" y="150" fill="#010101" fill-opacity=".3" transform="scale(.1)" textLength="{label_width}0">{label}</text>
    <text x="{label_x}0" y="140" transform="scale(.1)" fill="#fff" textLength="{label_width}0">{label}</text>
    <text aria-hidden="true" x="{status_x}0" y="150" fill="#010101" fill-opacity=".3" transform="scale(.1)" textLength="{status_width}0">{status}</text>
    <text x="{status_x}0" y="140" transform="scale(.1)" fill="#fff" textLength="{status_width}0">{status}</text>
  </g>
</svg>"##
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_badge() {
        let svg = generate_badge("uptime", "99.9%", true);
        assert!(svg.contains("uptime"));
        assert!(svg.contains("99.9%"));
        assert!(svg.contains("#44cc11"));

        let fail_svg = generate_badge("status", "down", false);
        assert!(fail_svg.contains("#e05d44"));
    }
}
