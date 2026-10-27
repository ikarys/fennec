//! The application layer: the agent loop, written against the ports in
//! `ports` so that storage, model and tools stay replaceable.

mod ask_user;
mod compaction;
mod delta_buffer;
pub mod ports;
mod prompt;
mod propose_criteria;
mod round;
#[cfg(test)]
mod test_support;
mod tool_calls;
pub mod turn;
pub mod workflow;
