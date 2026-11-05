//! The human's plugin definitions (`~/.agents/plugins.json`) and the
//! validity of an activation: a plugin runs only when the human activated it
//! and its definition and code have not changed since. A changed definition
//! or a changed file deactivates it, so a clone of the home runs nothing.
//! Plugins are global: nothing is read from a project's `.agents`.

use crate::domain::plugin::{PluginActivation, PluginDef, code_hash};
use std::path::{Path, PathBuf};

/// The human's usable plugin definitions, empty when
/// `<agents_home>/plugins.json` is absent or unreadable; a definition with
/// problems is left out.
pub fn read_defs(agents_home: &Path) -> Vec<PluginDef> {
    read_defs_checked(agents_home).0
}

/// The usable definitions and why the rest were left out: the file's read or
/// parse error, or a definition's problems. An absent file is no problem.
/// Settings shows them, so a typo never reads as "no plugin defined".
pub fn read_defs_checked(agents_home: &Path) -> (Vec<PluginDef>, Vec<String>) {
    let text = match std::fs::read_to_string(agents_home.join("plugins.json")) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (Vec::new(), Vec::new()),
        Err(e) => return (Vec::new(), vec![format!("plugins.json: {e}")]),
    };
    let defs: Vec<PluginDef> = match serde_json::from_str(&text) {
        Ok(defs) => defs,
        Err(e) => return (Vec::new(), vec![format!("plugins.json: {e}")]),
    };
    let mut problems = Vec::new();
    let usable = defs
        .into_iter()
        .filter(|def| {
            let found = def.problems();
            problems.extend(found.iter().map(|p| format!("{}: {p}", def.name)));
            found.is_empty()
        })
        .collect();
    (usable, problems)
}

/// The plugin's code directory, `<agents_home>/plugins/<name>/`.
pub fn code_dir(agents_home: &Path, name: &str) -> PathBuf {
    agents_home.join("plugins").join(name)
}

/// The code directory's files as (relative path, content), sorted by path.
/// Empty when the directory is absent.
pub fn code_files(agents_home: &Path, name: &str) -> Vec<(String, String)> {
    let dir = code_dir(agents_home, name);
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
pub fn compute_hash(agents_home: &Path, def: &PluginDef) -> String {
    code_hash(def, &code_files(agents_home, &def.name))
}

/// Whether a plugin is active: the human enabled it, and its definition and
/// code have not changed since it was activated (the stored hash matches).
pub fn is_active(
    agents_home: &Path,
    def: &PluginDef,
    activation: Option<&PluginActivation>,
) -> bool {
    match activation {
        Some(a) if a.enabled && a.name == def.name => compute_hash(agents_home, def) == a.code_hash,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write(dir: &Path, rel: &str, content: &str) {
        let path = dir.join(rel);
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
    fn a_broken_definitions_file_says_why() {
        let dir =
            std::env::temp_dir().join(format!("fennec-plugin-cfg-{}", crate::domain::id::new_id()));
        write(
            &dir,
            "plugins.json",
            r#"[{"name":"x","command":"python3","blocks":[{"id":"b","scope":"nowhere"}]}]"#,
        );
        let (defs, problems) = read_defs_checked(&dir);
        assert!(defs.is_empty());
        assert_eq!(problems.len(), 1);
        assert!(
            problems[0].starts_with("plugins.json: unknown variant `nowhere`"),
            "{problems:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_project_definitions_file_is_ignored() {
        // Plugins are global: a project's `.agents/plugins.json` is never
        // read, only the agents home's.
        let base =
            std::env::temp_dir().join(format!("fennec-plugin-cfg-{}", crate::domain::id::new_id()));
        let agents_home = base.join("agents");
        let workdir = base.join("repo");
        write(
            &agents_home,
            "plugins.json",
            r#"[{"name":"counter","command":"python3","args":["counter.py"]}]"#,
        );
        write(
            &workdir,
            ".agents/plugins.json",
            r#"[{"name":"local","command":"python3","args":["local.py"]}]"#,
        );

        let defs = read_defs(&agents_home);
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "counter");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_changed_definition_or_code_deactivates() {
        let dir =
            std::env::temp_dir().join(format!("fennec-plugin-cfg-{}", crate::domain::id::new_id()));
        write(&dir, "plugins.json", "[]");
        write(&dir, "plugins/counter/counter.py", "print(1)");

        let d = def("counter");
        let hash = compute_hash(&dir, &d);
        let activation = PluginActivation {
            name: "counter".into(),
            path: "plugins/counter".into(),
            code_hash: hash.clone(),
            network: false,
            enabled: true,
        };
        assert!(is_active(&dir, &d, Some(&activation)));

        // A changed code file changes the hash, so the activation no longer matches.
        write(&dir, "plugins/counter/counter.py", "print(2)");
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
        write(&dir, "plugins/counter/counter.py", "print(1)");

        let d = def("counter");
        let hash = compute_hash(&dir, &d);
        let activation = PluginActivation {
            name: "counter".into(),
            path: "plugins/counter".into(),
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
        write(&dir, "plugins/counter/counter.py", "print(1)");

        let d = def("counter");
        let activation = PluginActivation {
            name: "counter".into(),
            path: "plugins/counter".into(),
            code_hash: compute_hash(&dir, &d),
            network: false,
            enabled: true,
        };
        assert!(is_active(&dir, &d, Some(&activation)));

        write(&dir, "plugins/counter/counter.py", "print(2)");
        assert!(!is_active(&dir, &d, Some(&activation)));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
