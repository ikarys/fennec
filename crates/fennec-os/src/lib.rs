//! fennec's library, split by trust zone (see the repository's AGENTS.md).
//!
//! `kernel`: runs on the host and decides — what agents may run, read and
//! write, the worlds' boundary, rules, plugins, the review queue. It never
//! runs what a model wrote.
//!
//! `engine`: drives the models — the agent loop, its tools, the gateway,
//! framing, solving, proof, roles. The code is trusted; everything it gets
//! from a model is not, and runs only inside a world.

pub mod engine;
pub mod kernel;
pub mod serve;

// Flat paths (`fennec_os::guard`, `crate::world`) stay valid while modules
// still mix zones; the kernel's surface narrows when it is hardened (step 3).
pub use engine::{
    agent, analysis, chat, proof, provider, recipe, review_stage, skill, sleep, solve, spec, team,
    tools,
};
pub use kernel::{grant, guard, knowledge, outside, plugin, review, rules, schedule, world, zones};

/// Tests that move process-wide variables (`HOME`, `XDG_CONFIG_HOME`) hold
/// this while they do, whichever module they are in.
#[cfg(test)]
pub(crate) static ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());
