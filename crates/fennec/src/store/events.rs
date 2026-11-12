//! Event log: appends, paged and targeted reads, dangling-turn repair, and the `EventLog` impl.

use super::{Result, SqliteStore};
use crate::agent::ports::EventLog;
use crate::domain::clock::now_ms;
use crate::domain::event::{DoneReason, Event};
use rusqlite::{OptionalExtension, params};

impl SqliteStore {
    fn decode_events(rows: Vec<(u64, String)>) -> Vec<(u64, Event)> {
        rows.into_iter()
            .filter_map(|(seq, payload)| {
                // A payload this version cannot read is skipped, not fatal,
                // as in `events`.
                serde_json::from_str(&payload)
                    .ok()
                    .map(|event| (seq, event))
            })
            .collect()
    }

    /// One page of the log before `seq`, ascending, with sequence numbers: at
    /// least the last `limit` events, extended back to the start of the turn
    /// they begin in, so the UI never folds half a message or misses that a
    /// turn is running. A page can exceed `limit` when a turn is long.
    pub fn events_before(
        &self,
        session_id: &str,
        seq: u64,
        limit: u64,
    ) -> Result<Vec<(u64, Event)>> {
        let mut stmt = self.db.prepare_cached(
            "SELECT seq, payload FROM events
                 WHERE session_id = ?1 AND seq < ?2 AND seq >= COALESCE((
                   SELECT MAX(t.seq) FROM events t
                   WHERE t.session_id = ?1 AND t.event_type = 'running.changed'
                     AND json_extract(t.payload, '$.data.isRunning') = 1
                     AND t.seq <= (
                       SELECT MIN(seq) FROM (
                         SELECT seq FROM events
                         WHERE session_id = ?1 AND seq < ?2
                         ORDER BY seq DESC LIMIT ?3
                       )
                     )
                 ), 1)
                 ORDER BY seq",
        )?;
        // `u64::MAX` (no `before`) would cast to a negative i64 and match
        // nothing; the log's seqs fit in i64.
        let bound = seq.min(i64::MAX as u64) as i64;
        let rows = stmt.query_map(params![session_id, bound, limit as i64], |r| {
            Ok((r.get::<_, i64>(0)? as u64, r.get::<_, String>(1)?))
        })?;
        Ok(Self::decode_events(
            rows.collect::<rusqlite::Result<Vec<(u64, String)>>>()?,
        ))
    }

    /// The `workflow.*` events of the latest run started before `seq`
    /// (inclusive of its `workflow.started`), ascending; empty when there is
    /// no `workflow.started` before `seq`.
    pub fn workflow_run_before(&self, session_id: &str, seq: u64) -> Result<Vec<(u64, Event)>> {
        let mut stmt = self.db.prepare_cached(
            "SELECT seq, payload FROM events
                 WHERE session_id = ?1
                   AND seq < ?2
                   AND event_type LIKE 'workflow.%'
                   AND seq >= (
                     SELECT MAX(seq) FROM events
                     WHERE session_id = ?1
                       AND event_type = 'workflow.started'
                       AND seq < ?2
                   )
                 ORDER BY seq",
        )?;
        // `u64::MAX` (no `before`) would cast to a negative i64 and match
        // nothing; the log's seqs fit in i64.
        let bound = seq.min(i64::MAX as u64) as i64;
        let rows = stmt.query_map(params![session_id, bound], |r| {
            Ok((r.get::<_, i64>(0)? as u64, r.get::<_, String>(1)?))
        })?;
        Ok(Self::decode_events(
            rows.collect::<rusqlite::Result<Vec<(u64, String)>>>()?,
        ))
    }

    /// Raw stored payloads after `seq`, for streaming them as they are.
    pub fn events_after(&self, session_id: &str, seq: u64) -> Result<Vec<(u64, String)>> {
        let mut stmt = self.db.prepare_cached(
            "SELECT seq, payload FROM events WHERE session_id = ?1 AND seq > ?2 ORDER BY seq",
        )?;
        let rows = stmt.query_map(params![session_id, seq as i64], |r| {
            Ok((r.get::<_, i64>(0)? as u64, r.get(1)?))
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// The session's newest sequence number, 0 when the log is empty. The log
    /// is append-only, so it watermarks a fold: an unchanged max seq means an
    /// unchanged log.
    pub fn last_seq(&self, session_id: &str) -> Result<u64> {
        Ok(self.db.query_row(
            "SELECT COALESCE(MAX(seq), 0) FROM events WHERE session_id = ?1",
            [session_id],
            |r| r.get::<_, i64>(0),
        )? as u64)
    }

    /// The session's `running.changed` events at/after `since_ms`, in log order.
    pub fn running_changes(&self, session_id: &str, since_ms: u64) -> Result<Vec<(u64, bool)>> {
        let mut stmt = self.db.prepare_cached(
            "SELECT timestamp, payload FROM events WHERE session_id = ?1 AND event_type = 'running.changed' AND timestamp >= ?2 ORDER BY seq",
        )?;
        let rows = stmt.query_map(params![session_id, since_ms as i64], |r| {
            Ok((r.get::<_, i64>(0)? as u64, r.get::<_, String>(1)?))
        })?;
        Ok(rows
            .collect::<rusqlite::Result<Vec<(u64, String)>>>()?
            .into_iter()
            .filter_map(|(timestamp, payload)| {
                serde_json::from_str::<serde_json::Value>(&payload)
                    .ok()
                    .and_then(|value| value["data"]["isRunning"].as_bool())
                    .map(|is_running| (timestamp, is_running))
            })
            .collect())
    }

    /// The last agent a session switched to (`mode.changed`), or None. A
    /// targeted read, not the full log: listing a project must not deserialize
    /// every session's events.
    pub fn last_mode(&self, session_id: &str) -> Result<Option<String>> {
        Ok(self
            .db
            .query_row(
                "SELECT json_extract(payload, '$.data.mode') FROM events
                 WHERE session_id = ?1 AND event_type = 'mode.changed'
                 ORDER BY seq DESC LIMIT 1",
                [session_id],
                |r| r.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten())
    }

    /// Ends the turns a crash left open: sessions whose last `running.changed`
    /// says running get `isRunning: false`, so the UI stops waiting on a turn
    /// no process runs. Called at server start, before any turn exists.
    /// Returns how many were closed.
    // ponytail: a CLI turn running in another process at that moment is closed too.
    pub fn close_dangling_turns(&self) -> Result<usize> {
        let ids = self
            .db
            .prepare(
                "SELECT e.session_id FROM events e
                 WHERE e.event_type = 'running.changed'
                   AND json_extract(e.payload, '$.data.isRunning') = 1
                   AND e.seq = (SELECT MAX(seq) FROM events
                                WHERE session_id = e.session_id AND event_type = 'running.changed')",
            )?
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<String>>>()?;
        for id in &ids {
            self.end_cut_turn(id)?;
        }
        Ok(ids.len())
    }

    /// Ends a turn that was cut (a crash, a panic in its thread) like any other.
    pub fn end_cut_turn(&self, session_id: &str) -> Result<()> {
        // The cut turn ends like any other, with a done: otherwise the last
        // done in the log is an earlier one (a workflow's human node waiting
        // for the user), and the run looks waiting instead of interrupted.
        let message_id: String = self.db.query_row(
            "SELECT COALESCE(MAX(json_extract(payload, '$.data.messageId')), '')
             FROM events WHERE session_id = ?1 AND seq = (SELECT MAX(seq) FROM events
                 WHERE session_id = ?1 AND event_type = 'message.start')",
            params![session_id],
            |r| r.get(0),
        )?;
        self.insert_event(
            session_id,
            &Event::ChatDone {
                message_id,
                reason: DoneReason::Error,
                duration_ms: None,
            },
        )?;
        self.insert_event(session_id, &Event::RunningChanged { is_running: false })?;
        Ok(())
    }

    pub(super) fn insert_event(&self, session_id: &str, event: &Event) -> Result<u64> {
        let payload = serde_json::to_string(event).expect("events always serialize");
        Ok(self.db.query_row(
            "INSERT INTO events (session_id, seq, timestamp, event_type, payload)
             VALUES (?1, (SELECT COALESCE(MAX(seq), 0) + 1 FROM events WHERE session_id = ?1), ?2, ?3, ?4)
             RETURNING seq",
            params![session_id, now_ms() as i64, event.kind(), payload],
            |row| row.get::<_, i64>(0).map(|s| s as u64),
        )?)
    }

    fn payloads(&self, sql: &str, params: impl rusqlite::Params) -> Result<Vec<String>> {
        let mut stmt = self.db.prepare_cached(sql)?;
        let rows = stmt.query_map(params, |row| row.get(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}

impl EventLog for SqliteStore {
    fn append(&self, session_id: &str, event: &Event) -> std::result::Result<u64, String> {
        Ok(self.insert_event(session_id, event)?)
    }

    fn events(&self, session_id: &str) -> std::result::Result<Vec<Event>, String> {
        let payloads = self.events_after(session_id, 0)?;
        Ok(decoded(payloads.into_iter().map(|(_, payload)| payload)))
    }

    fn window(&self, session_id: &str) -> std::result::Result<Vec<Event>, String> {
        let payloads = self.payloads(
            "SELECT payload FROM events WHERE session_id = ?1 AND seq >= COALESCE(
               (SELECT MAX(seq) FROM events WHERE session_id = ?1 AND event_type = 'context.compacted'), 0)
             ORDER BY seq",
            [session_id],
        )?;
        Ok(decoded(payloads))
    }

    fn events_of(
        &self,
        session_id: &str,
        kinds: &[&str],
    ) -> std::result::Result<Vec<Event>, String> {
        if kinds.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = (2..=kinds.len() + 1)
            .map(|i| format!("?{i}"))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT payload FROM events WHERE session_id = ?1 AND event_type IN ({placeholders}) ORDER BY seq"
        );
        let params = std::iter::once(session_id).chain(kinds.iter().copied());
        Ok(decoded(
            self.payloads(&sql, rusqlite::params_from_iter(params))?,
        ))
    }
}

/// A payload this version cannot read is skipped, not fatal: the log may hold event types
/// ported later.
fn decoded(payloads: impl IntoIterator<Item = String>) -> Vec<Event> {
    payloads
        .into_iter()
        .filter_map(|payload| serde_json::from_str(&payload).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A store with one session, for the tests that append events.
    fn store_with_session() -> (SqliteStore, String) {
        let store = SqliteStore::in_memory().unwrap();
        let project = store.project_for("/tmp/targeted-reads").unwrap();
        let session = store.create_session(&project, "t").unwrap();
        (store, session)
    }

    fn cut() -> Event {
        Event::ContextCompacted {
            before_tokens: 0,
            after_tokens: 0,
            summary: "s".into(),
        }
    }

    #[test]
    fn window_starts_at_the_last_cut() {
        let (store, cut_session) = store_with_session();
        for event in [
            Event::RunningChanged { is_running: true },
            cut(),
            Event::RunningChanged { is_running: false },
            cut(),
            Event::QueueCancelled {
                queue_id: "q".into(),
            },
        ] {
            store.append(&cut_session, &event).unwrap();
        }
        assert_eq!(
            store.window(&cut_session).unwrap(),
            vec![
                cut(),
                Event::QueueCancelled {
                    queue_id: "q".into()
                }
            ]
        );

        let project = store.project_for("/tmp/targeted-reads-plain").unwrap();
        let plain = store.create_session(&project, "t").unwrap();
        store
            .append(&plain, &Event::RunningChanged { is_running: true })
            .unwrap();
        assert_eq!(store.window(&plain).unwrap(), store.events(&plain).unwrap());
    }

    #[test]
    fn events_of_keeps_the_kinds_asked() {
        let (store, session) = store_with_session();
        for event in [
            Event::RunningChanged { is_running: true },
            cut(),
            Event::QueueCancelled {
                queue_id: "q".into(),
            },
        ] {
            store.append(&session, &event).unwrap();
        }
        assert_eq!(
            store
                .events_of(&session, &["running.changed", "queue.cancelled"])
                .unwrap(),
            vec![
                Event::RunningChanged { is_running: true },
                Event::QueueCancelled {
                    queue_id: "q".into()
                }
            ]
        );
        assert!(store.events_of(&session, &[]).unwrap().is_empty());
    }
    #[test]
    fn events_round_trip_in_order_and_sessions_report_how_they_ended() {
        let store = SqliteStore::in_memory().unwrap();
        let project = store.project_for("/w").unwrap();
        assert_eq!(project.name, "w");
        assert_eq!(store.project_for("/w").unwrap().id, project.id);
        let sid = store.create_session(&project, "t").unwrap();
        let ev = Event::MessageDelta {
            message_id: "m".into(),
            content: "hi".into(),
            tokens_per_second: None,
        };
        assert_eq!(store.append(&sid, &ev).unwrap(), 2);
        let all = store.events(&sid).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[1], ev);
        let wire = serde_json::to_value(&ev).unwrap();
        assert_eq!(wire["type"], "message.delta");
        assert_eq!(wire["data"]["messageId"], "m");
        store
            .append(
                &sid,
                &Event::ChatDone {
                    message_id: "m".into(),
                    reason: DoneReason::Stopped,
                    duration_ms: None,
                },
            )
            .unwrap();
        let row = store.session(&sid).unwrap().unwrap();
        assert_eq!(row.last_done, Some(DoneReason::Stopped));
        assert_eq!(store.events_after(&sid, 2).unwrap().len(), 1);
    }

    #[test]
    fn last_seq_tracks_the_newest_event() {
        let store = SqliteStore::in_memory().unwrap();
        let project = store.project_for("/w").unwrap();
        let sid = store.create_session(&project, "s").unwrap();
        // A new session holds its init event at seq 1.
        assert_eq!(store.last_seq(&sid).unwrap(), 1);
        store
            .append(&sid, &Event::RunningChanged { is_running: true })
            .unwrap();
        store
            .append(&sid, &Event::RunningChanged { is_running: false })
            .unwrap();
        assert_eq!(store.last_seq(&sid).unwrap(), 3);
    }

    #[test]
    fn close_dangling_turns_ends_only_open_turns() {
        let store = SqliteStore::in_memory().unwrap();
        let project = store.project_for("/w").unwrap();
        let open = store.create_session(&project, "open").unwrap();
        let ended = store.create_session(&project, "ended").unwrap();
        store
            .append(&open, &Event::RunningChanged { is_running: true })
            .unwrap();
        for running in [true, false] {
            store
                .append(
                    &ended,
                    &Event::RunningChanged {
                        is_running: running,
                    },
                )
                .unwrap();
        }

        assert_eq!(store.close_dangling_turns().unwrap(), 1);
        assert_eq!(store.last_seq(&open).unwrap(), 4);
        let row = store.session(&open).unwrap().unwrap();
        assert_eq!(row.last_done, Some(DoneReason::Error));
        assert_eq!(store.last_seq(&ended).unwrap(), 3);
        assert_eq!(store.close_dangling_turns().unwrap(), 0);
    }

    #[test]
    fn events_before_starts_pages_at_a_turn_start() {
        let store = SqliteStore::in_memory().unwrap();
        let project = store.project_for("/w").unwrap();
        let sid = store.create_session(&project, "t").unwrap();
        for turn in 0..2 {
            store
                .append(&sid, &Event::RunningChanged { is_running: true })
                .unwrap();
            for i in 0..3 {
                store
                    .append(
                        &sid,
                        &Event::MessageDelta {
                            message_id: "m".into(),
                            content: format!("t{turn}c{i}"),
                            tokens_per_second: None,
                        },
                    )
                    .unwrap();
            }
            store
                .append(&sid, &Event::RunningChanged { is_running: false })
                .unwrap();
        }

        let page = store.events_before(&sid, u64::MAX, 2).unwrap();
        assert_eq!(
            page.iter().map(|(s, _)| *s).collect::<Vec<_>>(),
            vec![7, 8, 9, 10, 11]
        );

        let page = store.events_before(&sid, 7, 2).unwrap();
        assert_eq!(
            page.iter().map(|(s, _)| *s).collect::<Vec<_>>(),
            vec![2, 3, 4, 5, 6]
        );

        let page = store.events_before(&sid, 2, 10).unwrap();
        assert_eq!(page.iter().map(|(s, _)| *s).collect::<Vec<_>>(), vec![1]);

        let page = store.events_before(&sid, u64::MAX, 100).unwrap();
        assert_eq!(page.len(), 11);
        assert_eq!(
            page.iter().map(|(s, _)| *s).collect::<Vec<_>>(),
            (1..=11).collect::<Vec<_>>()
        );

        assert!(store.events_before(&sid, 1, 10).unwrap().is_empty());
        assert!(store.events_before("missing", 10, 10).unwrap().is_empty());
    }

    #[test]
    fn workflow_run_before_returns_only_the_latest_run_before_seq() {
        let store = SqliteStore::in_memory().unwrap();
        let project = store.project_for("/w").unwrap();
        let sid = store.create_session(&project, "t").unwrap();

        fn workflow_started(run_id: &str) -> Event {
            let workflow: crate::domain::workflow::definition::Workflow =
                serde_json::from_value(serde_json::json!({
                    "id": "w",
                    "name": "w",
                    "nodes": [],
                    "edges": []
                }))
                .unwrap();
            Event::WorkflowStarted {
                run_id: run_id.into(),
                workflow,
                task: "task".into(),
                step_by_step: false,
                started_at: None,
            }
        }

        store.append(&sid, &workflow_started("r1")).unwrap();
        store
            .append(
                &sid,
                &Event::MessageDelta {
                    message_id: "m".into(),
                    content: "between".into(),
                    tokens_per_second: None,
                },
            )
            .unwrap();
        store.append(&sid, &workflow_started("r2")).unwrap();
        store
            .append(
                &sid,
                &Event::MessageDelta {
                    message_id: "m".into(),
                    content: "after".into(),
                    tokens_per_second: None,
                },
            )
            .unwrap();

        // seqs: 1 from create_session, 2 r1, 3 delta, 4 r2, 5 delta.
        let run = store.workflow_run_before(&sid, u64::MAX).unwrap();
        assert_eq!(run.iter().map(|(s, _)| *s).collect::<Vec<_>>(), vec![4]);

        let run = store.workflow_run_before(&sid, 4).unwrap();
        assert_eq!(run.iter().map(|(s, _)| *s).collect::<Vec<_>>(), vec![2]);

        assert!(store.workflow_run_before(&sid, 2).unwrap().is_empty());
    }
}
