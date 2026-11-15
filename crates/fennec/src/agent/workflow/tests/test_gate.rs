//! The tests gate fennec answers itself when every check proves the tests fail first.

use super::*;

// fennec answers the tests gate itself only when every check runs a test that
// fails before the work; otherwise it asks, and says why.
fn run_to_tests_gate(
    plan_text: &str,
    commands: FakeCommands,
    always_validate_tests: bool,
) -> Vec<Event> {
    let workflow: Workflow = serde_json::from_value(serde_json::json!({
        "id": "wf",
        "name": "wf",
        "description": "",
        "maxSteps": 10,
        "nodes": [
            { "id": "start", "kind": "start", "position": { "x": 0, "y": 0 } },
            { "id": "plan", "kind": "agent", "name": "Plan", "agent": "plan",
              "prompt": "plan", "position": { "x": 0, "y": 100 } },
            { "id": "approve", "kind": "human", "name": "Approve", "question": "go?",
              "choices": ["Go", "Revise"], "position": { "x": 0, "y": 150 } },
            { "id": "tests", "kind": "agent", "name": "Tests", "agent": "tests",
              "prompt": "write tests", "position": { "x": 0, "y": 200 } },
            { "id": "approve-tests", "kind": "human", "name": "Validate",
              "question": "validate?", "choices": ["Validate", "Rework"],
              "position": { "x": 0, "y": 300 } },
            { "id": "criteria", "kind": "criteria", "name": "Criteria", "plan": "plan",
              "position": { "x": 0, "y": 400 } },
            { "id": "end", "kind": "end", "outcome": "done", "position": { "x": 0, "y": 500 } }
        ],
        "edges": [
            { "from": "start", "port": "next", "to": "plan" },
            { "from": "plan", "port": "done", "to": "approve" },
            { "from": "approve", "port": "Go", "to": "tests" },
            { "from": "approve", "port": "Revise", "to": "plan" },
            { "from": "tests", "port": "done", "to": "approve-tests" },
            { "from": "approve-tests", "port": "Validate", "to": "criteria" },
            { "from": "criteria", "port": "pass", "to": "end" },
            { "from": "criteria", "port": "fail", "to": "end" },
            { "from": "approve-tests", "port": "Rework", "to": "tests" }
        ]
    }))
    .unwrap();
    let model = Scripted(RefCell::new(vec![reply("## Tests", vec![])]));
    let stop = AtomicBool::new(false);
    let children = FakeChildren::default();
    let env = WorkflowEnv {
        children: &children,
        commands: &commands,
    };
    let mut h = Harness::new(Box::new(model), PathBuf::from("/tmp"), &stop);
    let mut tests = plan();
    tests.id = "tests".into();
    h.agents = vec![plan(), tests];
    let mut turn = h.turn();
    turn.always_validate_tests = always_validate_tests;
    run_workflow(
        &mut turn,
        &env,
        WorkflowJob::StartFromPlan {
            workflow,
            task: "do it".into(),
            plan: plan_text.into(),
            attachments: Vec::new(),
        },
    )
    .unwrap();
    h.log.0.into_inner()
}

fn failing_checks() -> FakeCommands {
    FakeCommands {
        exit_codes: Mutex::new(vec![1]),
        output: Some("running 1 test\ntest a ... FAILED"),
    }
}

fn asked(events: &[Event]) -> Option<&str> {
    events.iter().find_map(|event| match event {
        Event::WorkflowHumanAsked { question, .. } => Some(question.as_str()),
        _ => None,
    })
}

#[test]
fn tests_gate_is_answered_when_every_check_fails_before_the_work() {
    let events = run_to_tests_gate(
        "## Done when\n- C1 — check: `cargo test a`\n- C2 — check: `cargo test b`",
        failing_checks(),
        false,
    );
    assert_eq!(asked(&events), None, "{events:?}");
    assert!(events.iter().any(|event| matches!(
        event,
        Event::WorkflowNodeFinished { node_id, port, output, .. }
            if node_id == "approve-tests" && port == "Validate"
                && output.contains("Validated by fennec") && output.contains("C2 `cargo test b`")
    )));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::TestsApproved { .. }))
    );
}

#[test]
fn tests_gate_asks_when_a_criterion_has_no_check() {
    let events = run_to_tests_gate(
        "## Done when\n- C1 — check: `cargo test a`\n- C2",
        failing_checks(),
        false,
    );
    assert!(asked(&events).is_some_and(|q| q.contains("fennec stopped here: C2 has no check")));
}

#[test]
fn tests_gate_asks_when_a_check_passes_before_the_work() {
    let commands = FakeCommands {
        exit_codes: Mutex::new(vec![0]),
        output: Some("running 1 test\ntest a ... ok"),
    };
    let events = run_to_tests_gate(
        "## Done when\n- C1 — check: `cargo test a`",
        commands,
        false,
    );
    assert!(asked(&events).is_some_and(|q| q.contains("C1's check passes before the work")));
}

#[test]
fn tests_gate_asks_when_a_check_runs_no_test() {
    let commands = FakeCommands {
        exit_codes: Mutex::new(vec![0]),
        output: Some("running 0 tests\n\ntest result: ok. 0 passed; 0 failed"),
    };
    let events = run_to_tests_gate(
        "## Done when\n- C1 — check: `cargo test a`",
        commands,
        false,
    );
    assert!(asked(&events).is_some_and(|q| q.contains("C1's check runs no test")));
}

#[test]
fn tests_gate_always_asks_when_the_human_wants_to_read_the_tests() {
    let events = run_to_tests_gate(
        "## Done when\n- C1 — check: `cargo test a`",
        failing_checks(),
        true,
    );
    assert_eq!(asked(&events), Some("validate?"));
}
