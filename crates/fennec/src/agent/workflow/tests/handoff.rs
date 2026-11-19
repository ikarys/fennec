//! Handoff agent nodes: a new context window in the session.

use super::*;

fn handoff_workflow(with_session_before: bool) -> Workflow {
    let mut nodes = vec![
        Node {
            id: "start".into(),
            name: None,
            position: Position::default(),
            kind: NodeKind::Start,
        },
        Node {
            id: "end".into(),
            name: None,
            position: Position::default(),
            kind: NodeKind::End {
                outcome: EndOutcome::Done,
            },
        },
    ];
    let mut edges = vec![Edge {
        from: "start".into(),
        port: "next".into(),
        to: if with_session_before { "a" } else { "b" }.into(),
        max_passes: None,
        pause: false,
    }];
    if with_session_before {
        nodes.push(Node {
            id: "a".into(),
            name: Some("Agent A".into()),
            position: Position::default(),
            kind: NodeKind::Agent {
                agent: "plan".into(),
                prompt: "{{task}}".into(),
                retry_prompt: None,
                outputs: vec!["done".into()],
                context: AgentContext::Session,
                verdicts: false,
            },
        });
        edges.push(Edge {
            from: "a".into(),
            port: "done".into(),
            to: "b".into(),
            max_passes: None,
            pause: false,
        });
    }
    nodes.push(Node {
        id: "b".into(),
        name: Some("Handoff B".into()),
        position: Position::default(),
        kind: NodeKind::Agent {
            agent: "plan".into(),
            prompt: "step b".into(),
            retry_prompt: None,
            outputs: vec!["done".into()],
            context: AgentContext::Handoff,
            verdicts: false,
        },
    });
    edges.push(Edge {
        from: "b".into(),
        port: "done".into(),
        to: "end".into(),
        max_passes: None,
        pause: false,
    });
    Workflow {
        id: "w".into(),
        name: "w".into(),
        description: String::new(),
        max_steps: 50,
        nodes,
        edges,
    }
}

fn view_contains(view: &[&Event], needle: &str) -> bool {
    view.iter().any(|e| match e {
        Event::MessageStart { content, .. } => {
            content.as_deref().is_some_and(|c| c.contains(needle))
        }
        Event::MessageDelta { content, .. } => content.contains(needle),
        _ => false,
    })
}

#[test]
fn a_handoff_node_opens_a_new_window() {
    let model = Scripted(RefCell::new(vec![
        reply("a done", vec![]),
        reply("b done", vec![]),
    ]));
    let stop = AtomicBool::new(false);
    let children = FakeChildren::default();
    let commands = FakeCommands::default();
    let env = WorkflowEnv {
        children: &children,
        commands: &commands,
        changes: &CHANGES,
    };
    let mut h = Harness::new(Box::new(model), PathBuf::from("/tmp"), &stop);
    h.agents = vec![plan()];

    let reason = run_workflow(
        &mut h.turn(),
        &env,
        WorkflowJob::Start {
            workflow: handoff_workflow(true),
            task: "do it".into(),
            step_by_step: false,
            attachments: Vec::new(),
        },
    )
    .unwrap();
    assert_eq!(reason, DoneReason::Complete);

    let events = h.log.0.into_inner();
    let compacted = events
        .iter()
        .filter(|e| matches!(e, Event::ContextCompacted { .. }))
        .count();
    assert_eq!(compacted, 1);

    let cut = events
        .iter()
        .rposition(|e| matches!(e, Event::ContextCompacted { .. }))
        .unwrap();
    let started_b = events
        .iter()
        .position(|e| matches!(e, Event::WorkflowNodeStarted { node_id, .. } if node_id == "b"))
        .unwrap();
    assert!(cut > started_b);
    let step_b = events
        .iter()
        .position(|e| {
            matches!(
                e,
                Event::MessageStart {
                    content: Some(content),
                    ..
                } if content.contains("step b")
            )
        })
        .unwrap();
    assert!(cut < step_b);

    let view = crate::domain::compaction::model_view(&events);
    assert!(!view_contains(&view, "a done"));
    assert!(!view_contains(&view, "do it"));
}

#[test]
fn a_handoff_node_first_in_the_run_does_not_cut() {
    let model = Scripted(RefCell::new(vec![reply("b done", vec![])]));
    let stop = AtomicBool::new(false);
    let children = FakeChildren::default();
    let commands = FakeCommands::default();
    let env = WorkflowEnv {
        children: &children,
        commands: &commands,
        changes: &CHANGES,
    };
    let mut h = Harness::new(Box::new(model), PathBuf::from("/tmp"), &stop);
    h.agents = vec![plan()];

    let reason = run_workflow(
        &mut h.turn(),
        &env,
        WorkflowJob::Start {
            workflow: handoff_workflow(false),
            task: "do it".into(),
            step_by_step: false,
            attachments: Vec::new(),
        },
    )
    .unwrap();
    assert_eq!(reason, DoneReason::Complete);

    let events = h.log.0.into_inner();
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::ContextCompacted { .. }))
    );
}

#[test]
fn a_resumed_handoff_node_keeps_its_window() {
    // The run ended in error inside the handoff node, after its window was opened:
    // resuming it goes on in that window instead of cutting a second time.
    let workflow = handoff_workflow(false);
    let model = Scripted(RefCell::new(vec![reply("b done", vec![])]));
    let stop = AtomicBool::new(false);
    let children = FakeChildren::default();
    let commands = FakeCommands::default();
    let env = WorkflowEnv {
        children: &children,
        commands: &commands,
        changes: &CHANGES,
    };
    let mut h = Harness::new(Box::new(model), PathBuf::from("/tmp"), &stop);
    h.agents = vec![plan()];

    h.log.0.borrow_mut().extend([
        Event::WorkflowStarted {
            run_id: "r".into(),
            workflow,
            task: "do it".into(),
            step_by_step: false,
            started_at: None,
        },
        Event::WorkflowNodeStarted {
            run_id: "r".into(),
            node_id: "b".into(),
            pass: 1,
            via: None,
            child_session_id: None,
            started_at: None,
        },
        Event::ContextCompacted {
            before_tokens: 0,
            after_tokens: 0,
            summary: "New window".into(),
        },
        Event::MessageStart {
            message_id: "m1".into(),
            role: crate::domain::event::Role::User,
            content: Some("step b".into()),
            is_system_generated: true,
            metadata: None,
            attachments: Vec::new(),
        },
        Event::WorkflowFinished {
            run_id: "r".into(),
            outcome: RunOutcome::Error,
            reason: Some("boom".into()),
            stats: None,
        },
    ]);

    let reason = run_workflow(&mut h.turn(), &env, WorkflowJob::Resume).unwrap();
    assert_eq!(reason, DoneReason::Complete);

    let events = h.log.0.into_inner();
    let compacted = events
        .iter()
        .filter(|e| matches!(e, Event::ContextCompacted { .. }))
        .count();
    assert_eq!(compacted, 1);
}
