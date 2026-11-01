//! The human's answer to a `rule.approval_requested`: store the chosen rule,
//! then run the recorded call in a turn.

use super::App;
use super::http::{Request, error, send_json};
use super::turns::Job;
use crate::agent::ports::EventLog;
use crate::domain::rule_approval::{Choice, pending};
use rules::{Access, Action, NewRule, Origin, RuleStore, Scope, TargetKind};
use serde_json::json;
use std::net::TcpStream;

pub(super) fn answer_rule_approval(
    app: &App,
    req: &Request,
    stream: &mut TcpStream,
    id: &str,
) -> Result<(), String> {
    let body = req.json()?;
    let call_id = body["callId"].as_str().unwrap_or("").to_string();
    let store = app.store()?;
    let Some((session, project)) = app.session_and_project(id)? else {
        return error(stream, 404, "no such session");
    };
    if app.turns.is_running(id) {
        return error(stream, 409, "a turn is already running");
    }
    let Some(choice) = Choice::parse(body["choice"].as_str().unwrap_or("")) else {
        return error(stream, 400, "unknown choice");
    };
    let events = store.events(id)?;
    let Some(pending) = pending(&events).filter(|pending| pending.call_id == call_id) else {
        return error(stream, 409, "no pending approval for this call");
    };

    // What the answer stores: nothing for "once", a session grant, or a project rule.
    let stored = match choice {
        Choice::Once => None,
        Choice::Session => Some((Scope::Session(id.to_string()), Action::Allow)),
        Choice::Always => Some((Scope::Project(project.id.clone()), Action::Allow)),
        Choice::Never => Some((Scope::Project(project.id.clone()), Action::Block)),
    };
    let rule_id = match stored {
        None => None,
        Some((scope, action)) => {
            let Some(kind) = TargetKind::parse(&pending.kind) else {
                return error(stream, 400, &format!("unknown rule kind: {}", pending.kind));
            };
            let rule_store =
                RuleStore::open(&app.settings.rules_path()).map_err(|e| e.to_string())?;
            let new_rule = NewRule {
                scope,
                kind,
                pattern: pending.pattern.clone(),
                access: pending.access.as_deref().and_then(Access::parse),
                action,
                note: None,
                origin: Origin::Answer {
                    session_id: id.to_string(),
                },
            };
            match rule_store.add(new_rule) {
                Ok(Ok(rule)) => Some(rule.id),
                Ok(Err(e)) => return error(stream, 400, &e.to_string()),
                Err(e) => return error(stream, 500, &e.to_string()),
            }
        }
    };

    let place = app.place(&store, &session, &project)?;
    match app.turns.start(
        id,
        place,
        Job::Approval {
            call_id,
            choice,
            rule_id,
        },
    ) {
        Ok(()) => send_json(stream, 202, &json!({})),
        Err(e) => error(stream, 409, &e),
    }
}
