//! The human's attention, server-wide: one event when a top-level session's
//! turn ends (its answer, a question, a workflow waiting for them), so the UI
//! can notify wherever the human is; child sessions are a workflow's inner
//! steps and stay silent.

use super::App;
use serde_json::json;
use std::io::Write;
use std::net::TcpStream;
use std::time::{Duration, Instant};

/// A comment line now and then, so a closed browser tab is noticed.
const PING: Duration = Duration::from_secs(15);

pub fn serve(app: &App, stream: &mut TcpStream) -> Result<(), String> {
    let store = app.store()?;
    // No backlog of old notices: only turns that were running when the human
    // connected and then end should reach them.
    let mut previous = app.turns.running_ids();
    stream
        .write_all(
            b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\nConnection: keep-alive\r\n\r\n",
        )
        .map_err(|e| e.to_string())?;
    let mut last_write = Instant::now();
    loop {
        let seen = app.notifier.generation();
        let current = app.turns.running_ids();
        let mut out = String::new();
        for id in previous.iter().filter(|id| !current.contains(*id)) {
            if let Some(session) = store.session(id)?
                && session.parent_id.is_none()
            {
                out.push_str(&format!(
                    "data: {}\n\n",
                    json!({
                        "sessionId": session.id,
                        "title": session.title,
                        "projectId": session.project_id,
                    })
                ));
            }
        }
        previous = current;
        if out.is_empty() && last_write.elapsed() >= PING {
            out.push_str(": ping\n\n");
        }
        if !out.is_empty() {
            if stream.write_all(out.as_bytes()).is_err() {
                return Ok(()); // the browser went away
            }
            last_write = Instant::now();
        }
        app.notifier.wait_past(seen, PING);
    }
}
