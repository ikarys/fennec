//! The human's own tools opened from the UI, and the project's dev servers.

pub(super) mod editor;
pub(super) mod file_manager;
pub(super) mod servers;
pub(super) mod terminal;
#[cfg(test)]
// The test file is protected and writes `Preferences::default()` + a field.
#[allow(clippy::field_reassign_with_default)]
mod terminal_test;
