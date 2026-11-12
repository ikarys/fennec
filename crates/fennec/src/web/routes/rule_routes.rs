//! The rules routes (contract: `docs/devs/api.md`): the human manages rules here; no
//! agent tool reaches them. The `rules` crate owns the rules; this module only
//! translates HTTP to its calls.

use crate::web::http::HttpError;
use std::collections::HashMap;
use std::net::TcpStream;

use serde_json::{Value, json};

use crate::domain::kernel_rules;
use crate::web::App;
use crate::web::http::{Request, error, send_json};
use rules::{Access, Action, NewRule, Origin, Rule, RuleStore, Scope, TargetKind};

const MAX_PATTERN_CHARS: usize = 500;
const MAX_NOTE_CHARS: usize = 1_000;

/// `GET /api/rules`: every rule, grouped for the human's management screen.
pub fn list(app: &App, stream: &mut TcpStream) -> Result<(), HttpError> {
    let rules = rule_store(app)?.list().map_err(|e| e.to_string())?;
    let store = app.store()?;

    let mut global: Vec<Value> = Vec::new();
    let mut projects: Vec<ProjectGroup> = Vec::new();
    let mut sessions: Vec<SessionGroup> = Vec::new();
    let mut project_index: HashMap<String, usize> = HashMap::new();
    let mut session_index: HashMap<String, usize> = HashMap::new();

    for rule in &rules {
        match &rule.scope {
            Scope::Global => global.push(view(rule)),
            Scope::Project(id) => {
                let index = match project_index.get(id) {
                    Some(&index) => index,
                    None => {
                        let name = store
                            .project(id)?
                            .map(|project| Value::String(project.name))
                            .unwrap_or(Value::Null);
                        projects.push(ProjectGroup {
                            id: id.clone(),
                            name,
                            rules: Vec::new(),
                        });
                        let index = projects.len() - 1;
                        project_index.insert(id.clone(), index);
                        index
                    }
                };
                projects[index].rules.push(view(rule));
            }
            Scope::Session(id) => {
                let index = match session_index.get(id) {
                    Some(&index) => index,
                    None => {
                        let (title, project_id) = match store.session(id)? {
                            Some(session) => (
                                Value::String(session.title),
                                Value::String(session.project_id),
                            ),
                            None => (Value::Null, Value::Null),
                        };
                        sessions.push(SessionGroup {
                            id: id.clone(),
                            title,
                            project_id,
                            rules: Vec::new(),
                        });
                        let index = sessions.len() - 1;
                        session_index.insert(id.clone(), index);
                        index
                    }
                };
                sessions[index].rules.push(view(rule));
            }
        }
    }

    let projects = projects
        .into_iter()
        .map(|group| {
            json!({
                "id": group.id,
                "name": group.name,
                "rules": group.rules,
            })
        })
        .collect::<Vec<Value>>();
    let sessions = sessions
        .into_iter()
        .map(|group| {
            json!({
                "id": group.id,
                "title": group.title,
                "projectId": group.project_id,
                "rules": group.rules,
            })
        })
        .collect::<Vec<Value>>();

    send_json(
        stream,
        200,
        &json!({
            // Read-only: the UI can show why a call was refused by a rule nobody can delete.
            "fennec": kernel_rules::all().iter().map(view).collect::<Vec<_>>(),
            "global": global,
            "projects": projects,
            "sessions": sessions,
        }),
    )
}

/// `POST /api/rules`: add a human rule.
pub fn add(app: &App, req: &Request, stream: &mut TcpStream) -> Result<(), HttpError> {
    let store = app.store()?;
    let body = req.json()?;
    let rule = match parse_new_rule(&body, |id| {
        store
            .project(id)
            .map(|project| project.is_some())
            .map_err(|e| e.to_string())
    }) {
        Ok(rule) => rule,
        Err(AddError::Store(message)) => return Err(message.into()),
        Err(AddError::ProjectNotFound) => return error(stream, 404, "project not found"),
        Err(AddError::Bad(message)) => return error(stream, 400, &message),
    };

    match rule_store(app)?.add(rule).map_err(|e| e.to_string())? {
        Ok(rule) => send_json(stream, 201, &json!({ "rule": view(&rule) })),
        Err(pattern_error) => error(stream, 400, &pattern_error.0),
    }
}

/// `POST /api/rules/{id}/delete`: delete a rule by its numeric id.
pub fn delete(app: &App, stream: &mut TcpStream, id: &str) -> Result<(), HttpError> {
    let Ok(id) = id.parse::<i64>() else {
        return error(stream, 404, "rule not found");
    };
    let deleted = rule_store(app)?.delete(id).map_err(|e| e.to_string())?;
    if !deleted {
        return error(stream, 404, "rule not found");
    }
    send_json(stream, 200, &json!({ "ok": true }))
}

/// `GET /api/projects/{id}/rules`: the project page's read-only summary.
pub fn list_project(app: &App, stream: &mut TcpStream, project_id: &str) -> Result<(), HttpError> {
    let store = app.store()?;
    if store.project(project_id)?.is_none() {
        return error(stream, 404, "project not found");
    }

    let rules = rule_store(app)?.list().map_err(|e| e.to_string())?;
    let project = rules
        .iter()
        .filter(|rule| matches!(&rule.scope, Scope::Project(id) if id == project_id))
        .map(view)
        .collect::<Vec<Value>>();
    let global = rules
        .iter()
        .filter(|rule| matches!(&rule.scope, Scope::Global))
        .map(view)
        .collect::<Vec<Value>>();

    send_json(stream, 200, &json!({ "rules": project, "global": global }))
}

fn rule_store(app: &App) -> Result<RuleStore, String> {
    RuleStore::open(&app.settings.rules_path()).map_err(|e| e.to_string())
}

/// The human's input error is a 400; a missing project is a 404; a store failure
/// is an outer server error.
#[derive(Debug)]
enum AddError {
    Bad(String),
    ProjectNotFound,
    Store(String),
}

fn parse_new_rule(
    body: &Value,
    project_exists: impl Fn(&str) -> Result<bool, String>,
) -> Result<NewRule, AddError> {
    let scope = parse_scope(body, project_exists)?;
    let kind = serde_json::from_value::<TargetKind>(body["kind"].clone())
        .map_err(|_| AddError::Bad("kind must be command, tool or path".to_string()))?;
    let action = serde_json::from_value::<Action>(body["action"].clone())
        .map_err(|_| AddError::Bad("action must be allow, ask or block".to_string()))?;
    let pattern = body["pattern"].as_str().unwrap_or("").trim();
    if pattern.is_empty() {
        return Err(AddError::Bad("pattern is required".into()));
    }
    if pattern.chars().count() > MAX_PATTERN_CHARS {
        return Err(AddError::Bad(format!(
            "pattern: at most {MAX_PATTERN_CHARS} characters"
        )));
    }
    let access =
        if matches!(kind, TargetKind::Path) {
            match &body["access"] {
                Value::Null => None,
                Value::String(text) => Some(Access::parse(text).ok_or_else(|| {
                    AddError::Bad("access must be read, write or readWrite".into())
                })?),
                _ => {
                    return Err(AddError::Bad(
                        "access must be read, write or readWrite".into(),
                    ));
                }
            }
        } else {
            None
        };
    let note = match &body["note"] {
        Value::Null => None,
        Value::String(text) if text.chars().count() <= MAX_NOTE_CHARS => Some(text.clone()),
        Value::String(_) => {
            return Err(AddError::Bad(format!(
                "note: at most {MAX_NOTE_CHARS} characters"
            )));
        }
        _ => return Err(AddError::Bad("note must be a string".into())),
    };

    Ok(NewRule {
        scope,
        kind,
        pattern: pattern.to_string(),
        access,
        action,
        note,
        origin: Origin::Human,
    })
}

fn parse_scope(
    body: &Value,
    project_exists: impl Fn(&str) -> Result<bool, String>,
) -> Result<Scope, AddError> {
    match &body["scope"] {
        Value::String(scope) if scope == "global" => Ok(Scope::Global),
        Value::Object(object) => {
            let Some(project_id) = object.get("project").and_then(Value::as_str) else {
                return Err(AddError::Bad(
                    "scope must be \"global\" or { \"project\": \"<id>\" }".into(),
                ));
            };
            match project_exists(project_id) {
                Ok(true) => Ok(Scope::Project(project_id.to_string())),
                Ok(false) => Err(AddError::ProjectNotFound),
                Err(message) => Err(AddError::Store(message)),
            }
        }
        _ => Err(AddError::Bad(
            "scope must be \"global\" or { \"project\": \"<id>\" }".into(),
        )),
    }
}

/// `understood` is how the `rules` crate read the pattern, so the UI shows it
/// without recomputing the semantics.
fn view(rule: &Rule) -> Value {
    json!({
        "id": rule.id,
        "scope": rule.scope.label(),
        "kind": rule.target.kind(),
        "pattern": rule.target.pattern(),
        "access": rule.target.access(),
        "action": rule.action,
        "note": rule.note,
        "origin": origin_json(&rule.origin),
        "understood": rule.target.describe(),
    })
}

fn origin_json(origin: &Origin) -> Value {
    match origin {
        Origin::Human => json!("human"),
        Origin::Fennec => json!("fennec"),
        Origin::Answer { session_id } => json!({ "session": session_id }),
    }
}

struct ProjectGroup {
    id: String,
    name: Value,
    rules: Vec<Value>,
}

struct SessionGroup {
    id: String,
    title: Value,
    project_id: Value,
    rules: Vec<Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_valid_path_rule_with_access_read() {
        let body = json!({
            "scope": "global",
            "kind": "path",
            "pattern": "src/**",
            "access": "read",
            "action": "allow"
        });

        let rule = parse_new_rule(&body, |_| Ok(true)).unwrap();

        assert!(matches!(rule.scope, Scope::Global));
        assert!(matches!(rule.kind, TargetKind::Path));
        assert_eq!(rule.pattern, "src/**");
        assert!(matches!(rule.access, Some(Access::Read)));
        assert!(matches!(rule.action, Action::Allow));
        assert!(rule.note.is_none());
        assert!(matches!(rule.origin, Origin::Human));
    }

    #[test]
    fn a_missing_pattern_is_rejected() {
        let body = json!({
            "scope": "global",
            "kind": "command",
            "action": "block"
        });

        match parse_new_rule(&body, |_| Ok(true)) {
            Err(AddError::Bad(message)) => assert_eq!(message, "pattern is required"),
            other => panic!("expected a bad pattern error, got {other:?}"),
        }
    }

    #[test]
    fn a_bad_action_is_rejected() {
        let body = json!({
            "scope": "global",
            "kind": "command",
            "pattern": "git",
            "action": "nope"
        });

        match parse_new_rule(&body, |_| Ok(true)) {
            Err(AddError::Bad(message)) => {
                assert_eq!(message, "action must be allow, ask or block")
            }
            other => panic!("expected a bad action error, got {other:?}"),
        }
    }

    #[test]
    fn access_is_ignored_on_a_command_rule() {
        let body = json!({
            "scope": "global",
            "kind": "command",
            "pattern": "git",
            "access": "bogus",
            "action": "block"
        });

        let rule = parse_new_rule(&body, |_| Ok(true)).unwrap();

        assert!(rule.access.is_none());
    }
}
