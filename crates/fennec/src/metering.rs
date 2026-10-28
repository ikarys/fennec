//! Adapter between fennec's event log and the `usage` ledger: every model call
//! the log records is also written to the ledger, which outlives the sessions.
use crate::agent::ports::EventLog;
use crate::domain::clock::now_ms;
use crate::domain::event::Event;
use crate::domain::stats::MessageStats;
use crate::store::SqliteStore;
use crate::store::calls::SessionOwner;
use std::collections::BTreeMap;
use token_usage::{Call, Usage};

pub fn call_for(
    owner: &SessionOwner,
    session_id: &str,
    seq: u64,
    message_id: &str,
    at_ms: i64,
    stats: &MessageStats,
) -> token_usage::Call {
    Call {
        external_id: Some(format!("{session_id}:{seq}")),
        at_ms,
        product: "fennec".into(),
        group_id: owner.project_id.clone(),
        group_name: owner.project_name.clone(),
        run_id: session_id.to_string(),
        kind: if owner.parent_id.is_some() {
            "child"
        } else {
            "turn"
        }
        .to_string(),
        model: stats.model.clone(),
        usage: Usage {
            prompt_tokens: stats.prefill_tokens,
            cached_tokens: stats.cached_prompt_tokens,
            completion_tokens: stats.generation_tokens,
        },
        total_secs: stats.total_time,
        prefill_speed: stats.prefill_speed,
        generation_speed: stats.generation_speed,
        breakdown: BTreeMap::new(),
        savings: BTreeMap::new(),
        // Lets a view tie a ledger call back to the message that made it.
        labels: [("messageId".to_string(), message_id.to_string())].into(),
    }
}

pub struct MeteredLog<'a> {
    pub log: &'a dyn EventLog,
    pub store: &'a SqliteStore,
    pub ledger: &'a token_usage::Ledger,
}

impl EventLog for MeteredLog<'_> {
    fn append(&self, session_id: &str, event: &Event) -> Result<u64, String> {
        let seq = self.log.append(session_id, event)?;
        if let Event::MessageDone {
            message_id,
            stats: Some(stats),
            partial: false,
        } = event
        {
            // The event log is the source of truth; ledger writes are best-effort.
            match self.store.session_owner(session_id) {
                Ok(Some(owner)) => {
                    let call =
                        call_for(&owner, session_id, seq, message_id, now_ms() as i64, stats);
                    if let Err(e) = self.ledger.record(&call) {
                        eprintln!("fennec: usage ledger: {e}");
                    }
                }
                Ok(None) => {}
                Err(e) => eprintln!("fennec: usage ledger: {e}"),
            }
        }
        Ok(seq)
    }

    fn events(&self, session_id: &str) -> Result<Vec<Event>, String> {
        self.log.events(session_id)
    }
}

/// Idempotent thanks to `external_id` (rows stored before labels existed get them); scans the whole log. Ponytail: done at
/// each server start, fine until logs get huge (then keep a high-water mark).
pub fn backfill(store: &SqliteStore, ledger: &token_usage::Ledger) -> Result<usize, String> {
    let calls = store.recorded_calls().map_err(|e| e.to_string())?;
    ledger
        .import(calls.iter().map(|c| {
            call_for(
                &c.owner,
                &c.session_id,
                c.seq,
                &c.message_id,
                c.at_ms,
                &c.stats,
            )
        }))
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner(parent: Option<&str>) -> SessionOwner {
        SessionOwner {
            project_id: "project".into(),
            project_name: "Project".into(),
            parent_id: parent.map(str::to_string),
        }
    }

    fn stats() -> MessageStats {
        MessageStats {
            model: "test-model".into(),
            total_time: 2.0,
            prefill_tokens: 100,
            cached_prompt_tokens: 20,
            generation_tokens: 30,
            prefill_speed: 50.0,
            generation_speed: 15.0,
        }
    }

    #[test]
    fn call_for_maps_fields() {
        let call = call_for(&owner(None), "session", 7, "m7", 123, &stats());
        assert_eq!(call.external_id.as_deref(), Some("session:7"));
        assert_eq!(call.at_ms, 123);
        assert_eq!(call.product, "fennec");
        assert_eq!(call.group_id, "project");
        assert_eq!(call.group_name, "Project");
        assert_eq!(call.run_id, "session");
        assert_eq!(call.kind, "turn");
        assert_eq!(call.model, "test-model");
        assert_eq!(call.usage.prompt_tokens, 100);
        assert_eq!(call.usage.cached_tokens, 20);
        assert_eq!(call.usage.completion_tokens, 30);
        assert_eq!(call.total_secs, 2.0);
        assert_eq!(call.prefill_speed, 50.0);
        assert_eq!(call.generation_speed, 15.0);
        assert!(call.breakdown.is_empty());
        assert!(call.savings.is_empty());
        assert_eq!(call.labels["messageId"], "m7");
    }

    #[test]
    fn call_for_uses_child_kind_for_child_sessions() {
        let call = call_for(&owner(Some("parent")), "session", 1, "m1", 0, &stats());
        assert_eq!(call.kind, "child");
    }

    #[test]
    fn metered_log_records_only_complete_stats() {
        let store = SqliteStore::in_memory().expect("store");
        let project = store.project_for("/tmp/meter").expect("project");
        let session = store.create_session(&project, "t").expect("session");
        let ledger = token_usage::Ledger::open_in_memory().expect("ledger");
        let metered = MeteredLog {
            log: &store,
            store: &store,
            ledger: &ledger,
        };

        let seq = metered
            .append(
                &session,
                &Event::MessageDone {
                    message_id: "complete".into(),
                    stats: Some(stats()),
                    partial: false,
                },
            )
            .expect("append");
        // Event 1 is the session's creation.
        assert_eq!(seq, 2);

        metered
            .append(
                &session,
                &Event::MessageDone {
                    message_id: "partial".into(),
                    stats: Some(stats()),
                    partial: true,
                },
            )
            .expect("append");
        metered
            .append(
                &session,
                &Event::MessageDone {
                    message_id: "no-stats".into(),
                    stats: None,
                    partial: false,
                },
            )
            .expect("append");

        let summary = ledger
            .summary(&token_usage::Filter::default())
            .expect("summary");
        assert_eq!(summary.calls, 1);
        assert_eq!(summary.prompt_tokens, 100);
        assert_eq!(summary.cached_tokens, 20);
        assert_eq!(summary.completion_tokens, 30);
    }

    #[test]
    fn backfill_imports_plain_store_events_once() {
        let store = SqliteStore::in_memory().expect("store");
        let project = store.project_for("/tmp/backfill").expect("project");
        let session = store.create_session(&project, "t").expect("session");

        store
            .append(
                &session,
                &Event::MessageDone {
                    message_id: "complete".into(),
                    stats: Some(stats()),
                    partial: false,
                },
            )
            .expect("append");
        store
            .append(
                &session,
                &Event::MessageDone {
                    message_id: "partial".into(),
                    stats: Some(stats()),
                    partial: true,
                },
            )
            .expect("append");
        store
            .append(
                &session,
                &Event::MessageDone {
                    message_id: "no-stats".into(),
                    stats: None,
                    partial: false,
                },
            )
            .expect("append");

        let ledger = token_usage::Ledger::open_in_memory().expect("ledger");
        let imported = backfill(&store, &ledger).expect("backfill");
        assert_eq!(imported, 1);

        let summary = ledger
            .summary(&token_usage::Filter::default())
            .expect("summary");
        assert_eq!(summary.calls, 1);
        assert_eq!(summary.prompt_tokens, 100);
        assert_eq!(summary.cached_tokens, 20);
        assert_eq!(summary.completion_tokens, 30);

        let second = backfill(&store, &ledger).expect("backfill");
        assert_eq!(second, 0);
        let summary = ledger
            .summary(&token_usage::Filter::default())
            .expect("summary");
        assert_eq!(summary.calls, 1);
    }

    #[test]
    fn backfill_does_not_double_count_metered_events() {
        let store = SqliteStore::in_memory().expect("store");
        let project = store.project_for("/tmp/meter-backfill").expect("project");
        let session = store.create_session(&project, "t").expect("session");
        let ledger = token_usage::Ledger::open_in_memory().expect("ledger");
        let metered = MeteredLog {
            log: &store,
            store: &store,
            ledger: &ledger,
        };

        metered
            .append(
                &session,
                &Event::MessageDone {
                    message_id: "complete".into(),
                    stats: Some(stats()),
                    partial: false,
                },
            )
            .expect("append");

        let imported = backfill(&store, &ledger).expect("backfill");
        assert_eq!(imported, 0);
        let summary = ledger
            .summary(&token_usage::Filter::default())
            .expect("summary");
        assert_eq!(summary.calls, 1);
    }
}
