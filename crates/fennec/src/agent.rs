//! The application layer: the agent loop, written against the ports in
//! `ports` so that storage, model and tools stay replaceable.

mod delta_buffer;
pub mod ports;
mod prompt;
mod round;
mod tool_calls;
pub mod turn;
