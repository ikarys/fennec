//! The project's architecture rules on disk.

use std::path::Path;

use crate::domain::architecture::Rules;

/// The project's rules: `.agents/architecture` when present, none otherwise.
pub fn load(workdir: &Path) -> Rules {
    let content = std::fs::read_to_string(workdir.join(".agents").join("architecture"));
    Rules::parse(&content.unwrap_or_default())
}
