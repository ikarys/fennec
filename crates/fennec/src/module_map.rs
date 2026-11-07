//! The project map's source files (those git tracks, read from the workdir)
//! and its committed copy, `docs/map.md`.

use std::path::Path;

use crate::domain::map_doc::{self, MAP_FILE, MapFileState};
use crate::domain::module_map::{self, ModuleMap, SourceFile};

/// Larger files are generated or vendored: not worth reading for imports.
const MAX_FILE_BYTES: u64 = 1 << 20;

/// The map of the project's tracked files; empty outside a git repository.
pub fn load(workdir: &Path) -> ModuleMap {
    module_map::build(&tracked_files(workdir))
}

/// The committed map file's text, when present.
pub fn committed_map_file(workdir: &Path) -> Option<String> {
    std::fs::read_to_string(workdir.join(MAP_FILE)).ok()
}

/// How `docs/map.md` compares with the map of the code as it is.
pub fn map_file_state(workdir: &Path, map: &ModuleMap) -> MapFileState {
    let on_disk = committed_map_file(workdir);
    map_doc::state(on_disk.as_deref(), &map_doc::render(map))
}

/// The human's click: `docs/map.md` written in their tree, not committed.
pub fn write_map_file(workdir: &Path, map: &ModuleMap) -> std::io::Result<()> {
    let path = workdir.join(MAP_FILE);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, map_doc::render(map))
}

pub(crate) fn tracked_files(workdir: &Path) -> Vec<SourceFile> {
    let Some(out) = crate::git::stdout(workdir, &["ls-files", "-z"]) else {
        return Vec::new();
    };
    out.split('\0')
        .filter(|path| module_map::is_source_path(path))
        .filter_map(|path| {
            let full = workdir.join(path);
            if full.metadata().ok()?.len() > MAX_FILE_BYTES {
                return None;
            }
            let content = std::fs::read_to_string(full).ok()?;
            Some(SourceFile {
                path: path.to_string(),
                content,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_this_repository() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let map = load(&root);
        let has = |from: &str, to: &str| {
            map.dependencies
                .iter()
                .any(|dependency| dependency.from == from && dependency.to == to)
        };
        assert!(has("crates/fennec/src/web", "crates/fennec/src/domain"));
        assert!(has("web/src/pages", "web/src/lib"));
        // Strings in test fixtures are not imports.
        assert!(!has("crates/fennec/src/domain", "crates/fennec/src/web"));
        assert!(has("crates/fennec/src/web/views", "crates/token-usage/src"));
    }
}
