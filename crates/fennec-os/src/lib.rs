pub mod agent;
pub mod chat;
pub mod grant;
pub mod guard;
pub mod outside;
pub mod plugin;
pub mod proof;
pub mod provider;
pub mod recipe;
pub mod review;
pub mod review_stage;
pub mod rules;
pub mod schedule;
pub mod serve;
pub mod skill;
pub mod sleep;
pub mod solve;
pub mod spec;
pub mod team;
pub mod tools;
pub mod world;

/// Tests that move process-wide variables (`HOME`, `XDG_CONFIG_HOME`) hold
/// this while they do, whichever module they are in.
#[cfg(test)]
pub(crate) static ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());
