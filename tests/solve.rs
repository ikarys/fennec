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
use fennec_os::{review, rules, sleep, spec};
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
        base: None,
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

    // A new goal: the landed one would be replayed without the model.
    let mut next = task(&[]);
    next.goal = "Write 43 into the file `answer`.".to_string();
    let spy = Spy(Mutex::new(String::new()));
    solve(&f.worlds, &f.project, &next, &[&spy], 10).unwrap();
    let system = spy.0.lock().unwrap().clone();
    assert!(
        system.contains("Know-how kept for this project:\n  - checks run with `sh check.sh`"),
        "{system}"
    );
}

fn commit_file(f: &Fixture, path: &str, content: &str) {
    std::fs::write(f.project.join(path), content).unwrap();
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
            path,
        ],
    );
}

#[test]
fn a_landed_goal_comes_back_as_a_replay_without_the_model() {
    let f = fixture("recipe");
    let first = Script::new("first", &[write("answer", "42\n"), done()]);
    let record = solve(&f.worlds, &f.project, &task(&[]), &[&first], 10).unwrap();
    land(&f.worlds, record["winner"].as_str().unwrap()).unwrap();
    commit_file(&f, "answer", "0\n");

    // Same goal again: the recipe passes, the model is never called.
    let mute = Script::new("mute", &[]);
    let record = solve(&f.worlds, &f.project, &task(&[]), &[&mute], 10).unwrap();
    let attempts = record["attempts"].as_array().unwrap();
    assert_eq!(attempts.len(), 1, "{record:#}");
    assert!(
        attempts[0]["model"]
            .as_str()
            .unwrap()
            .starts_with("recipe:")
    );
    land(&f.worlds, record["winner"].as_str().unwrap()).unwrap();

    // The project moved on: the replay fails its checks and the model runs.
    commit_file(&f, "check.sh", "grep -qx 43 answer\n");
    let fresh = Script::new("fresh", &[write("answer", "43\n"), done()]);
    let record = solve(&f.worlds, &f.project, &task(&[]), &[&fresh], 10).unwrap();
    let attempts = record["attempts"].as_array().unwrap();
    assert_eq!(attempts.len(), 2, "{record:#}");
    assert_eq!(attempts[0]["passed"], false);
    assert_eq!(record["winner"], attempts[1]["world"]);
    assert_eq!(f.worlds.list().unwrap().len(), 1);

    // A rejected latest run retires the recipe.
    review::reject(&f.worlds, record["winner"].as_str().unwrap(), None).unwrap();
    let again = Script::new("again", &[write("answer", "43\n"), done()]);
    let record = solve(&f.worlds, &f.project, &task(&[]), &[&again], 10).unwrap();
    assert_eq!(record["attempts"].as_array().unwrap().len(), 1);
    assert_eq!(record["attempts"][0]["model"], "again");
}

fn finish(summary: &str) -> (&'static str, Value) {
    ("done", json!({"summary": summary}))
}

const SPEC: &str = "goal: Write 42 into `answer`; spec.sh must pass.\nverify: sh spec.sh";

#[test]
fn a_request_compiles_to_failing_tests_that_the_solution_must_pass() {
    let f = fixture("spec");
    let request = "answer should say 42";

    // Checked by the harness, not by the human: tests that already pass, and
    // tests in the harness dir, are not a spec.
    let lazy = Script::new("lazy", &[write("spec.sh", "true\n"), finish(SPEC)]);
    let s = spec::compile(&f.worlds, &f.project, request, &lazy, 10).unwrap();
    assert_eq!(
        s["reason"],
        "the tests already pass: they do not test the request"
    );
    let sly = Script::new(
        "sly",
        &[write(".fennec-os/spec.sh", "false\n"), finish(SPEC)],
    );
    let s = spec::compile(&f.worlds, &f.project, request, &sly, 10).unwrap();
    assert!(
        s["reason"]
            .as_str()
            .unwrap()
            .starts_with("rule harness-dir"),
        "{s:#}"
    );
    assert!(f.worlds.list().unwrap().is_empty());

    // A rejected spec's correction reaches the next compile.
    let vague = Script::new(
        "vague",
        &[write("spec.sh", "test -s answer\n"), finish(SPEC)],
    );
    let s = spec::compile(&f.worlds, &f.project, request, &vague, 10).unwrap();
    assert_eq!(s["status"], "proposed", "{s:#}");
    spec::reject(
        &f.worlds,
        s["world"].as_str().unwrap(),
        Some("check the exact value"),
    )
    .unwrap();
    let spy = Spy(Mutex::new(String::new()));
    spec::compile(&f.worlds, &f.project, request, &spy, 10).unwrap();
    assert!(
        spy.0
            .lock()
            .unwrap()
            .contains("rejected with:\n  - check the exact value")
    );

    let good = Script::new(
        "good",
        &[write("spec.sh", "grep -qx 42 answer\n"), finish(SPEC)],
    );
    let s = spec::compile(&f.worlds, &f.project, request, &good, 10).unwrap();
    assert_eq!(s["status"], "proposed", "{s:#}");
    assert_eq!(s["protect"], json!(["spec.sh"]));
    assert_eq!(s["verify"], json!(["sh spec.sh"]));
    let id = s["world"].as_str().unwrap().to_string();

    // Accepted: attempts start from the tests and may not edit them.
    let cheat = Script::new("cheat", &[write("spec.sh", "true\n"), done()]);
    let honest = Script::new("honest", &[write("answer", "42\n"), done()]);
    let record = spec::accept(&f.worlds, &id, &[&cheat, &honest], 10).unwrap();
    let attempts = record["attempts"].as_array().unwrap();
    assert_eq!(
        attempts[0]["reason"],
        "rule task.protect violated: changed spec.sh"
    );
    assert_eq!(record["winner"], attempts[1]["world"]);
    assert_eq!(record["spec"], json!(id));
    assert_eq!(attempts[1]["changed_lines"], 1);

    land(&f.worlds, record["winner"].as_str().unwrap()).unwrap();
    assert!(f.project.join("spec.sh").exists());
    assert!(f.worlds.list().unwrap().is_empty());
    assert_eq!(spec::load(&f.worlds, &id).unwrap()["status"], "accepted");
}

#[test]
fn sparring_proposes_only_findings_proven_by_a_new_failing_test() {
    let f = fixture("spar");
    let finding =
        "goal: check.sh fails when `answer` is missing; spar.sh must pass.\nverify: sh spar.sh";

    // Failing by breaking what was there is not a finding.
    let vandal = Script::new(
        "vandal",
        &[
            write("check.sh", "false\n"),
            write("spar.sh", "false\n"),
            finish(finding),
        ],
    );
    let s = spec::spar(&f.worlds, &f.project, &vandal, 10).unwrap();
    assert_eq!(
        s["reason"], "tests go in new files; check.sh already existed",
        "{s:#}"
    );
    assert!(f.worlds.list().unwrap().is_empty());

    // A failing test, added: a spec to validate, like a compiled request.
    let finder = Script::new(
        "finder",
        &[write("spar.sh", "test -f answer\n"), finish(finding)],
    );
    let s = spec::spar(&f.worlds, &f.project, &finder, 10).unwrap();
    assert_eq!(s["status"], "proposed", "{s:#}");
    assert_eq!(s["origin"], "spar");
    assert_eq!(
        s["request"],
        "spar: check.sh fails when `answer` is missing; spar.sh must pass."
    );
    let id = s["world"].as_str().unwrap();
    spec::reject(&f.worlds, id, Some("a missing answer is meant to fail")).unwrap();

    // The next round knows what was found and what the human said.
    let spy = Spy(Mutex::new(String::new()));
    spec::spar(&f.worlds, &f.project, &spy, 10).unwrap();
    assert!(spy.0.lock().unwrap().contains(
        "do not report them again:\n  - check.sh fails when `answer` is missing; spar.sh must pass. (rejected: a missing answer is meant to fail)"
    ));
}

#[test]
fn a_prose_summary_gets_one_turn_to_take_the_form() {
    let f = fixture("reformat");
    let prose = Script::new(
        "prose",
        &[
            write("spec.sh", "grep -qx 42 answer\n"),
            finish("I wrote good tests."),
        ],
    );
    prose
        .replies
        .lock()
        .unwrap()
        .push_back(json!({"role": "assistant", "content": SPEC}));
    let s = spec::compile(&f.worlds, &f.project, "answer should say 42", &prose, 10).unwrap();
    assert_eq!(s["status"], "proposed", "{s:#}");
    assert_eq!(s["summary"], "I wrote good tests.");
    assert_eq!(s["verify"], json!(["sh spec.sh"]));
}

/// A model that takes its time, like a 27B model on a busy GPU.
struct Slow(Script);

impl Model for Slow {
    fn chat(&self, messages: &[Value], tools: &Value) -> io::Result<Value> {
        std::thread::sleep(std::time::Duration::from_millis(300));
        self.0.chat(messages, tools)
    }
    fn label(&self) -> String {
        self.0.label()
    }
}

#[test]
fn the_first_pass_stops_the_attempts_still_running() {
    let f = fixture("first");
    let steps: Vec<_> = (0..20).map(|i| write("notes", &format!("{i}\n"))).collect();
    let slow = Slow(Script::new("slow", &steps));
    let fast = Script::new("fast", &[write("answer", "42\n"), done()]);
    let record = solve(&f.worlds, &f.project, &task(&[]), &[&slow, &fast], 40).unwrap();
    let attempts = record["attempts"].as_array().unwrap();
    assert_eq!(attempts[0]["reason"], "stopped: another attempt passed");
    assert_eq!(record["winner"], attempts[1]["world"]);
}
