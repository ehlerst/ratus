//! Abstract Syntax Tree representations for Gatus condition expressions.

use std::time::Duration;

/// Condition placeholder identifying which attribute of the probe result to inspect.
#[derive(Debug, Clone, PartialEq)]
pub enum Placeholder {
    /// HTTP status code or exit code, e.g. `[STATUS]`.
    Status,

    /// Response time in milliseconds, e.g. `[RESPONSE_TIME]`.
    ResponseTime,

    /// Entire body or a JSONPath/dot-delimited path, e.g. `[BODY]` or `[BODY].data.status`.
    Body(Option<String>),

    /// Response header value, e.g. `[HEADERS].content-type`.
    Headers(String),

    /// TLS certificate time remaining until expiration, e.g. `[CERTIFICATE_EXPIRATION]`.
    CertificateExpiration,

    /// Whether the network socket connected successfully, e.g. `[CONNECTED]`.
    Connected,

    /// Resolved IP address, e.g. `[IP]`.
    Ip,

    /// DNS response code, e.g. `[DNS_RCODE]`.
    DnsRcode,

    /// Length of a body string or JSON array, e.g. `len([BODY].items)`.
    Length(Box<Placeholder>),

    /// Substring or array element containment check, e.g. `has([BODY], "UP")`.
    Has(Box<Placeholder>, String),
}

/// Relational comparison operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Comparator {
    /// Equal `==`
    Eq,
    /// Not equal `!=`
    NotEq,
    /// Less than `<`
    Lt,
    /// Less than or equal `<=`
    Lte,
    /// Greater than `>`
    Gt,
    /// Greater than or equal `>=`
    Gte,
}

/// Target literal value to compare the placeholder against.
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    /// Integer number (e.g. 200, 0, -1).
    Int(i64),

    /// Floating point number (e.g. 250.5).
    Float(f64),

    /// Boolean flag (`true` or `false`).
    Bool(bool),

    /// String literal (e.g. "UP", "OK", "application/json").
    String(String),

    /// Human-readable duration (e.g. 48h, 30s).
    Duration(Duration),
}

/// A parsed, compiled condition expression ready for microsecond evaluation.
#[derive(Debug, Clone, PartialEq)]
pub struct Condition {
    /// Original raw condition string.
    pub raw: String,
    /// Placeholder specifying the probe variable.
    pub placeholder: Placeholder,
    /// Comparison operator.
    pub comparator: Comparator,
    /// Target value to match.
    pub target: Literal,
}
