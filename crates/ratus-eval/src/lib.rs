//! High-speed, zero-allocation condition evaluation engine for Ratus.

#![deny(missing_docs)]
#![deny(clippy::all)]

pub mod ast;
pub mod context;
pub mod eval;
pub mod parser;

pub use ast::{Comparator, Condition, Literal, Placeholder};
pub use context::EvaluationContext;
pub use eval::resolve_json_path;
pub use parser::parse_condition;
