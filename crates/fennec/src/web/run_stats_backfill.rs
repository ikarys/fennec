//! Old runs' timelines, rebuilt when served: a `workflow.finished` recorded
//! before node spans existed gets them from its run's events. The log stays as written.

use crate::domain::event::Event;
use crate::domain::workflow::stats::fill_missing_spans;
use crate::store::SqliteStore;

pub fn backfilled(store: &SqliteStore, session_id: &str, seq: u64, payload: String) -> String {
    if !payload.contains("\"workflow.finished\"") || payload.contains("\"spans\"") {
        return payload;
    }

    let mut event: Event = match serde_json::from_str(&payload) {
        Ok(event) => event,
        Err(_) => return payload,
    };

    let Event::WorkflowFinished {
        run_id,
        stats: Some(stats),
        ..
    } = &mut event
    else {
        return payload;
    };

    if stats.nodes.is_empty() {
        return payload;
    }

    let entries = match store.events_after(session_id, 0) {
        Ok(entries) => entries,
        Err(_) => return payload,
    };

    let events: Vec<Event> = entries
        .into_iter()
        .filter(|(entry_seq, _)| *entry_seq <= seq)
        .filter_map(|(_, payload)| serde_json::from_str::<Event>(&payload).ok())
        .collect();

    let start_index = events.iter().rposition(
        |event| matches!(event, Event::WorkflowStarted { run_id: r, .. } if r == run_id),
    );

    let Some(start_index) = start_index else {
        return payload;
    };

    fill_missing_spans(stats, &events[start_index..]);

    serde_json::to_string(&event).unwrap_or(payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::ports::EventLog;
    use crate::domain::workflow::definition::Workflow;
    use crate::domain::workflow::run::RunOutcome;
    use crate::domain::workflow::stats::NodeSpan;
    use crate::domain::workflow::stats::{NodeStats, RunStats};

    #[test]
    fn backfilled_rebuilds_the_timeline_of_an_old_finished_run() {
        let store = SqliteStore::in_memory().unwrap();
        let project = store.project_for("/tmp/fennec").unwrap();
        let session_id = store.create_session(&project, "backfill").unwrap();

        store
            .append(
                &session_id,
                &Event::WorkflowStarted {
                    run_id: "r".into(),
                    workflow: Workflow {
                        id: "w".into(),
                        name: "w".into(),
                        description: String::new(),
                        max_steps: 2,
                        nodes: vec![],
                        edges: vec![],
                    },
                    task: String::new(),
                    step_by_step: false,
                    started_at: Some(1_000),
                },
            )
            .unwrap();
        store
            .append(
                &session_id,
                &Event::WorkflowNodeStarted {
                    run_id: "r".into(),
                    node_id: "a".into(),
                    pass: 1,
                    via: None,
                    child_session_id: None,
                    started_at: Some(1_100),
                },
            )
            .unwrap();
        store
            .append(
                &session_id,
                &Event::WorkflowNodeFinished {
                    run_id: "r".into(),
                    node_id: "a".into(),
                    port: "done".into(),
                    output: String::new(),
                    exit_code: None,
                    comment: None,
                    duration_ms: Some(200),
                },
            )
            .unwrap();

        // Stats as recorded before node spans existed.
        let finished = Event::WorkflowFinished {
            run_id: "r".into(),
            outcome: RunOutcome::Done,
            reason: None,
            stats: Some(RunStats {
                working_ms: 200,
                nodes: vec![NodeStats {
                    node_id: "a".into(),
                    label: "a".into(),
                    runs: 1,
                    working_ms: 200,
                    spans: vec![],
                }],
                ..RunStats::default()
            }),
        };
        store.append(&session_id, &finished).unwrap();

        let entries = store.events_after(&session_id, 0).unwrap();
        let (seq, payload) = entries
            .iter()
            .find(|(_, payload)| payload.contains("\"workflow.finished\""))
            .unwrap()
            .clone();

        let result = backfilled(&store, &session_id, seq, payload);
        let parsed: Event = serde_json::from_str(&result).unwrap();

        let Event::WorkflowFinished {
            stats: Some(stats), ..
        } = parsed
        else {
            panic!("expected a workflow.finished event");
        };

        let node = stats.nodes.iter().find(|node| node.node_id == "a").unwrap();
        assert_eq!(
            node.spans,
            vec![NodeSpan {
                offset_ms: 100,
                duration_ms: 200
            }]
        );

        let non_finished = entries
            .iter()
            .find(|(_, payload)| payload.contains("\"workflow.node.started\""))
            .unwrap()
            .1
            .clone();
        assert_eq!(
            backfilled(&store, &session_id, seq, non_finished.clone()),
            non_finished
        );
    }
}
