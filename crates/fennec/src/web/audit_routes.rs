//! A session's audit report turned into backlog ideas (contract: docs/api.md), on the
//! human's click only: the client names finding numbers, the server writes the ideas'
//! text from the report in the log, and records the creation in the session's log.

use std::net::TcpStream;

use serde_json::json;

use super::App;
use super::http::{Request, error, send_json};
use super::notifier::NotifyingLog;
use super::routes::session_and_project;
use crate::agent::ports::EventLog;
use crate::domain::audit;
use crate::domain::conversation::last_assistant_text;
use crate::domain::event::Event;
use crate::domain::id::new_id;
use backlog::BacklogEvent;

/// `POST /api/sessions/{id}/audit/tasks` `{findings: [number]}` → `{created}`.
pub fn create_tasks(
    app: &App,
    req: &Request,
    stream: &mut TcpStream,
    session_id: &str,
) -> Result<(), String> {
    let store = app.store()?;
    let Some((session, project)) = session_and_project(&store, session_id)? else {
        return error(stream, 404, "no such session");
    };

    let body = req.json()?;
    // Every entry must be a finding number: a malformed list is refused whole.
    let requested: Option<Vec<u32>> = body["findings"].as_array().and_then(|values| {
        values
            .iter()
            .map(|value| value.as_u64().and_then(|n| u32::try_from(n).ok()))
            .collect()
    });
    let mut numbers = match requested {
        Some(numbers) if !numbers.is_empty() => numbers,
        _ => return error(stream, 400, "no finding selected"),
    };
    numbers.sort_unstable();
    numbers.dedup();

    let events = store.events(&session.id)?;
    let taken = audit::taken(&events);
    if numbers.iter().any(|number| taken.contains(number)) {
        return error(stream, 409, "finding already turned into a task");
    }
    let Some(audit) = audit::parse(&last_assistant_text(&events)) else {
        return error(stream, 400, "no audit in this session");
    };

    for number in &numbers {
        if !audit
            .findings
            .iter()
            .any(|finding| &finding.number == number)
        {
            return error(stream, 400, "unknown finding");
        }
    }

    let mut created = 0;
    let mut idea_ids = Vec::new();
    for finding in &audit.findings {
        if numbers.contains(&finding.number) {
            let idea_id = new_id();
            store.append_idea_event(
                &project.id,
                &BacklogEvent::Added {
                    idea_id: idea_id.clone(),
                    text: audit.task_text(finding),
                    attachments: Vec::new(),
                },
            )?;
            idea_ids.push(idea_id);
            created += 1;
        }
    }

    let log = NotifyingLog {
        log: &store,
        notifier: &app.notifier,
    };
    log.append(
        &session.id,
        &Event::AuditTasksCreated {
            findings: numbers,
            idea_ids,
        },
    )?;

    send_json(stream, 200, &json!({ "created": created }))
}
