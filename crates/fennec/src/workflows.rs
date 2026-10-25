//! Workflow discovery: embedded defaults, then `~/.agents/workflows`, then
//! the project's `.agents/workflows`, which overrides a workflow of the same
//! id. Like `agents.rs`.

use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::workflow::definition::Workflow;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Origin {
    Project,
    Global,
    Default,
}

pub struct Found {
    pub workflow: Workflow,
    pub origin: Origin,
    /// The file as read, unknown fields included.
    pub raw: Value,
}

const DEFAULTS: [&str; 1] = [include_str!("../workflows/plan-build-verify.workflow.json")];

/// The workflows a session in `workdir` can use.
pub fn discover(workdir: &Path) -> Vec<Found> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    discover_in(home.as_deref(), workdir)
}

fn discover_in(home: Option<&Path>, workdir: &Path) -> Vec<Found> {
    let mut found = Vec::new();

    for content in DEFAULTS {
        if let Some(workflow) =
            load_workflow("plan-build-verify.workflow.json", content, Origin::Default)
        {
            insert_or_override(&mut found, workflow);
        }
    }

    if let Some(home) = home {
        let dir = home.join(".agents").join("workflows");
        for workflow in load_workflows_directory(&dir, Origin::Global) {
            insert_or_override(&mut found, workflow);
        }
    }

    let dir = workdir.join(".agents").join("workflows");
    for workflow in load_workflows_directory(&dir, Origin::Project) {
        insert_or_override(&mut found, workflow);
    }

    found.sort_by(|a, b| a.workflow.name.cmp(&b.workflow.name));
    found
}

/// Missing dir → empty, as in `agents.rs`.
fn load_workflows_directory(dir: &Path, origin: Origin) -> Vec<Found> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    let mut entries: Vec<_> = entries.filter_map(|entry| entry.ok()).collect();
    entries.sort_by_key(|entry| entry.file_name());

    let mut found = Vec::new();
    for entry in entries {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.ends_with(".workflow.json") {
            continue;
        }

        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(_) => continue,
        };

        if let Some(workflow) = load_workflow(name, &content, origin) {
            found.push(workflow);
        }
    }

    found
}

/// Parses one file; skips it (naming the file) when it does not parse or its
/// id differs from the file name.
fn load_workflow(file_name: &str, content: &str, origin: Origin) -> Option<Found> {
    let raw = match serde_json::from_str::<Value>(content) {
        Ok(raw) => raw,
        Err(e) => {
            eprintln!("fennec: skipping {file_name}: {e}");
            return None;
        }
    };
    let workflow = match serde_json::from_value::<Workflow>(raw.clone()) {
        Ok(workflow) => workflow,
        Err(e) => {
            eprintln!("fennec: skipping {file_name}: {e}");
            return None;
        }
    };
    let id = file_name
        .strip_suffix(".workflow.json")
        .unwrap_or(file_name);
    if workflow.id != id {
        eprintln!("fennec: skipping {file_name}: id is {}", workflow.id);
        return None;
    }
    Some(Found {
        workflow,
        origin,
        raw,
    })
}

fn insert_or_override(found: &mut Vec<Found>, new: Found) {
    if let Some(slot) = found
        .iter()
        .position(|existing| existing.workflow.id == new.workflow.id)
    {
        found[slot] = new;
    } else {
        found.push(new);
    }
}

pub fn find(workdir: &Path, id: &str) -> Option<Found> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    discover_in(home.as_deref(), workdir)
        .into_iter()
        .find(|found| found.workflow.id == id)
}

pub fn save(workdir: &Path, origin: Origin, raw: &Value) -> Result<PathBuf, String> {
    let id = raw.get("id").and_then(Value::as_str).unwrap_or("");
    if !valid_id(id) {
        return Err(format!("invalid workflow id: {id}"));
    }
    let dir = match origin {
        Origin::Project => workdir.join(".agents").join("workflows"),
        Origin::Global => {
            let home = PathBuf::from(std::env::var_os("HOME").ok_or("no home directory")?);
            home.join(".agents").join("workflows")
        }
        Origin::Default => return Err("cannot save an embedded default".into()),
    };
    fs::create_dir_all(&dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
    let path = dir.join(format!("{id}.workflow.json"));
    let pretty =
        serde_json::to_string_pretty(raw).map_err(|e| format!("serializing workflow: {e}"))?;
    fs::write(&path, format!("{pretty}\n"))
        .map_err(|e| format!("writing {}: {e}", path.display()))?;
    Ok(path)
}

pub fn valid_id(id: &str) -> bool {
    if id.len() > 64 {
        return false;
    }
    let mut chars = id.chars();
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() || first.is_ascii_digit() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::id::new_id;

    fn fixture() -> (PathBuf, PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!("fennec-workflows-{}", new_id()));
        let home = root.join("home");
        let workdir = root.join("workdir");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&workdir).unwrap();
        (root, home, workdir)
    }

    fn workflows_dir(base: &Path) -> PathBuf {
        let dir = base.join(".agents").join("workflows");
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_workflow(dir: &Path, id: &str, name: &str) {
        let raw = serde_json::json!({
            "id": id,
            "name": name,
            "nodes": [
                { "id": "start", "kind": "start", "position": { "x": 0, "y": 0 } },
                { "id": "end", "kind": "end", "outcome": "done", "position": { "x": 0, "y": 100 } }
            ],
            "edges": [
                { "from": "start", "port": "next", "to": "end" }
            ]
        });
        std::fs::write(
            dir.join(format!("{id}.workflow.json")),
            serde_json::to_string(&raw).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn project_overrides_global_overrides_default() {
        let (root, home, workdir) = fixture();
        let home_dir = workflows_dir(&home);
        let work_dir = workflows_dir(&workdir);
        write_workflow(&home_dir, "plan-build-verify", "Global PBC");
        write_workflow(&work_dir, "plan-build-verify", "Project PBC");
        write_workflow(&work_dir, "custom", "Custom");

        let found = discover_in(Some(&home), &workdir);
        let pbc = found
            .iter()
            .find(|f| f.workflow.id == "plan-build-verify")
            .unwrap();
        assert_eq!(pbc.origin, Origin::Project);
        assert_eq!(pbc.workflow.name, "Project PBC");
        let custom = found.iter().find(|f| f.workflow.id == "custom").unwrap();
        assert_eq!(custom.origin, Origin::Project);
        let names: Vec<_> = found.iter().map(|f| f.workflow.name.clone()).collect();
        assert_eq!(names, vec!["Custom", "Project PBC"]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn global_overrides_default() {
        let (root, home, workdir) = fixture();
        let home_dir = workflows_dir(&home);
        write_workflow(&home_dir, "plan-build-verify", "Global PBC");

        let found = discover_in(Some(&home), &workdir);
        let pbc = found
            .iter()
            .find(|f| f.workflow.id == "plan-build-verify")
            .unwrap();
        assert_eq!(pbc.origin, Origin::Global);
        assert_eq!(pbc.workflow.name, "Global PBC");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_bad_file_is_skipped() {
        let (root, home, workdir) = fixture();
        let work_dir = workflows_dir(&workdir);
        std::fs::write(
            work_dir.join("other.workflow.json"),
            r#"{ "id": "mismatch", "name": "x", "nodes": [], "edges": [] }"#,
        )
        .unwrap();
        std::fs::write(work_dir.join("broken.workflow.json"), "{ not json").unwrap();

        let found = discover_in(Some(&home), &workdir);
        assert!(found.iter().all(|f| f.workflow.id != "mismatch"));
        assert!(found.iter().all(|f| f.workflow.id != "broken"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn save_then_find_round_trips_unknown_fields() {
        let (root, _home, workdir) = fixture();
        let raw = serde_json::json!({
            "id": "my-flow",
            "name": "My Flow",
            "unknownField": { "nested": [1, 2, 3] },
            "nodes": [
                { "id": "start", "kind": "start", "position": { "x": 0, "y": 0 } },
                { "id": "end", "kind": "end", "outcome": "done", "position": { "x": 0, "y": 100 } }
            ],
            "edges": [
                { "from": "start", "port": "next", "to": "end" }
            ]
        });
        save(&workdir, Origin::Project, &raw).unwrap();
        let found = find(&workdir, "my-flow").unwrap();
        assert_eq!(found.origin, Origin::Project);
        assert_eq!(
            found.raw["unknownField"]["nested"],
            serde_json::json!([1, 2, 3])
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_embedded_default_validates() {
        let (_root, _home, workdir) = fixture();
        let found = discover_in(None, &workdir);
        let default = found
            .iter()
            .find(|f| f.workflow.id == "plan-build-verify")
            .unwrap();
        let agents = crate::agents::discover(&workdir);
        let issues = crate::domain::workflow::validate::validate(&default.workflow, &agents);
        assert!(issues.is_empty(), "issues: {issues:?}");
    }
}
