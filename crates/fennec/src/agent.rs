//! The application layer: the agent loop, written against the ports in
//! `ports` so that storage, model and tools stay replaceable.

mod ask_user;
mod compaction;
mod delta_buffer;
pub mod ports;
mod preflight;
mod prompt;
mod propose_criteria;
mod propose_task;
mod queue;
mod record_decision;
mod request_waiver;
mod round;
pub mod rule_approval;
mod rule_check;
mod suggest_agent;
#[cfg(test)]
mod test_support;
mod tool_calls;
pub mod turn;
pub mod waiver;
pub mod workflow;
