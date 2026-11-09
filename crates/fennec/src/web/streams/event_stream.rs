//! Server-Sent Events: a session's stored log from `Last-Event-ID` (or
//! `?after=`), then new events as they are appended, woken by the notifier.

use crate::notifier::Notifier;
use crate::store::SqliteStore;
use crate::web::http::Request;
use crate::web::http::{HttpError, require_session};
use std::io::Write;
use std::net::TcpStream;
use std::time::{Duration, Instant};

/// A comment line now and then, so a closed browser tab is noticed.
const PING: Duration = Duration::from_secs(15);

pub fn serve(
    store: &SqliteStore,
    notifier: &Notifier,
    req: &Request,
    stream: &mut TcpStream,
    id: &str,
) -> Result<(), HttpError> {
    require_session(store, id)?;
    let mut last = resume_point(req);
    stream
        .write_all(
            b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\nConnection: keep-alive\r\n\r\n",
        )
        .map_err(|e| e.to_string())?;
    let mut last_write = Instant::now();
    loop {
        // Read the generation before the store, so an append in between
        // still wakes the wait below.
        let seen = notifier.generation();
        let mut out = String::new();
        for (seq, payload) in store.events_after(id, last)? {
            let payload = crate::web::jobs::run_stats_backfill::backfilled(store, id, seq, payload);
            // Payloads are single-line JSON: serde_json never emits raw newlines.
            out.push_str(&format!("id: {seq}\ndata: {payload}\n\n"));
            last = seq;
        }
        if out.is_empty() && last_write.elapsed() >= PING {
            out.push_str(": ping\n\n");
        }
        if !out.is_empty() {
            if stream.write_all(out.as_bytes()).is_err() {
                return Ok(()); // the browser went away
            }
            last_write = Instant::now();
        }
        notifier.wait_past(seen, PING);
    }
}

fn resume_point(req: &Request) -> u64 {
    req.header("last-event-id")
        .map(str::to_string)
        .or_else(|| {
            req.query
                .split('&')
                .find_map(|kv| kv.strip_prefix("after=").map(str::to_string))
        })
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}
