//! Fennec's rule engine.
//!
//! This crate owns the human's rules: the model, the matching, the decision, and the
//! storage in its own SQLite file. A host only asks the crate what to do and enforces
//! the answer.

mod answer;
pub mod command;
mod decide;
pub mod path;
mod rule;
mod store;

pub use answer::{Proposal, proposal};
pub use command::{CommandPattern, PatternError, segments};
pub use decide::{Decision, PathRequest, Request};
pub use rule::{
    Access, Action, NO_RULES, Origin, PathPattern, Rule, Rules, Scope, Target, TargetKind,
};
pub use store::{NewRule, RuleStore, StoreError};
