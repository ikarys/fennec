//! Fennec rule model and command-pattern matching.
//!
//! This crate is intentionally fennec-free: it performs no I/O and owns no
//! database. It only represents rules and evaluates in-memory command, tool,
//! and path-rule metadata, so another product can reuse it without pulling in
//! fennec runtime concerns.

pub mod command;
pub mod rule;

pub use command::{CommandPattern, PatternError, segments};
pub use rule::{Action, NO_RULES, Rule, Rules, Scope, Target, TargetKind};
