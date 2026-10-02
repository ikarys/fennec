//! Solve and land, driven by scripted models: the verdict comes from the
//! checks, never from what the model says.

mod common;

use std::collections::VecDeque;
use std::io;
use std::process::Command;
use std::sync::Mutex;

use common::Fixture;
use fennec_os::provider::Model;
use fennec_os::solve::{Task, land, solve};
use fennec_os::{review, rules, sleep};
use serde_json::{Value, json};

struct Script {
    name: &'static str,
    replies: Mutex<VecDeque<Value>>,
}

impl Script {
    /// Each step is one tool call `(name, arguments)`.
    fn new(name: &'static str, steps: &[(&str, Value)]) -> Self {
        let replies = steps
            .iter()
            .enumerate()
            .map(|(i, (tool, args))| {
                json!({"content": null, "reasoning_content": "hmm", "tool_calls": [{
                    "id": format!("c{i}"), "type": "function",
                    "function": {"name": tool, "arguments": args.to_string()}
                }]})
            })
            .collect();
        Self {
            name,
            replies: Mutex::new(replies),
        }
    }
}

impl Model for Script {
    fn chat(&self, messages: &[Value], _tools: &Value) -> io::Result<Value> {
        // The harness must not send the model's reasoning back.
        assert!(
            messages
                .iter()
                .all(|m| m.get("reasoning_content").is_none())
        );
        self.replies
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| io::Error::other("script exhausted"))
    }
    fn label(&self) -> String {
        self.name.to_string()
    }
}

fn write(path: &str, content: &str) -> (&'static str, Value) {
    ("write_file", json!({"path": path, "content": content}))
}

fn done() -> (&'static str, Value) {
    ("done", json!({"summary": "all good, trust me"}))
}

fn git(f: &Fixture, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(&f.project)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn task(protect: &[&str]) -> Task {
    Task {
        goal: "Write 42 into the file `answer`.".to_string(),
        verify: vec!["sh check.sh".to_string()],
        protect: protect.iter().map(|p| p.to_string()).collect(),
    }
}

fn fixture(name: &str) -> Fixture {
    let f = Fixture::new(name);
    std::fs::write(f.project.join("check.sh"), "grep -qx 42 answer\n").unwrap();
    git(&f, &["add", "-A"]);
    git(
        &f,
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-qm",
            "check",
        ],
    );
    f
}

#[test]
fn the_smallest_passing_world_wins_and_lands() {
    let f = fixture("solve");
    let wrong = Script::new("wrong", &[write("answer", "41\n"), done()]);
    let noisy = Script::new(
        "noisy",
        &[write("answer", "42\n"), write("notes", "a\nb\nc\n"), done()],
    );
    let tight = Script::new("tight", &[write("answer", "42\n"), done()]);
    let models: Vec<&dyn Model> = vec![&wrong, &noisy, &tight];

    let record = solve(&f.worlds, &f.project, &task(&[]), &models, 10).unwrap();
    let attempts = record["attempts"].as_array().unwrap();
    let passed: Vec<bool> = attempts.iter().map(|a| a["passed"] == true).collect();
    assert_eq!(passed, [false, true, true], "{record:#}");
    assert_eq!(attempts[0]["reason"], "verify failed");
    let winner = record["winner"].as_str().unwrap().to_string();
    assert_eq!(winner, attempts[2]["world"]);
    // Losers are gone; only the winner waits for landing.
    assert_eq!(f.worlds.list().unwrap(), vec![winner.clone()]);

    let commit = land(&f.worlds, &winner).unwrap();
    assert_eq!(git(&f, &["rev-parse", "HEAD"]), commit);
    assert_eq!(
        std::fs::read_to_string(f.project.join("answer")).unwrap(),
        "42\n"
    );
    assert!(f.worlds.list().unwrap().is_empty());
    assert!(land(&f.worlds, &winner).is_err());
}

#[test]
fn editing_a_protected_check_fails_even_if_it_passes() {
    let f = fixture("protect");
    let cheat = Script::new("cheat", &[write("check.sh", "exit 0\n"), done()]);
    let record = solve(&f.worlds, &f.project, &task(&["check.sh"]), &[&cheat], 10).unwrap();
    assert_eq!(record["winner"], Value::Null);
    assert_eq!(
        record["attempts"][0]["reason"],
        "rule task.protect violated: changed check.sh"
    );
    assert!(f.worlds.list().unwrap().is_empty());
}

#[test]
fn a_world_touched_after_its_verdict_cannot_land() {
    let f = fixture("tamper");
    let tight = Script::new("tight", &[write("answer", "42\n"), done()]);
    let record = solve(&f.worlds, &f.project, &task(&[]), &[&tight], 10).unwrap();
    let id = record["winner"].as_str().unwrap();
    let world = f.worlds.open(id).unwrap();
    assert!(f.sh(&world, "echo sneaky > extra").0);
    assert!(land(&f.worlds, id).is_err());
    assert!(!f.project.join("answer").exists());
}

#[test]
fn a_model_that_stops_answering_is_a_failed_attempt() {
    let f = fixture("exhausted");
    let mute = Script::new("mute", &[write("answer", "42\n")]);
    let record = solve(&f.worlds, &f.project, &task(&[]), &[&mute], 10).unwrap();
    assert_eq!(record["attempts"][0]["reason"], "script exhausted");
    assert_eq!(record["winner"], Value::Null);
}

#[test]
fn a_world_rebases_and_reverifies_when_the_project_moved() {
    let f = fixture("rebase");
    let tight = Script::new("tight", &[write("answer", "42\n"), done()]);
    let record = solve(&f.worlds, &f.project, &task(&[]), &[&tight], 10).unwrap();
    let id = record["winner"].as_str().unwrap();

    std::fs::write(f.project.join("other"), "meanwhile\n").unwrap();
    git(&f, &["add", "-A"]);
    git(
        &f,
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-qm",
            "other",
        ],
    );
    let moved = git(&f, &["rev-parse", "HEAD"]);

    let commit = land(&f.worlds, id).unwrap();
    assert_ne!(commit, record["attempts"][0]["commit"]);
    assert_eq!(git(&f, &["rev-parse", "HEAD~1"]), moved);
    assert_eq!(
        std::fs::read_to_string(f.project.join("answer")).unwrap(),
        "42\n"
    );
}

#[test]
fn a_rebase_that_breaks_the_checks_does_not_land() {
    let f = fixture("rebreak");
    let tight = Script::new("tight", &[write("answer", "42\n"), done()]);
    let record = solve(&f.worlds, &f.project, &task(&[]), &[&tight], 10).unwrap();
    let id = record["winner"].as_str().unwrap();

    // Meanwhile the project tightens its check: the old verdict means nothing.
    std::fs::write(f.project.join("check.sh"), "grep -qx 43 answer\n").unwrap();
    git(
        &f,
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-qam",
            "stricter",
        ],
    );
    let head = git(&f, &["rev-parse", "HEAD"]);

    let err = land(&f.worlds, id).unwrap_err().to_string();
    assert!(err.contains("verify failed after rebase"), "{err}");
    assert_eq!(git(&f, &["rev-parse", "HEAD"]), head);
    assert!(land(&f.worlds, id).is_err());
}

fn commit_rules(f: &Fixture, rules: Value) {
    std::fs::create_dir_all(f.project.join(".fennec-os")).unwrap();
    std::fs::write(f.project.join(".fennec-os/rules.json"), rules.to_string()).unwrap();
    git(f, &["add", "-A"]);
    git(
        f,
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-qm",
            "rules",
        ],
    );
}

#[test]
fn project_rules_judge_every_attempt() {
    let f = fixture("rules");
    commit_rules(
        &f,
        json!([
            {"id": "r1", "kind": "paired_change", "when": "answer", "then": "notes"},
            {"id": "r2", "kind": "standing_command", "command": "false", "severity": "warn"},
        ]),
    );
    let bare = Script::new("bare", &[write("answer", "42\n"), done()]);
    let paired = Script::new(
        "paired",
        &[write("answer", "42\n"), write("notes", "why\n"), done()],
    );
    let cheat = Script::new(
        "cheat",
        &[
            write(".fennec-os/rules.json", "[]"),
            write("answer", "42\n"),
            done(),
        ],
    );
    let record = solve(
        &f.worlds,
        &f.project,
        &task(&[]),
        &[&bare, &paired, &cheat],
        10,
    )
    .unwrap();
    let a = record["attempts"].as_array().unwrap();
    assert_eq!(
        a[0]["reason"],
        "rule r1 violated: changed answer but nothing under notes"
    );
    assert_eq!(a[1]["passed"], true, "{:#}", a[1]);
    // A failing `warn` rule is evidence, not a verdict.
    assert!(
        a[1]["checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["rule"] == "r2" && c["exit"] == 1)
    );
    assert_eq!(
        a[2]["reason"],
        "rule harness-dir violated: changed .fennec-os/rules.json"
    );
    assert_eq!(record["winner"], a[1]["world"]);
}

#[test]
fn a_correction_said_twice_is_counted_and_promoted() {
    let f = fixture("review");
    let mut ids = Vec::new();
    for _ in 0..2 {
        let tight = Script::new("tight", &[write("answer", "42\n"), done()]);
        let record = solve(&f.worlds, &f.project, &task(&[]), &[&tight], 10).unwrap();
        ids.push(record["winner"].as_str().unwrap().to_string());
    }
    assert_eq!(review::queue(&f.worlds).unwrap().len(), 2);
    assert_eq!(
        review::reject(&f.worlds, &ids[0], Some("Always run clippy!")).unwrap(),
        1
    );
    assert_eq!(
        review::reject(&f.worlds, &ids[1], Some("always run  clippy")).unwrap(),
        2
    );
    assert!(review::queue(&f.worlds).unwrap().is_empty());
    assert!(f.worlds.list().unwrap().is_empty());

    let rule = json!({"kind": "standing_command", "command": "cargo clippy"});
    assert_eq!(rules::promote(&f.project, &rule, &ids[1]).unwrap(), "r1");
    let written: Value = serde_json::from_str(
        &std::fs::read_to_string(f.project.join(".fennec-os/rules.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(written[0]["origin"], ids[1].as_str());
    assert!(rules::promote(&f.project, &json!({"kind": "vibes"}), "x").is_err());
}

/// Answers `done` at once and keeps the system prompt it was given.
struct Spy(Mutex<String>);

impl Model for Spy {
    fn chat(&self, messages: &[Value], _tools: &Value) -> io::Result<Value> {
        *self.0.lock().unwrap() = messages[0]["content"].as_str().unwrap().to_string();
        let (tool, args) = done();
        Ok(
            json!({"tool_calls": [{"id": "c", "function": {"name": tool, "arguments": args.to_string()}}]}),
        )
    }
    fn label(&self) -> String {
        "spy".to_string()
    }
}

#[test]
fn a_new_run_starts_from_the_project_history() {
    let f = fixture("agenda");
    let tight = Script::new("tight", &[write("answer", "42\n"), done()]);
    let first = solve(&f.worlds, &f.project, &task(&[]), &[&tight], 10).unwrap();
    land(&f.worlds, first["winner"].as_str().unwrap()).unwrap();
    let again = Script::new(
        "again",
        &[write("answer", "42\n"), write("x", "1\n"), done()],
    );
    let second = solve(&f.worlds, &f.project, &task(&[]), &[&again], 10).unwrap();
    review::reject(
        &f.worlds,
        second["winner"].as_str().unwrap(),
        Some("no stray files"),
    )
    .unwrap();

    let spy = Spy(Mutex::new(String::new()));
    solve(&f.worlds, &f.project, &task(&["tests"]), &[&spy], 10).unwrap();
    let system = spy.0.lock().unwrap().clone();
    assert!(
        system.contains("landed \"Write 42 into the file `answer`.\": all good, trust me"),
        "{system}"
    );
    assert!(system.contains("correction: no stray files"), "{system}");
    assert!(system.contains("do not change tests"), "{system}");
}

/// Answers every distillation with the same facts, reasoning inline.
struct Distiller;

impl Model for Distiller {
    fn chat(&self, messages: &[Value], _tools: &Value) -> io::Result<Value> {
        let trajectory = messages[1]["content"].as_str().unwrap();
        assert!(trajectory.contains("write answer"), "{trajectory}");
        Ok(
            json!({"content": "<think>- not this</think>\n- checks run with `sh check.sh`\nnoise\n- answers end with a newline\n- third\n- fourth"}),
        )
    }
    fn label(&self) -> String {
        "distiller".to_string()
    }
}

#[test]
fn sleep_proposes_know_how_that_a_human_keeps() {
    let f = fixture("sleep");
    let tight = Script::new("tight", &[write("answer", "42\n"), done()]);
    let run = solve(&f.worlds, &f.project, &task(&[]), &[&tight], 10).unwrap();
    land(&f.worlds, run["winner"].as_str().unwrap()).unwrap();

    let new = sleep::sleep(&f.worlds, &Distiller).unwrap();
    let texts: Vec<&str> = new.iter().map(|c| c["text"].as_str().unwrap()).collect();
    assert_eq!(
        texts,
        [
            "checks run with `sh check.sh`",
            "answers end with a newline",
            "third"
        ]
    );
    // A run is distilled once.
    assert!(sleep::sleep(&f.worlds, &Distiller).unwrap().is_empty());

    sleep::keep(&f.worlds, "k1").unwrap();
    sleep::dismiss(&f.worlds, "k2").unwrap();
    assert!(sleep::keep(&f.worlds, "k2").is_err());
    git(&f, &["add", "-A"]);
    git(
        &f,
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-qm",
            "know-how",
        ],
    );
    assert_eq!(
        sleep::knowhow(&f.project).unwrap(),
        ["checks run with `sh check.sh`"]
    );

    let spy = Spy(Mutex::new(String::new()));
    solve(&f.worlds, &f.project, &task(&[]), &[&spy], 10).unwrap();
    let system = spy.0.lock().unwrap().clone();
    assert!(
        system.contains("Know-how kept for this project:\n  - checks run with `sh check.sh`"),
        "{system}"
    );
}
