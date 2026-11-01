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
mod record_decision;
mod round;
#[cfg(test)]
mod test_support;
mod tool_calls;
pub mod turn;
pub mod workflow;
