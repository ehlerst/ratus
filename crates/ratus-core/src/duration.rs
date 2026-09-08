//! Human-readable duration parser and serde adapter.

use serde::{de, Deserialize, Deserializer, Serializer};
use std::fmt;
use std::time::Duration;

/// Parse a human-readable duration string such as "30s", "5m", "1h", "24h", "500ms".
pub fn parse_duration(input: &str) -> Result<Duration, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("Duration string cannot be empty".to_string());
    }

    // Find the split point between number and unit
    let split_idx = trimmed
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .ok_or_else(|| "Missing time unit in duration (e.g. '30s', '5m')".to_string())?;

    let (num_str, unit_str) = trimmed.split_at(split_idx);
    let value: f64 = num_str
        .trim()
        .parse()
        .map_err(|_| format!("Invalid numerical value in duration: '{num_str}'"))?;

    if value < 0.0 {
        return Err("Duration cannot be negative".to_string());
    }

    let unit = unit_str.trim().to_ascii_lowercase();
    let millis = match unit.as_str() {
        "ms" | "millis" | "millisecond" | "milliseconds" => value,
        "s" | "sec" | "second" | "seconds" => value * 1000.0,
        "m" | "min" | "minute" | "minutes" => value * 60.0 * 1000.0,
        "h" | "hr" | "hour" | "hours" => value * 3600.0 * 1000.0,
        "d" | "day" | "days" => value * 86400.0 * 1000.0,
        _ => return Err(format!("Unknown time unit in duration: '{unit_str}'")),
    };

    Ok(Duration::from_millis(millis.round() as u64))
}

/// Format a duration as a compact human-readable string (e.g. "30s", "5m").
#[allow(clippy::manual_is_multiple_of)]
pub fn format_duration(dur: Duration) -> String {
    let total_secs = dur.as_secs();
    let millis = dur.subsec_millis();

    if total_secs == 0 && millis > 0 {
        format!("{millis}ms")
    } else if total_secs > 0 && total_secs % 86400 == 0 {
        format!("{}d", total_secs / 86400)
    } else if total_secs > 0 && total_secs % 3600 == 0 {
        format!("{}h", total_secs / 3600)
    } else if total_secs > 0 && total_secs % 60 == 0 {
        format!("{}m", total_secs / 60)
    } else {
        format!("{total_secs}s")
    }
}

/// Serde deserializer for human-readable duration strings or integer seconds.
pub fn deserialize_duration<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    struct DurationVisitor;

    impl<'de> de::Visitor<'de> for DurationVisitor {
        type Value = Duration;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a duration string like '30s', '5m' or integer seconds")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            parse_duration(value).map_err(de::Error::custom)
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Duration::from_secs(value))
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            if value < 0 {
                return Err(de::Error::custom("duration cannot be negative"));
            }
            Ok(Duration::from_secs(value as u64))
        }
    }

    deserializer.deserialize_any(DurationVisitor)
}

/// Serde serializer for human-readable duration strings.
pub fn serialize_duration<S>(dur: &Duration, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&format_duration(*dur))
}

/// Re-exports for serde(with = "crate::duration")
pub use deserialize_duration as deserialize;
pub use serialize_duration as serialize;

/// Optional duration deserializer.
pub fn deserialize_optional_duration<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Helper {
        String(String),
        Num(u64),
    }

    let opt = Option::<Helper>::deserialize(deserializer)?;
    match opt {
        Some(Helper::String(s)) => parse_duration(&s).map(Some).map_err(de::Error::custom),
        Some(Helper::Num(n)) => Ok(Some(Duration::from_secs(n))),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("30s").unwrap(), Duration::from_secs(30));
        assert_eq!(parse_duration("5m").unwrap(), Duration::from_secs(300));
        assert_eq!(parse_duration("1h").unwrap(), Duration::from_secs(3600));
        assert_eq!(parse_duration("2d").unwrap(), Duration::from_secs(172800));
        assert_eq!(parse_duration("500ms").unwrap(), Duration::from_millis(500));
        assert_eq!(parse_duration("1.5s").unwrap(), Duration::from_millis(1500));
        assert!(parse_duration("invalid").is_err());
        assert!(parse_duration("-5s").is_err());
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(Duration::from_secs(30)), "30s");
        assert_eq!(format_duration(Duration::from_secs(300)), "5m");
        assert_eq!(format_duration(Duration::from_secs(3600)), "1h");
        assert_eq!(format_duration(Duration::from_secs(172800)), "2d");
        assert_eq!(format_duration(Duration::from_millis(500)), "500ms");
    }
}
