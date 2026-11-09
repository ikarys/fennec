//! The files a session's file tools changed, counted from its `tool.call` events: the
//! context panel's badge and its section read the same totals and listed files.

use super::event::Event;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

/// One file a session's tools changed, with its line counts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModifiedFile {
    pub path: String,
    pub added: usize,
    pub removed: usize,
}

/// What a session's file tools changed: distinct files and line counts.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModifiedFiles {
    pub count: usize,
    pub added: usize,
    pub removed: usize,
    /// The most-changed files, at most `LISTED_FILES`, most lines first.
    pub files: Vec<ModifiedFile>,
}

/// The maximum number of files `ModifiedFiles::files` lists.
pub const LISTED_FILES: usize = 5;

/// Folds the `tool.call` events of `events`; `workdir` is the session's working directory.
pub fn tally(events: &[Event], workdir: &str) -> ModifiedFiles {
    let mut files: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let (mut added, mut removed) = (0, 0);
    for event in events {
        if let Event::ToolCall { tool_call, .. } = event {
            let args = &tool_call.arguments;
            match tool_call.name.as_str() {
                "write_file" => {
                    if let Some(path) = arg_str(args, "path").filter(|p| !p.is_empty()) {
                        let added_lines = line_count(arg_str(args, "content").unwrap_or(""));
                        let entry = files.entry(file_key(path, workdir)).or_default();
                        entry.0 += added_lines;
                        added += added_lines;
                    }
                }
                "edit_file" => {
                    if let Some(path) = arg_str(args, "path").filter(|p| !p.is_empty()) {
                        let added_lines = line_count(arg_str(args, "new_string").unwrap_or(""));
                        let removed_lines = line_count(arg_str(args, "old_string").unwrap_or(""));
                        let entry = files.entry(file_key(path, workdir)).or_default();
                        entry.0 += added_lines;
                        entry.1 += removed_lines;
                        added += added_lines;
                        removed += removed_lines;
                    }
                }
                _ => {}
            }
        }
    }
    let count = files.len();
    let mut listed: Vec<ModifiedFile> = files
        .into_iter()
        .map(|(path, (file_added, file_removed))| ModifiedFile {
            path,
            added: file_added,
            removed: file_removed,
        })
        .collect();
    listed.sort_by(|a, b| {
        let total_a = a.added + a.removed;
        let total_b = b.added + b.removed;
        total_b.cmp(&total_a).then_with(|| a.path.cmp(&b.path))
    });
    listed.truncate(LISTED_FILES);
    ModifiedFiles {
        count,
        added,
        removed,
        files: listed,
    }
}

fn arg_str<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key).and_then(Value::as_str)
}

// JS split(/\r?\n/).length is the number of \n pieces; stripping \r does not change it.
fn line_count(value: &str) -> usize {
    value.split('\n').count()
}

fn file_key(path: &str, workdir: &str) -> String {
    relative_to_workdir(path, workdir).unwrap_or_else(|| path.to_string())
}

fn relative_to_workdir(path: &str, workdir: &str) -> Option<String> {
    let mut workdir = workdir.replace('\\', "/");
    while workdir.ends_with('/') {
        workdir.pop();
    }
    if workdir.is_empty() {
        return None;
    }
    let path = path.replace('\\', "/");
    let is_absolute = path.starts_with('/') || is_drive_absolute(&path);
    if is_absolute {
        let prefix = format!("{workdir}/");
        if !path.starts_with(&prefix) {
            return None;
        }
        clean_relative(&path[prefix.len()..])
    } else {
        clean_relative(&path)
    }
}

fn is_drive_absolute(path: &str) -> bool {
    path.len() >= 2 && path.as_bytes()[0].is_ascii_alphabetic() && path.as_bytes()[1] == b':'
}

// Drop '' and '.'; any '..' is ambiguous, so the raw path is used as the key.
fn clean_relative(path: &str) -> Option<String> {
    let mut segments = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => return None,
            segment => segments.push(segment),
        }
    }
    if segments.is_empty() {
        None
    } else {
        Some(segments.join("/"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::event::ToolCall;
    use serde_json::json;

    fn call(name: &str, arguments: Value) -> Event {
        Event::ToolCall {
            message_id: "m".into(),
            tool_call: ToolCall {
                id: "t".into(),
                name: name.into(),
                arguments,
                parse_error: None,
                raw_arguments: None,
            },
        }
    }

    fn assert_totals(events: &[Event], workdir: &str, count: usize, added: usize, removed: usize) {
        let result = tally(events, workdir);
        assert_eq!(
            (result.count, result.added, result.removed),
            (count, added, removed)
        );
    }

    #[test]
    fn write_counts_its_lines() {
        let event = call(
            "write_file",
            json!({"path": "a.ts", "content": "line1\nline2"}),
        );
        assert_totals(&[event], "/w", 1, 2, 0);
    }

    #[test]
    fn repeated_writes_to_one_file_add_up() {
        let events = [
            call("write_file", json!({"path": "a.ts", "content": "one\ntwo"})),
            call("write_file", json!({"path": "a.ts", "content": "three"})),
        ];
        assert_totals(&events, "/w", 1, 3, 0);
    }

    #[test]
    fn edit_counts_new_and_old_lines() {
        let args = json!({"path": "a.ts", "old_string": "old1\nold2", "new_string": "new1"});
        let event = call("edit_file", args);
        assert_totals(&[event], "/w", 1, 1, 2);
    }

    #[test]
    fn read_changes_nothing() {
        let event = call("read_file", json!({"path": "a.ts"}));
        assert_totals(&[event], "/w", 0, 0, 0);
    }

    #[test]
    fn absolute_and_relative_forms_are_one_file() {
        let events = [
            call("write_file", json!({"path": "/w/src/a.rs", "content": "x"})),
            call("write_file", json!({"path": "./src/a.rs", "content": "x"})),
        ];
        assert_totals(&events, "/w", 1, 2, 0);
    }

    #[test]
    fn outside_workdir_path_is_kept_as_written() {
        let events = [
            call("write_file", json!({"path": "/other/x", "content": "x"})),
            call("write_file", json!({"path": "x", "content": "x"})),
        ];
        assert_totals(&events, "/w", 2, 2, 0);
    }

    #[test]
    fn files_list_the_most_changed_first() {
        let events = [
            call("write_file", json!({"path": "a.ts", "content": "one\ntwo"})),
            call(
                "write_file",
                json!({"path": "b.ts", "content": "one\ntwo\nthree\nfour"}),
            ),
        ];
        let result = tally(&events, "/w");
        assert_eq!(
            result.files,
            vec![
                ModifiedFile {
                    path: "b.ts".into(),
                    added: 4,
                    removed: 0,
                },
                ModifiedFile {
                    path: "a.ts".into(),
                    added: 2,
                    removed: 0,
                },
            ]
        );
    }

    #[test]
    fn files_stop_at_the_listed_cap() {
        let events: Vec<Event> = (0..LISTED_FILES + 2)
            .map(|i| {
                call(
                    "write_file",
                    json!({"path": format!("f{i}.ts"), "content": "x"}),
                )
            })
            .collect();
        let result = tally(&events, "/w");
        assert_eq!(result.count, LISTED_FILES + 2);
        assert_eq!(result.files.len(), LISTED_FILES);
    }
}
