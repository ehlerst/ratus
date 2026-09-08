//! High-speed condition evaluation engine.

use crate::ast::{Comparator, Condition, Literal, Placeholder};
use crate::context::EvaluationContext;
use serde_json::Value;
use std::time::Duration;

impl Condition {
    /// Evaluate this condition against the provided evaluation context.
    pub fn evaluate(&self, ctx: &EvaluationContext) -> bool {
        match &self.placeholder {
            Placeholder::Status => {
                let actual = ctx.status_code as i64;
                compare_numbers(actual, &self.comparator, &self.target)
            }
            Placeholder::ResponseTime => {
                let actual_ms = ctx.response_time.as_millis() as i64;
                match &self.target {
                    Literal::Duration(dur) => {
                        compare_duration(ctx.response_time, self.comparator, *dur)
                    }
                    _ => compare_numbers(actual_ms, &self.comparator, &self.target),
                }
            }
            Placeholder::Connected => {
                let actual = ctx.connected;
                compare_bool(actual, self.comparator, &self.target)
            }
            Placeholder::CertificateExpiration => match ctx.certificate_expiration {
                Some(actual) => match self.target {
                    Literal::Duration(target_dur) => {
                        compare_duration(actual, self.comparator, target_dur)
                    }
                    Literal::Int(target_secs) => compare_duration(
                        actual,
                        self.comparator,
                        Duration::from_secs(target_secs as u64),
                    ),
                    _ => false,
                },
                None => false,
            },
            Placeholder::Body(None) => match ctx.body_str() {
                Some(body_str) => compare_string(body_str, self.comparator, &self.target),
                None => false,
            },
            Placeholder::Body(Some(path)) => {
                let json = match ctx.json() {
                    Some(j) => j,
                    None => return false,
                };
                match resolve_json_path(&json, path) {
                    Some(val) => compare_json_value(val, self.comparator, &self.target),
                    None => false,
                }
            }
            Placeholder::Headers(header_name) => {
                let headers = match ctx.headers {
                    Some(h) => h,
                    None => return false,
                };
                match headers.get(header_name) {
                    Some(val) => compare_string(val, self.comparator, &self.target),
                    None => false,
                }
            }
            Placeholder::Length(inner) => {
                let len = match inner.as_ref() {
                    Placeholder::Body(None) => ctx.body.map(|b| b.len() as i64).unwrap_or(0),
                    Placeholder::Body(Some(path)) => {
                        let json = match ctx.json() {
                            Some(j) => j,
                            None => return false,
                        };
                        match resolve_json_path(&json, path) {
                            Some(Value::Array(arr)) => arr.len() as i64,
                            Some(Value::String(s)) => s.len() as i64,
                            Some(Value::Object(obj)) => obj.len() as i64,
                            _ => return false,
                        }
                    }
                    _ => return false,
                };
                compare_numbers(len, &self.comparator, &self.target)
            }
            Placeholder::Has(inner, pattern) => {
                let has = match inner.as_ref() {
                    Placeholder::Body(None) => {
                        ctx.body_str().map(|s| s.contains(pattern)).unwrap_or(false)
                    }
                    Placeholder::Body(Some(path)) => {
                        let json = match ctx.json() {
                            Some(j) => j,
                            None => return false,
                        };
                        match resolve_json_path(&json, path) {
                            Some(Value::Array(arr)) => arr.iter().any(|item| match item {
                                Value::String(s) => s == pattern,
                                Value::Number(n) => match pattern.parse::<i64>() {
                                    Ok(expected) => n.as_i64() == Some(expected),
                                    Err(_) => match pattern.parse::<f64>() {
                                        Ok(expected_f) => n
                                            .as_f64()
                                            .is_some_and(|f| (f - expected_f).abs() < f64::EPSILON),
                                        Err(_) => false,
                                    },
                                },
                                Value::Bool(b) => {
                                    pattern.parse::<bool>().is_ok_and(|expected| *b == expected)
                                }
                                _ => false,
                            }),
                            Some(Value::String(s)) => s.contains(pattern),
                            _ => false,
                        }
                    }
                    _ => false,
                };
                compare_bool(has, self.comparator, &self.target)
            }
            Placeholder::Ip => match ctx.ip {
                Some(ip) => compare_string(ip, self.comparator, &self.target),
                None => false,
            },
            Placeholder::DnsRcode => match ctx.dns_rcode {
                Some(code) => compare_string(code, self.comparator, &self.target),
                None => false,
            },
        }
    }
}

/// Helper to walk a JSON value by dot/bracket path (e.g. `data.users[0].name`).
pub fn resolve_json_path<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = root;

    for segment in path.split('.') {
        if segment.contains('[') && segment.ends_with(']') {
            let (key, idx_str) = segment.split_once('[')?;
            let idx_str = idx_str.strip_suffix(']')?;
            let idx: usize = idx_str.parse().ok()?;

            if !key.is_empty() {
                current = current.get(key)?;
            }
            current = current.get(idx)?;
        } else {
            current = current.get(segment)?;
        }
    }

    Some(current)
}

fn compare_numbers(actual: i64, comp: &Comparator, target: &Literal) -> bool {
    let target_val = match target {
        Literal::Int(i) => *i as f64,
        Literal::Float(f) => *f,
        Literal::String(s) => match s.parse::<f64>() {
            Ok(v) => v,
            Err(_) => return false,
        },
        _ => return false,
    };
    let act_f = actual as f64;

    match comp {
        Comparator::Eq => (act_f - target_val).abs() < f64::EPSILON,
        Comparator::NotEq => (act_f - target_val).abs() >= f64::EPSILON,
        Comparator::Lt => act_f < target_val,
        Comparator::Lte => act_f <= target_val,
        Comparator::Gt => act_f > target_val,
        Comparator::Gte => act_f >= target_val,
    }
}

fn compare_duration(actual: Duration, comp: Comparator, target: Duration) -> bool {
    match comp {
        Comparator::Eq => actual == target,
        Comparator::NotEq => actual != target,
        Comparator::Lt => actual < target,
        Comparator::Lte => actual <= target,
        Comparator::Gt => actual > target,
        Comparator::Gte => actual >= target,
    }
}

fn compare_bool(actual: bool, comp: Comparator, target: &Literal) -> bool {
    let target_bool = match target {
        Literal::Bool(b) => *b,
        Literal::String(s) => match s.to_ascii_lowercase().as_str() {
            "true" | "yes" | "1" => true,
            "false" | "no" | "0" => false,
            _ => return false,
        },
        _ => return false,
    };

    match comp {
        Comparator::Eq => actual == target_bool,
        Comparator::NotEq => actual != target_bool,
        _ => false,
    }
}

fn compare_string(actual: &str, comp: Comparator, target: &Literal) -> bool {
    let target_str = match target {
        Literal::String(s) => s.as_str(),
        Literal::Int(_) => return compare_numbers(actual.parse().unwrap_or(0), &comp, target),
        Literal::Bool(_) => return compare_bool(actual.parse().unwrap_or(false), comp, target),
        _ => return false,
    };

    match comp {
        Comparator::Eq => actual == target_str,
        Comparator::NotEq => actual != target_str,
        Comparator::Lt => actual < target_str,
        Comparator::Lte => actual <= target_str,
        Comparator::Gt => actual > target_str,
        Comparator::Gte => actual >= target_str,
    }
}

fn compare_json_value(actual: &Value, comp: Comparator, target: &Literal) -> bool {
    match actual {
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                compare_numbers(i, &comp, target)
            } else if let Some(f) = n.as_f64() {
                let target_f = match target {
                    Literal::Float(tf) => *tf,
                    Literal::Int(ti) => *ti as f64,
                    _ => return false,
                };
                match comp {
                    Comparator::Eq => (f - target_f).abs() < f64::EPSILON,
                    Comparator::NotEq => (f - target_f).abs() >= f64::EPSILON,
                    Comparator::Lt => f < target_f,
                    Comparator::Lte => f <= target_f,
                    Comparator::Gt => f > target_f,
                    Comparator::Gte => f >= target_f,
                }
            } else {
                false
            }
        }
        Value::Bool(b) => compare_bool(*b, comp, target),
        Value::String(s) => compare_string(s, comp, target),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_condition;

    #[test]
    fn test_eval_status_and_time() {
        let ctx = EvaluationContext::new(200, Duration::from_millis(150));
        let cond1 = parse_condition("[STATUS] == 200").unwrap();
        let cond2 = parse_condition("[RESPONSE_TIME] < 200").unwrap();
        let cond3 = parse_condition("[STATUS] == 500").unwrap();

        assert!(cond1.evaluate(&ctx));
        assert!(cond2.evaluate(&ctx));
        assert!(!cond3.evaluate(&ctx));
    }

    #[test]
    fn test_eval_json_body() {
        let json_data = br#"{"status": "UP", "data": {"items": [1, 2, 3], "code": 42}}"#;
        let ctx = EvaluationContext::new(200, Duration::from_millis(50)).with_body(json_data);

        let c1 = parse_condition("[BODY].status == UP").unwrap();
        let c2 = parse_condition("[BODY].data.code == 42").unwrap();
        let c3 = parse_condition("len([BODY].data.items) == 3").unwrap();
        let c4 = parse_condition("has([BODY], \"UP\") == true").unwrap();

        assert!(c1.evaluate(&ctx));
        assert!(c2.evaluate(&ctx));
        assert!(c3.evaluate(&ctx));
        assert!(c4.evaluate(&ctx));
    }

    #[test]
    fn test_eval_certificate_expiration() {
        let ctx = EvaluationContext::new(200, Duration::from_millis(50))
            .with_certificate_expiration(Duration::from_secs(72 * 3600));

        let c1 = parse_condition("[CERTIFICATE_EXPIRATION] > 48h").unwrap();
        let c2 = parse_condition("[CERTIFICATE_EXPIRATION] < 24h").unwrap();

        assert!(c1.evaluate(&ctx));
        assert!(!c2.evaluate(&ctx));
    }
}
