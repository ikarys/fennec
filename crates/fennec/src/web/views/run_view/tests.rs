//! The server-side run view, tests first: the plan moves the run
//! computation out of `web/src/lib/workflowRun.ts` into the server, which
//! serves a pre-calculated view the UI renders as it is. These tests are
//! the contract; the production part of the `run_view` module is what the
//! build adds.

use super::{
    Finished, NodeRun, PausedEdge, RunCounts, RunStepState, RunView, UpcomingStep, run_view,
};
use crate::domain::event::Event;
use crate::domain::workflow::definition::NodeKind;
use crate::domain::workflow::run::RunOutcome;

/// The run the tests drive: start → a → b → end.
fn workflow() -> crate::domain::workflow::definition::Workflow {
    serde_json::from_value(serde_json::json!({
        "id": "wf",
        "name": "WF",
        "nodes": [
            { "id": "start", "kind": "start", "position": { "x": 0, "y": 0 } },
            { "id": "a", "kind": "agent", "agent": "plan", "name": "A", "position": { "x": 0, "y": 0 } },
            { "id": "b", "kind": "agent", "agent": "build", "name": "B", "position": { "x": 0, "y": 0 } },
            { "id": "end", "kind": "end", "outcome": "done", "position": { "x": 0, "y": 0 } }
        ],
        "edges": [
            { "from": "start", "port": "next", "to": "a" },
            { "from": "a", "port": "done", "to": "b" },
            { "from": "b", "port": "next", "to": "end" }
        ]
    }))
    .unwrap()
}

fn started() -> Event {
    Event::WorkflowStarted {
        run_id: "run-1".into(),
        workflow: workflow(),
        task: "build".into(),
        step_by_step: false,
        started_at: None,
    }
}

fn node_started(node_id: &str, pass: u32) -> Event {
    Event::WorkflowNodeStarted {
        run_id: "run-1".into(),
        node_id: node_id.into(),
        pass,
        via: None,
        child_session_id: None,
        started_at: None,
    }
}

fn node_finished(node_id: &str, port: &str) -> Event {
    Event::WorkflowNodeFinished {
        run_id: "run-1".into(),
        node_id: node_id.into(),
        port: port.into(),
        output: String::new(),
        exit_code: None,
        comment: None,
        duration_ms: None,
    }
}

/// One step the server must serve pre-calculated: identity, state, port.
fn step(node_id: &str, state: RunStepState, port: Option<&str>) -> NodeRun {
    NodeRun {
        node_id: node_id.into(),
        label: if node_id == "a" {
            "A".into()
        } else if node_id == "b" {
            "B".into()
        } else {
            node_id.into()
        },
        kind: NodeKind::Agent {
            agent: String::new(),
            prompt: String::new(),
            retry_prompt: None,
            outputs: vec!["done".into()],
            context: crate::domain::workflow::definition::AgentContext::Session,
            verdicts: false,
        },
        pass: 1,
        state,
        port: port.map(str::to_string),
        child_session_id: None,
        started_at: None,
        duration_ms: None,
        output: None,
    }
}

#[test]
fn no_workflow_events_no_view() {
    assert_eq!(run_view(&[]), None);
    assert_eq!(
        run_view(&[Event::MessageDelta {
            message_id: "m".into(),
            content: "x".into(),
            tokens_per_second: None,
        }]),
        None
    );
    // A node event before any run started is not a run.
    assert_eq!(run_view(&[node_started("a", 1)]), None);
}

#[test]
fn a_started_run_is_served_with_empty_precomputed_fields() {
    let view: RunView = run_view(&[started()]).expect("a started run has a view");

    assert_eq!(view.run_id, "run-1");
    assert_eq!(view.workflow, workflow());
    assert_eq!(view.task, "build");
    assert!(!view.step_by_step);
    assert!(!view.pause_requested);
    assert!(view.runs.is_empty());
    assert!(view.open.is_empty());
    assert!(view.paused.is_none());
    assert!(view.questions.is_empty());
    assert!(view.finished.is_none());
    assert!(view.last_comment.is_none());
    assert!(view.rounds.is_empty());
    assert!(view.upcoming.is_empty());
    assert_eq!(
        view.counts,
        RunCounts {
            done: 0,
            open: 0,
            total: 0
        }
    );
}

#[test]
fn node_runs_carry_their_state_port_times_and_output() {
    let events = vec![
        started(),
        Event::WorkflowNodeStarted {
            run_id: "run-1".into(),
            node_id: "a".into(),
            pass: 1,
            via: None,
            child_session_id: Some("child-1".into()),
            started_at: Some(1_000),
        },
        Event::WorkflowNodeFinished {
            run_id: "run-1".into(),
            node_id: "a".into(),
            port: "done".into(),
            output: "the report".into(),
            exit_code: None,
            comment: None,
            duration_ms: Some(50),
        },
    ];
    let view = run_view(&events).expect("view");

    assert_eq!(
        view.runs,
        vec![NodeRun {
            started_at: Some(1_000),
            duration_ms: Some(50),
            output: Some("the report".into()),
            child_session_id: Some("child-1".into()),
            ..step("a", RunStepState::Done, Some("done"))
        }]
    );
    assert!(view.open.is_empty());
}

#[test]
fn a_second_pass_is_a_second_step() {
    let events = vec![
        started(),
        node_started("a", 1),
        node_finished("a", "done"),
        node_started("a", 2),
    ];
    let view = run_view(&events).expect("view");

    assert_eq!(
        view.runs,
        vec![
            step("a", RunStepState::Done, Some("done")),
            step("a", RunStepState::Running, None)
        ]
    );
    assert_eq!(view.runs[1].pass, 2);
    assert_eq!(view.open, vec!["a".into()]);
}

#[test]
fn a_resume_interrupts_the_running_nodes_and_restarts_the_row() {
    let events = vec![
        started(),
        node_started("a", 1),
        Event::WorkflowPaused {
            run_id: "run-1".into(),
            from: "a".into(),
            port: "next".into(),
            to: "a".into(),
        },
        Event::WorkflowResumed {
            run_id: "run-1".into(),
            comment: "go".into(),
        },
        node_started("a", 1),
    ];
    let view = run_view(&events).expect("view");

    // The interrupted node's restart reuses its row: one step, running again.
    assert_eq!(view.runs, vec![step("a", RunStepState::Running, None)]);
    assert_eq!(view.open, vec!["a".into()]);
    assert!(view.paused.is_none());
    assert_eq!(view.last_comment.as_deref(), Some("go"));
}

#[test]
fn a_finish_marks_the_run_and_ends_the_open_nodes_with_the_outcome() {
    let events = vec![
        started(),
        node_started("a", 1),
        Event::WorkflowFinished {
            run_id: "run-1".into(),
            outcome: RunOutcome::Stopped,
            reason: Some("the human stopped it".into()),
            stats: None,
        },
    ];
    let view = run_view(&events).expect("view");

    assert_eq!(
        view.finished,
        Some(Finished {
            outcome: RunOutcome::Stopped,
            reason: Some("the human stopped it".into()),
        })
    );
    // The node still running is the one that ended the run: state ended, port the outcome.
    assert_eq!(
        view.runs,
        vec![step("a", RunStepState::Ended, Some("stopped"))]
    );
    assert!(view.open.is_empty());
    assert!(view.upcoming.is_empty());
    assert_eq!(
        view.counts,
        RunCounts {
            done: 1,
            open: 0,
            total: 1
        }
    );
}

#[test]
fn all_outcomes_are_served() {
    for outcome in [
        RunOutcome::Done,
        RunOutcome::Blocked,
        RunOutcome::Stopped,
        RunOutcome::Error,
    ] {
        let events = vec![
            started(),
            Event::WorkflowFinished {
                run_id: "run-1".into(),
                outcome,
                reason: None,
                stats: None,
            },
        ];
        assert_eq!(
            run_view(&events)
                .expect("view")
                .finished
                .as_ref()
                .map(|f| f.outcome),
            Some(outcome)
        );
    }
}

#[test]
fn a_pause_request_and_the_paused_edge_are_served() {
    let events = vec![
        started(),
        Event::WorkflowPauseRequested {
            run_id: "run-1".into(),
        },
        Event::WorkflowPaused {
            run_id: "run-1".into(),
            from: "a".into(),
            port: "next".into(),
            to: "b".into(),
        },
    ];
    let view = run_view(&events).expect("view");

    assert!(!view.pause_requested);
    assert_eq!(
        view.paused,
        Some(PausedEdge {
            from: "a".into(),
            port: "next".into(),
            to: "b".into(),
        })
    );
}

#[test]
fn a_human_question_is_served_by_node() {
    let events = vec![
        started(),
        node_started("b", 1),
        Event::WorkflowHumanAsked {
            run_id: "run-1".into(),
            node_id: "b".into(),
            question: "Continue?".into(),
        },
    ];
    let view = run_view(&events).expect("view");

    assert_eq!(
        view.questions.get("b").map(String::as_str),
        Some("Continue?")
    );
}

#[test]
fn rounds_split_on_a_repeated_node() {
    let events = vec![
        started(),
        node_started("a", 1),
        node_finished("a", "done"),
        node_started("b", 1),
        node_finished("b", "next"),
        node_started("a", 2),
        node_finished("a", "done"),
        node_started("b", 2),
        node_finished("b", "next"),
        node_started("a", 3),
    ];
    let view = run_view(&events).expect("view");

    // Three rounds: the repeated a opens each new one.
    assert_eq!(view.rounds.len(), 3);
    assert_eq!(
        view.rounds[0]
            .iter()
            .map(|run| run.node_id.as_str())
            .collect::<Vec<_>>(),
        vec!["a", "b"]
    );
    assert_eq!(
        view.rounds[1]
            .iter()
            .map(|run| run.node_id.as_str())
            .collect::<Vec<_>>(),
        vec!["a", "b"]
    );
    assert_eq!(
        view.rounds[2]
            .iter()
            .map(|run| run.node_id.as_str())
            .collect::<Vec<_>>(),
        vec!["a"]
    );
}

#[test]
fn upcoming_follows_the_nominal_path_from_the_current_node() {
    let events = vec![started(), node_started("a", 1)];
    let view = run_view(&events).expect("view");

    // a → b → end: only b is a step to come, the end node is not listed.
    assert_eq!(
        view.upcoming,
        vec![UpcomingStep {
            node_id: "b".into(),
            label: "B".into(),
        }]
    );
}

#[test]
fn upcoming_prefers_the_positive_port() {
    let workflow: crate::domain::workflow::definition::Workflow = serde_json::from_value(
        serde_json::json!({
            "id": "wf",
            "name": "WF",
            "nodes": [
                { "id": "a", "kind": "agent", "agent": "plan", "name": "A", "position": { "x": 0, "y": 0 } },
                { "id": "x", "kind": "agent", "agent": "plan", "name": "X", "position": { "x": 0, "y": 0 } },
                { "id": "y", "kind": "agent", "agent": "build", "name": "Y", "position": { "x": 0, "y": 0 } }
            ],
            "edges": [
                { "from": "a", "port": "fail", "to": "x" },
                { "from": "a", "port": "done", "to": "y" }
            ]
        }),
    )
    .unwrap();
    let events = vec![
        Event::WorkflowStarted {
            run_id: "run-1".into(),
            workflow,
            task: "build".into(),
            step_by_step: false,
            started_at: None,
        },
        node_started("a", 1),
    ];
    let view = run_view(&events).expect("view");

    assert_eq!(
        view.upcoming,
        vec![UpcomingStep {
            node_id: "y".into(),
            label: "Y".into(),
        }]
    );
}

#[test]
fn upcoming_does_not_relist_a_node_in_a_loop() {
    let workflow: crate::domain::workflow::definition::Workflow = serde_json::from_value(
        serde_json::json!({
            "id": "wf",
            "name": "WF",
            "nodes": [
                { "id": "a", "kind": "agent", "agent": "plan", "name": "A", "position": { "x": 0, "y": 0 } },
                { "id": "b", "kind": "agent", "agent": "build", "name": "B", "position": { "x": 0, "y": 0 } }
            ],
            "edges": [
                { "from": "a", "port": "next", "to": "b" },
                { "from": "b", "port": "next", "to": "a" }
            ]
        }),
    )
    .unwrap();
    let events = vec![
        Event::WorkflowStarted {
            run_id: "run-1".into(),
            workflow,
            task: "build".into(),
            step_by_step: false,
            started_at: None,
        },
        node_started("a", 1),
    ];
    let view = run_view(&events).expect("view");

    assert_eq!(
        view.upcoming,
        vec![UpcomingStep {
            node_id: "b".into(),
            label: "B".into(),
        }]
    );
}

#[test]
fn upcoming_lists_the_paused_target_first() {
    let events = vec![
        started(),
        Event::WorkflowPaused {
            run_id: "run-1".into(),
            from: "a".into(),
            port: "next".into(),
            to: "b".into(),
        },
    ];
    let view = run_view(&events).expect("view");

    assert_eq!(
        view.upcoming,
        vec![UpcomingStep {
            node_id: "b".into(),
            label: "B".into(),
        }]
    );
}

#[test]
fn a_node_missing_from_the_definition_falls_back_to_its_id() {
    let events = vec![started(), node_started("zz", 1)];
    let view = run_view(&events).expect("view");

    assert_eq!(view.runs[0].node_id, "zz");
    assert_eq!(view.runs[0].label, "zz");
    assert!(matches!(view.runs[0].kind, NodeKind::Agent { .. }));
}

#[test]
fn counts_are_done_open_and_total() {
    // a done, b running, b → end: done 1, open 1, total 2.
    let events = vec![
        started(),
        node_started("a", 1),
        node_finished("a", "done"),
        node_started("b", 1),
    ];
    let view = run_view(&events).expect("view");
    assert_eq!(
        view.counts,
        RunCounts {
            done: 1,
            open: 1,
            total: 2
        }
    );

    // a running before b: the upcoming step counts in the total.
    let events = vec![started(), node_started("a", 1)];
    let view = run_view(&events).expect("view");
    assert_eq!(
        view.counts,
        RunCounts {
            done: 0,
            open: 1,
            total: 2
        }
    );
}

#[test]
fn a_second_run_replaces_the_view() {
    let events = vec![
        started(),
        node_started("a", 1),
        Event::WorkflowStarted {
            run_id: "run-2".into(),
            workflow: workflow(),
            task: "second".into(),
            step_by_step: true,
            started_at: None,
        },
    ];
    let view = run_view(&events).expect("view");

    assert_eq!(view.run_id, "run-2");
    assert_eq!(view.task, "second");
    assert!(view.step_by_step);
    assert!(view.runs.is_empty());
    assert!(view.open.is_empty());
    assert!(view.finished.is_none());
    assert!(view.rounds.is_empty());
    assert!(view.upcoming.is_empty());
    assert_eq!(
        view.counts,
        RunCounts {
            done: 0,
            open: 0,
            total: 0
        }
    );
}

// The response the UI renders as it is: camelCase, pre-calculated.
mod route {
    use super::*;
    use crate::agent::ports::EventLog;
    use crate::domain::id::new_id;
    use crate::notifier::NotifyingLog;
    use crate::web::http::Request;
    use crate::web::routes::session_routes::serve_run_view;
    use crate::web::test_support::app;
    use std::io::Read;
    use std::net::{TcpListener, TcpStream};

    fn run_request<F: FnOnce(&mut std::net::TcpStream)>(request: F) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let mut client = TcpStream::connect(addr).unwrap();
        let mut server = listener.accept().unwrap().0;
        request(&mut server);
        drop(server);
        let mut raw = Vec::new();
        client.read_to_end(&mut raw).unwrap();
        String::from_utf8_lossy(&raw).into_owned()
    }

    fn get_view(app: &crate::web::App, session: &str, req: &Request) -> serde_json::Value {
        let response = run_request(|stream| serve_run_view(app, req, stream, session).unwrap());
        let body = response.split("\r\n\r\n").nth(1).unwrap_or("");
        serde_json::from_str(body).expect("the run view is JSON")
    }

    #[test]
    fn the_run_view_route_serves_the_precomputed_view() {
        let db_path = std::env::temp_dir().join(format!("fennec-run-view-{}.db", new_id()));
        let app = app(db_path.clone());
        let store = app.store().unwrap();
        let project = store
            .project_for(&format!("/tmp/run-view-{}", new_id()))
            .unwrap();
        let session = store.create_session(&project, "t").unwrap();
        let log = NotifyingLog {
            log: &store,
            notifier: &app.notifier,
        };

        let events = vec![
            started(),
            node_started("a", 1),
            node_finished("a", "done"),
            node_started("b", 1),
            node_finished("b", "next"),
            node_started("a", 2),
        ];
        for event in &events {
            log.append(&session, event).unwrap();
        }

        let req = Request {
            method: "GET".into(),
            path: "/api/sessions/{id}/workflow/run/view".into(),
            query: String::new(),
            headers: Vec::new(),
            body: Vec::new(),
        };
        let view = get_view(&app, &session, &req);

        assert_eq!(view["runId"], "run-1");
        assert_eq!(view["task"], "build");
        // Steps with their states and ports.
        assert_eq!(view["runs"].as_array().unwrap().len(), 3);
        assert_eq!(view["runs"][0]["nodeId"], "a");
        assert_eq!(view["runs"][0]["state"], "done");
        assert_eq!(view["runs"][0]["port"], "done");
        assert_eq!(view["runs"][2]["pass"], 2);
        assert_eq!(view["runs"][2]["state"], "running");
        // Rounds: the repeated a opens the second round.
        assert_eq!(view["rounds"].as_array().unwrap().len(), 2);
        assert_eq!(view["rounds"][1][0]["nodeId"], "a");
        // Upcoming: a → b.
        assert_eq!(view["upcoming"][0]["nodeId"], "b");
        assert_eq!(view["upcoming"][0]["label"], "B");
        // Completion counts.
        assert_eq!(view["counts"]["done"], 2);
        assert_eq!(view["counts"]["open"], 1);
        assert_eq!(view["counts"]["total"], 3);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn the_run_view_route_refuses_a_session_without_a_run() {
        let db_path = std::env::temp_dir().join(format!("fennec-run-view-{}.db", new_id()));
        let app = app(db_path.clone());
        let store = app.store().unwrap();
        let project = store
            .project_for(&format!("/tmp/run-view-{}", new_id()))
            .unwrap();
        let session = store.create_session(&project, "t").unwrap();

        let req = Request {
            method: "GET".into(),
            path: "/api/sessions/{id}/workflow/run/view".into(),
            query: String::new(),
            headers: Vec::new(),
            body: Vec::new(),
        };
        let response = run_request(|stream| serve_run_view(&app, &req, stream, &session).unwrap());
        assert!(
            response.starts_with("HTTP/1.1 404"),
            "no run: 404, got {response}"
        );
        let _ = std::fs::remove_file(&db_path);
    }
}

// The plan's contract on the UI side: the client-side run fold is deleted,
// the run components render the server's view as it is, and docs/api.md
// documents the server-side run view.
mod ui_contract {
    use std::path::{Path, PathBuf};

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn read(path: &str) -> String {
        std::fs::read_to_string(root().join(path)).unwrap()
    }

    fn src_files(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                src_files(&path, out);
            } else if path
                .extension()
                .is_some_and(|ext| ext == "ts" || ext == "tsx")
            {
                out.push(path);
            }
        }
    }

    /// A `from "…workflowRun"` import, single line (the project's style).
    fn imports_run_fold(content: &str) -> bool {
        content.lines().any(|line| {
            line.split("from ").nth(1).is_some_and(|rest| {
                let rest = rest.trim();
                rest.split_once('\'')
                    .or_else(|| rest.split_once('"'))
                    .is_some_and(|(module, _)| module.ends_with("workflowRun"))
            })
        })
    }

    #[test]
    fn the_client_side_run_fold_is_deleted() {
        assert!(
            !root().join("web/src/lib/workflowRun.ts").exists(),
            "web/src/lib/workflowRun.ts must be deleted"
        );
        let mut files = Vec::new();
        src_files(&root().join("web/src"), &mut files);
        let importers: Vec<String> = files
            .iter()
            .filter(|file| imports_run_fold(&std::fs::read_to_string(file).unwrap()))
            .map(|file| file.to_string_lossy().into_owned())
            .collect();
        assert!(importers.is_empty(), "importers left: {importers:?}");
    }

    #[test]
    fn the_run_components_read_no_run_logic_from_the_deleted_fold() {
        for component in [
            "web/src/components/workflow/session/RunRail.tsx",
            "web/src/components/workflow/session/RunTimeline.tsx",
            "web/src/components/workflow/session/NodeDivider.tsx",
        ] {
            let content = read(component);
            assert!(
                !imports_run_fold(&content),
                "{component} imports the deleted fold"
            );
            // The run computation the server now serves pre-calculated.
            assert!(!content.contains("upcomingSteps("), "{component}");
            assert!(!content.contains("runRounds("), "{component}");
            assert!(!content.contains("runWaiting("), "{component}");
        }
    }

    #[test]
    fn the_api_docs_describe_the_server_side_run_view() {
        let api = read("docs/api.md");
        assert!(
            api.contains("workflow/run/view"),
            "docs/api.md must name the run view route"
        );
        assert!(
            api.contains("rounds"),
            "docs/api.md must document the rounds field"
        );
        assert!(
            api.contains("upcoming"),
            "docs/api.md must document the upcoming field"
        );
        assert!(
            api.contains("counts"),
            "docs/api.md must document the counts field"
        );
    }
}
