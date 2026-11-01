//! The rules routes (contract: `docs/api.md`): the human manages rules here; no
//! agent tool reaches them. `routes.rs` only dispatches here.

use std::net::TcpStream;

use serde_json::{Value, json};

use super::App;
use super::http::{Request, error, send_json};
use crate::store::NewRule;
use rules::{Action, Rule, Scope, Target, TargetKind};

const MAX_PATTERN_CHARS: usize = 500;
const MAX_NOTE_CHARS: usize = 1_000;

pub fn list_global(app: &App, stream: &mut TcpStream) -> Result<(), String> {
    let rules = app.store()?.global_rules()?;
    send_json(stream, 200, &json!({ "rules": views(&rules) }))
}

pub fn add_global(app: &App, req: &Request, stream: &mut TcpStream) -> Result<(), String> {
    if let Err(message) = add(app, req, Scope::Global)? {
        return error(stream, 400, &message);
    }
    list_global(app, stream)
}

pub fn delete(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), String> {
    if !app.store()?.delete_rule(id)? {
        return error(stream, 404, "rule not found");
    }
    send_json(stream, 200, &json!({ "ok": true }))
}

/// The project's rules, and the global ones that apply to it too (shown read-only).
pub fn list_project(app: &App, stream: &mut TcpStream, project_id: &str) -> Result<(), String> {
    let store = app.store()?;
    if store.project(project_id)?.is_none() {
        return error(stream, 404, "project not found");
    }
    let (project, global) = (store.project_rules(project_id)?, store.global_rules()?);
    send_json(
        stream,
        200,
        &json!({ "rules": views(&project), "global": views(&global) }),
    )
}

pub fn add_project(
    app: &App,
    req: &Request,
    stream: &mut TcpStream,
    project_id: &str,
) -> Result<(), String> {
    if app.store()?.project(project_id)?.is_none() {
        return error(stream, 404, "project not found");
    }
    if let Err(message) = add(app, req, Scope::Project(project_id.to_string()))? {
        return error(stream, 400, &message);
    }
    list_project(app, stream, project_id)
}

/// Outer error: the server failed; inner error: the human's input, a 400.
fn add(app: &App, req: &Request, scope: Scope) -> Result<Result<(), String>, String> {
    let rule = match parse_new_rule(&req.json()?, scope) {
        Ok(rule) => rule,
        Err(message) => return Ok(Err(message)),
    };
    Ok(app.store()?.add_rule(rule)?.map(|_| ()).map_err(|e| e.0))
}

fn parse_new_rule(body: &Value, scope: Scope) -> Result<NewRule, String> {
    let kind = serde_json::from_value::<TargetKind>(body["kind"].clone())
        .map_err(|_| "kind must be command, tool or path".to_string())?;
    let action = serde_json::from_value::<Action>(body["action"].clone())
        .map_err(|_| "action must be block or ask".to_string())?;
    let pattern = body["pattern"].as_str().unwrap_or("").trim();
    if pattern.is_empty() {
        return Err("pattern is required".into());
    }
    if pattern.chars().count() > MAX_PATTERN_CHARS {
        return Err(format!("pattern: at most {MAX_PATTERN_CHARS} characters"));
    }
    let note = match &body["note"] {
        Value::Null => None,
        Value::String(text) if text.trim().is_empty() => None,
        Value::String(text) if text.chars().count() <= MAX_NOTE_CHARS => {
            Some(text.trim().to_string())
        }
        Value::String(_) => return Err(format!("note: at most {MAX_NOTE_CHARS} characters")),
        _ => return Err("note must be a string".into()),
    };
    Ok(NewRule {
        scope,
        kind,
        pattern: pattern.to_string(),
        action,
        note,
    })
}

fn views(rules: &[Rule]) -> Vec<Value> {
    rules.iter().map(view).collect()
}

/// `understood` is how fennec read the pattern, so the UI shows it without computing it.
fn view(rule: &Rule) -> Value {
    let understood = match &rule.target {
        Target::Command(command) => command.describe(),
        Target::Tool(name) => format!("tool {name}"),
        Target::Path(path) => format!("writes to {path}"),
    };
    json!({
        "id": rule.id,
        "scope": if matches!(rule.scope, Scope::Global) { "global" } else { "project" },
        "kind": rule.target.kind(),
        "pattern": rule.target.pattern(),
        "action": rule.action,
        "note": rule.note,
        "understood": understood,
    })
}
