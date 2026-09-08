//! Fast parser for Gatus condition grammar into typed ASTs.

use crate::ast::{Comparator, Condition, Literal, Placeholder};
use ratus_core::error::{RatusError, Result};

/// Parse a raw condition expression string into a compiled `Condition`.
pub fn parse_condition(raw: &str) -> Result<Condition> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(RatusError::Evaluation(
            "Condition expression cannot be empty".to_string(),
        ));
    }

    // Find the comparison operator
    let (comp_op, comp_idx, op_len) = find_comparator(trimmed)?;
    let (left, right) = (
        &trimmed[..comp_idx].trim(),
        &trimmed[comp_idx + op_len..].trim(),
    );

    if left.is_empty() {
        return Err(RatusError::Evaluation(format!(
            "Missing left-hand placeholder in condition '{raw}'"
        )));
    }
    if right.is_empty() {
        return Err(RatusError::Evaluation(format!(
            "Missing right-hand target in condition '{raw}'"
        )));
    }

    let placeholder = parse_placeholder(left)?;
    let target = parse_literal(right);

    Ok(Condition {
        raw: raw.to_string(),
        placeholder,
        comparator: comp_op,
        target,
    })
}

/// Locate comparator and its byte index.
fn find_comparator(input: &str) -> Result<(Comparator, usize, usize)> {
    // Check 2-char comparators first
    let two_char_ops = [
        ("==", Comparator::Eq),
        ("!=", Comparator::NotEq),
        ("<=", Comparator::Lte),
        (">=", Comparator::Gte),
    ];

    let mut best_match: Option<(Comparator, usize, usize)> = None;

    for (op_str, op_enum) in two_char_ops {
        if let Some(idx) = input.find(op_str) {
            match best_match {
                None => best_match = Some((op_enum, idx, op_str.len())),
                Some((_, best_idx, _)) if idx < best_idx => {
                    best_match = Some((op_enum, idx, op_str.len()));
                }
                _ => {}
            }
        }
    }

    if let Some(res) = best_match {
        return Ok(res);
    }

    // Check 1-char comparators: '<', '>'
    // Make sure it's not inside brackets or quotes
    let one_char_ops = [('<', Comparator::Lt), ('>', Comparator::Gt)];

    for (op_char, op_enum) in one_char_ops {
        if let Some(idx) = input.find(op_char) {
            match best_match {
                None => best_match = Some((op_enum, idx, 1)),
                Some((_, best_idx, _)) if idx < best_idx => {
                    best_match = Some((op_enum, idx, 1));
                }
                _ => {}
            }
        }
    }

    best_match.ok_or_else(|| {
        RatusError::Evaluation(format!(
            "No valid comparison operator (==, !=, <, <=, >, >=) found in condition '{input}'"
        ))
    })
}

/// Parse left-hand placeholder or function call.
fn parse_placeholder(input: &str) -> Result<Placeholder> {
    let s = input.trim();

    // Check for `len(...)`
    if let Some(inner) = s.strip_prefix("len(").and_then(|r| r.strip_suffix(')')) {
        let inner_ph = parse_placeholder(inner.trim())?;
        return Ok(Placeholder::Length(Box::new(inner_ph)));
    }

    // Check for `has(ph, "target")`
    if let Some(inner) = s.strip_prefix("has(").and_then(|r| r.strip_suffix(')')) {
        if let Some((ph_str, pat_str)) = split_has_arguments(inner) {
            let inner_ph = parse_placeholder(ph_str.trim())?;
            let pattern = unquote(pat_str.trim());
            return Ok(Placeholder::Has(Box::new(inner_ph), pattern));
        }
    }

    // Check for standard placeholders
    if s.starts_with("[STATUS]") {
        Ok(Placeholder::Status)
    } else if s.starts_with("[RESPONSE_TIME]") {
        Ok(Placeholder::ResponseTime)
    } else if let Some(rest) = s.strip_prefix("[BODY]") {
        let path = rest.strip_prefix('.').map(|p| p.trim().to_string());
        Ok(Placeholder::Body(path))
    } else if let Some(rest) = s.strip_prefix("[HEADERS]") {
        let name = rest
            .strip_prefix('.')
            .or_else(|| rest.strip_prefix('[').and_then(|r| r.strip_suffix(']')))
            .map(|n| n.trim().to_ascii_lowercase())
            .unwrap_or_default();
        Ok(Placeholder::Headers(name))
    } else if s.starts_with("[CERTIFICATE_EXPIRATION]") {
        Ok(Placeholder::CertificateExpiration)
    } else if s.starts_with("[CONNECTED]") {
        Ok(Placeholder::Connected)
    } else if s.starts_with("[IP]") {
        Ok(Placeholder::Ip)
    } else if s.starts_with("[DNS_RCODE]") {
        Ok(Placeholder::DnsRcode)
    } else {
        Err(RatusError::Evaluation(format!(
            "Unrecognized placeholder in expression '{s}'"
        )))
    }
}

/// Helper to split arguments in `has(ph, "pat")`
fn split_has_arguments(input: &str) -> Option<(&str, &str)> {
    let comma_idx = input.find(',')?;
    Some((&input[..comma_idx], &input[comma_idx + 1..]))
}

/// Parse right-hand side literal target value.
fn parse_literal(input: &str) -> Literal {
    let raw = input.trim();

    // Check for boolean
    if raw.eq_ignore_ascii_case("true") {
        return Literal::Bool(true);
    }
    if raw.eq_ignore_ascii_case("false") {
        return Literal::Bool(false);
    }

    // Check for duration (e.g. 48h, 30s)
    if let Ok(dur) = ratus_core::duration::parse_duration(raw) {
        // If it was purely numeric without duration unit, parse_duration fails, so this is genuine duration
        if raw.ends_with(|c: char| c.is_alphabetic()) {
            return Literal::Duration(dur);
        }
    }

    // Check for integer
    if let Ok(val) = raw.parse::<i64>() {
        return Literal::Int(val);
    }

    // Check for float
    if let Ok(val) = raw.parse::<f64>() {
        return Literal::Float(val);
    }

    // String literal
    Literal::String(unquote(raw))
}

/// Strip enclosing single or double quotes if present.
fn unquote(s: &str) -> String {
    let trimmed = s.trim();
    if (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
    {
        if trimmed.len() >= 2 {
            trimmed[1..trimmed.len() - 1].to_string()
        } else {
            String::new()
        }
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_parse_status() {
        let cond = parse_condition("[STATUS] == 200").unwrap();
        assert_eq!(cond.placeholder, Placeholder::Status);
        assert_eq!(cond.comparator, Comparator::Eq);
        assert_eq!(cond.target, Literal::Int(200));
    }

    #[test]
    fn test_parse_response_time() {
        let cond = parse_condition("[RESPONSE_TIME] < 250").unwrap();
        assert_eq!(cond.placeholder, Placeholder::ResponseTime);
        assert_eq!(cond.comparator, Comparator::Lt);
        assert_eq!(cond.target, Literal::Int(250));
    }

    #[test]
    fn test_parse_body_path() {
        let cond = parse_condition("[BODY].data.status == UP").unwrap();
        assert_eq!(
            cond.placeholder,
            Placeholder::Body(Some("data.status".to_string()))
        );
        assert_eq!(cond.comparator, Comparator::Eq);
        assert_eq!(cond.target, Literal::String("UP".to_string()));
    }

    #[test]
    fn test_parse_certificate_expiration() {
        let cond = parse_condition("[CERTIFICATE_EXPIRATION] > 48h").unwrap();
        assert_eq!(cond.placeholder, Placeholder::CertificateExpiration);
        assert_eq!(cond.comparator, Comparator::Gt);
        assert_eq!(
            cond.target,
            Literal::Duration(Duration::from_secs(48 * 3600))
        );
    }

    #[test]
    fn test_parse_functions() {
        let cond = parse_condition("has([BODY], \"SERVING\") == true").unwrap();
        assert_eq!(
            cond.placeholder,
            Placeholder::Has(Box::new(Placeholder::Body(None)), "SERVING".to_string())
        );
        assert_eq!(cond.comparator, Comparator::Eq);
        assert_eq!(cond.target, Literal::Bool(true));

        let len_cond = parse_condition("len([BODY].items) > 0").unwrap();
        assert_eq!(
            len_cond.placeholder,
            Placeholder::Length(Box::new(Placeholder::Body(Some("items".to_string()))))
        );
        assert_eq!(len_cond.comparator, Comparator::Gt);
        assert_eq!(len_cond.target, Literal::Int(0));
    }
}
