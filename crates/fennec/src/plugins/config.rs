//! The project's plugin definitions (`.agents/plugins.json`) and the validity
//! of an activation: a plugin runs only when the human activated it and its
//! definition and code have not changed since. A changed definition or a
//! changed file deactivates it, so a clone of a repository runs nothing.

use crate::domain::plugin::{PluginActivation, PluginDef, code_hash};
use std::path::{Path, PathBuf};

/// The project's usable plugin definitions, empty when `.agents/plugins.json`
/// is absent or unreadable; a definition with problems is left out.
pub fn read_defs(workdir: &Path) -> Vec<PluginDef> {
    let path = workdir.join(".agents").join("plugins.json");
    let defs: Vec<PluginDef> = std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    defs.into_iter()
        .filter(|d| d.problems().is_empty())
        .collect()
}

/// The plugin's code directory, `.agents/plugins/<name>/`.
pub fn code_dir(workdir: &Path, name: &str) -> PathBuf {
    workdir.join(".agents").join("plugins").join(name)
}

/// The code directory's files as (relative path, content), sorted by path.
/// Empty when the directory is absent.
pub fn code_files(workdir: &Path, name: &str) -> Vec<(String, String)> {
    let dir = code_dir(workdir, name);
    let mut out = Vec::new();
    collect(&dir, &dir, &mut out);
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn collect(root: &Path, path: &Path, out: &mut Vec<(String, String)>) {
    let Ok(entries) = std::fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(root, &path, out);
        } else if let (Ok(content), Ok(rel)) =
            (std::fs::read_to_string(&path), path.strip_prefix(root))
        {
            out.push((rel.to_string_lossy().into_owned(), content));
        }
    }
}

/// The hash of a definition and its code, recomputed from disk.
pub fn compute_hash(workdir: &Path, def: &PluginDef) -> String {
    code_hash(def, &code_files(workdir, &def.name))
}

/// Whether a plugin is active: the human enabled it, and its definition and
/// code have not changed since it was activated (the stored hash matches).
pub fn is_active(workdir: &Path, def: &PluginDef, activation: Option<&PluginActivation>) -> bool {
    match activation {
        Some(a) if a.enabled && a.name == def.name => compute_hash(workdir, def) == a.code_hash,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write(workdir: &Path, rel: &str, content: &str) {
        let path = workdir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let mut file = std::fs::File::create(path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
    }

    fn def(name: &str) -> PluginDef {
        PluginDef {
            name: name.into(),
            command: "python3".into(),
            args: vec![format!("{name}.py")],
            env: Default::default(),
            blocks: Vec::new(),
        }
    }

    #[test]
    fn an_absent_definitions_file_reads_as_empty() {
        let dir =
            std::env::temp_dir().join(format!("fennec-plugin-cfg-{}", crate::domain::id::new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(read_defs(&dir).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_changed_definition_or_code_deactivates() {
        let dir =
            std::env::temp_dir().join(format!("fennec-plugin-cfg-{}", crate::domain::id::new_id()));
        write(&dir, ".agents/plugins.json", "[]");
        write(&dir, ".agents/plugins/counter/counter.py", "print(1)");

        let d = def("counter");
        let hash = compute_hash(&dir, &d);
        let activation = PluginActivation {
            name: "counter".into(),
            path: ".agents/plugins/counter".into(),
            code_hash: hash.clone(),
            network: false,
            enabled: true,
        };
        assert!(is_active(&dir, &d, Some(&activation)));

        // A changed code file changes the hash, so the activation no longer matches.
        write(&dir, ".agents/plugins/counter/counter.py", "print(2)");
        assert!(!is_active(&dir, &d, Some(&activation)));

        // A disabled activation is not active even with a matching hash.
        let off = PluginActivation {
            enabled: false,
            ..activation.clone()
        };
        let hash2 = compute_hash(&dir, &d);
        assert!(!is_active(
            &dir,
            &d,
            Some(&PluginActivation {
                code_hash: hash2,
                ..off
            })
        ));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn plugin_activation() {
        // A changed definition deactivates the plugin: the stored hash no
        // longer matches the definition on disk.
        let dir =
            std::env::temp_dir().join(format!("fennec-plugin-act-{}", crate::domain::id::new_id()));
        write(&dir, ".agents/plugins/counter/counter.py", "print(1)");

        let d = def("counter");
        let hash = compute_hash(&dir, &d);
        let activation = PluginActivation {
            name: "counter".into(),
            path: ".agents/plugins/counter".into(),
            code_hash: hash,
            network: false,
            enabled: true,
        };
        assert!(is_active(&dir, &d, Some(&activation)));

        let mut changed = d.clone();
        changed.command = "python3.12".into();
        assert!(!is_active(&dir, &changed, Some(&activation)));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn plugin_code_changed() {
        // A changed code file deactivates the plugin: the hash covers the
        // definition and the code directory's contents.
        let dir = std::env::temp_dir().join(format!(
            "fennec-plugin-code-{}",
            crate::domain::id::new_id()
        ));
        write(&dir, ".agents/plugins/counter/counter.py", "print(1)");

        let d = def("counter");
        let activation = PluginActivation {
            name: "counter".into(),
            path: ".agents/plugins/counter".into(),
            code_hash: compute_hash(&dir, &d),
            network: false,
            enabled: true,
        };
        assert!(is_active(&dir, &d, Some(&activation)));

        write(&dir, ".agents/plugins/counter/counter.py", "print(2)");
        assert!(!is_active(&dir, &d, Some(&activation)));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
