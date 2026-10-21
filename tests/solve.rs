//! Solve and land, driven by scripted models: the verdict comes from the
//! checks, never from what the model says.

mod common;

use std::collections::VecDeque;
use std::io;
use std::process::Command;
use std::sync::Mutex;

use common::Fixture;
use fennec_os::provider::Model;
use fennec_os::solve::{Task, land, plan, push, solve};
use fennec_os::{grant, proof, review, rules, skill, sleep, spec, team};
use serde_json::{Value, json};

/// The global skill list reads process-wide variables; the one test that
/// moves it holds the lock.
static ENV: Mutex<()> = Mutex::new(());

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
        // Reasoning stays in the exchange for the journal; the provider
        // leaves it out of what it sends (`provider::outgoing`).
        assert!(
            messages
                .iter()
                .filter(|m| m["role"] == "assistant")
                .all(|m| m["reasoning_content"].is_string())
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
    fn variant<'s>(&'s self, _: &str) -> Box<dyn Model + 's> {
        Box::new(Script {
            name: self.name,
            replies: Mutex::new(VecDeque::new()),
        })
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
        grant: Vec::new(),
        skills: None,
        roles: None,
        discussion: None,
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
fn attempts_run_in_turn_and_the_first_pass_lands() {
    let f = fixture("solve");
    let wrong = Script::new(
        "wrong",
        // `done` is refused twice while the check fails, then the
        // verification judges.
        &[write("answer", "41\n"), done(), done(), done()],
    );
    let noisy = Script::new(
        "noisy",
        &[write("answer", "42\n"), write("notes", "a\nb\nc\n"), done()],
    );
    let tight = Script::new("tight", &[write("answer", "42\n"), done()]);
    let models: Vec<&dyn Model> = vec![&wrong, &tight, &noisy];

    let record = solve(&f.worlds, &f.project, &task(&[]), &models, 10, None).unwrap();
    let attempts = record["attempts"].as_array().unwrap();
    // The third model is never called: the second passed.
    let passed: Vec<bool> = attempts.iter().map(|a| a["passed"] == true).collect();
    assert_eq!(passed, [false, true], "{record:#}");
    assert_eq!(noisy.replies.lock().unwrap().len(), 3);
    assert_eq!(attempts[0]["reason"], "verify failed");
    let winner = record["winner"].as_str().unwrap().to_string();
    assert_eq!(winner, attempts[1]["world"]);
    // Losers are gone; only the winner waits for landing.
    assert_eq!(f.worlds.list().unwrap(), vec![winner.clone()]);

    let before = git(&f, &["rev-parse", "HEAD"]);
    let commit = land(&f.worlds, &winner, None, Some("feat: write the answer")).unwrap();
    assert_eq!(git(&f, &["rev-parse", "HEAD"]), commit);
    // One commit, the human's, without the harness's checkpoints.
    assert_eq!(git(&f, &["rev-parse", "HEAD~1"]), before);
    assert_eq!(
        git(&f, &["log", "-1", "--format=%an <%ae> %cn|%s"]),
        "Human <human@example.com> Human|feat: write the answer"
    );
    assert_eq!(
        std::fs::read_to_string(f.project.join("answer")).unwrap(),
        "42\n"
    );
    assert!(f.worlds.list().unwrap().is_empty());
    assert!(land(&f.worlds, &winner, None, None).is_err());
}

#[test]
fn a_crashed_attempt_goes_on_in_its_own_world() {
    let f = fixture("resume");
    let world = f.worlds.fork(&f.project.to_string_lossy()).unwrap();
    let log_path = f.dir.join("live/resume.json");
    std::fs::create_dir_all(log_path.parent().unwrap()).unwrap();
    // What the crashed attempt left: its world, its conversation, and a
    // reply whose call never ran.
    std::fs::write(
        &log_path,
        json!({
            "world": world.id,
            "goal": "Write 42 into the file `answer`.",
            "steps": 1,
            "from": 0,
            "log": [
                {"role": "system", "content": "the system message"},
                {"role": "user", "content": "Write 42 into the file `answer`."},
                {
                    "role": "assistant", "content": null,
                    "reasoning_content": "hmm",
                    "tool_calls": [{
                        "id": "c0", "type": "function",
                        "function": {
                            "name": "write_file",
                            "arguments": "{\"path\": \"answer\", \"content\": \"42\\n\"}"
                        }
                    }]
                }
            ],
            "criteria": [],
            "read": []
        })
        .to_string(),
    )
    .unwrap();
    // The pending call runs without a model call; `done` is refused twice
    // (the run must say what good looks like), then it goes through.
    let spy = Spy(Mutex::new(String::new()));
    let record = solve(
        &f.worlds,
        &f.project,
        &task(&[]),
        &[&spy as &dyn Model],
        10,
        Some(&log_path),
    )
    .unwrap();
    assert_eq!(record["status"], "ready", "{record:#}");
    // The same world, not a fork: its pending write ran in it.
    assert_eq!(record["winner"], world.id);
    assert_eq!(record["attempts"][0]["world"], world.id);
    assert_eq!(
        std::fs::read_to_string(world.path.join("answer")).unwrap(),
        "42\n"
    );
    // The window the model saw: the system prompt once, not twice.
    let seen = spy.0.lock().unwrap().clone();
    assert_eq!(
        seen.matches("You work in a sandboxed copy of a project")
            .count(),
        1,
        "{record:#}"
    );
    // A run that goes on keeps its log only if it crashes again.
    assert!(!log_path.exists());
    assert_eq!(f.worlds.list().unwrap(), vec![world.id]);
}

#[test]
fn work_that_changed_nothing_fails_where_it_stands() {
    let f = fixture("unchanged");
    // What is asked already holds: the model finds nothing to change.
    std::fs::write(f.project.join("answer"), "42\n").unwrap();
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
            "answer",
        ],
    );
    let before = git(&f, &["rev-parse", "HEAD"]);
    let idle = Script::new("idle", &[done()]);
    let record = solve(
        &f.worlds,
        &f.project,
        &task(&[]),
        &[&idle as &dyn Model],
        10,
        None,
    )
    .unwrap();
    // An empty diff would pass an empty proof and land as work never done.
    assert_eq!(record["status"], "failed", "{record:#}");
    assert!(record["winner"].is_null());
    assert_eq!(
        record["attempts"][0]["reason"],
        "no file changed: the work produced nothing to prove"
    );
    assert_eq!(git(&f, &["rev-parse", "HEAD"]), before, "nothing landed");
    assert!(f.worlds.list().unwrap().is_empty());
}

#[test]
fn a_chosen_skill_is_listed_mounted_read_only_and_traced() {
    let f = fixture("skill");
    // The global choice lives outside the project: give it a home of its own.
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", f.dir.join("config"));
    }
    let dir = f.project.join(".claude/skills/answer");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("SKILL.md"),
        "---\nname: answer\ndescription: How this person answers.\n---\nAlways 42.\n",
    )
    .unwrap();
    assert_eq!(skill::catalog(Some(&f.project)).unwrap()[0].name, "answer");
    assert!(skill::chosen(Some(&f.project)).unwrap().is_empty());
    skill::choose("answer", true).unwrap();
    skill::choose("answer", true).unwrap();
    assert_eq!(skill::chosen(Some(&f.project)).unwrap().len(), 1);

    let reader = Script::new(
        "reader",
        &[
            (
                "shell",
                json!({"command": "cat ~/.skills/answer/SKILL.md && ! touch ~/.skills/answer/x"}),
            ),
            write("answer", "42\n"),
            done(),
        ],
    );
    let record = solve(&f.worlds, &f.project, &task(&[]), &[&reader], 10, None).unwrap();
    let attempt = &record["attempts"][0];
    let said = attempt["messages"].to_string();
    assert!(said.contains("answer: How this person answers."), "{said}");
    // Read inside the sandbox, and read-only: the touch failed, so exit 0.
    assert!(
        said.contains("exit 0") && said.contains("Always 42."),
        "{said}"
    );
    assert_eq!(attempt["skills"], json!(["answer"]), "{attempt:#}");
    // Framing picked none for this task: it is not told of the skill.
    let plain = Script::new("plain", &[write("answer", "42\n"), done()]);
    let mut none = task(&[]);
    none.skills = Some(vec![]);
    let record = solve(&f.worlds, &f.project, &none, &[&plain], 10, None).unwrap();
    let said = record["attempts"][0]["messages"].to_string();
    assert!(!said.contains("How this person answers."), "{said}");

    skill::choose("answer", false).unwrap();
    assert!(skill::chosen(Some(&f.project)).unwrap().is_empty());
}

#[test]
fn checks_run_in_bash_as_models_write_them() {
    let f = fixture("bash");
    let tight = Script::new("tight", &[write("answer", "42\n"), done()]);
    let mut t = task(&[]);
    t.verify = vec!["diff <(printf '42\\n') answer && [[ -s answer ]]".to_string()];
    let record = solve(&f.worlds, &f.project, &t, &[&tight], 10, None).unwrap();
    assert!(record["winner"].is_string(), "{record:#}");
}

#[test]
fn a_run_lands_on_the_branch_the_rules_ask_for_and_is_pushed() {
    let f = fixture("branch");
    let remote = f.project.with_file_name("remote.git");
    Command::new("git")
        .args(["init", "-q", "--bare"])
        .arg(&remote)
        .status()
        .unwrap();
    git(&f, &["remote", "add", "origin", remote.to_str().unwrap()]);
    let tight = Script::new("tight", &[write("answer", "42\n"), done()]);
    let record = solve(&f.worlds, &f.project, &task(&[]), &[&tight], 10, None).unwrap();
    let winner = record["winner"].as_str().unwrap().to_string();

    tight.replies.lock().unwrap().push_back(json!({
        "content": "<think>rules say feat/</think>branch: feat/answer\n\nfeat: write the answer"
    }));
    let target = plan(&f.worlds, &winner, &tight).unwrap();
    assert_eq!(target.branch, "feat/answer");
    assert_eq!(target.message.as_deref(), Some("feat: write the answer"));
    assert_eq!(target.remote.as_deref(), Some("origin"));

    let before = git(&f, &["rev-parse", "HEAD"]);
    let commit = land(
        &f.worlds,
        &winner,
        Some(&target.branch),
        target.message.as_deref(),
    )
    .unwrap();
    // The human's branch and worktree are untouched; the work is on its own.
    assert_eq!(git(&f, &["rev-parse", "HEAD"]), before);
    assert!(!f.project.join("answer").exists());
    assert_eq!(git(&f, &["rev-parse", "feat/answer"]), commit);
    assert_eq!(git(&f, &["rev-parse", "feat/answer~1"]), before);

    push(&f.project, "origin", "feat/answer").unwrap();
    let pushed = Command::new("git")
        .arg("--git-dir")
        .arg(&remote)
        .args(["rev-parse", "feat/answer"])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&pushed.stdout).trim(), commit);
}

#[test]
fn work_queued_on_a_branch_builds_on_what_landed_there() {
    let mut f = fixture("queue-branch");
    f.worlds.base = Some("fleet".to_string());
    let before = git(&f, &["rev-parse", "HEAD"]);
    let first = Script::new("first", &[write("answer", "42\n"), done()]);
    let record = solve(&f.worlds, &f.project, &task(&[]), &[&first], 10, None).unwrap();
    let one = land(
        &f.worlds,
        record["winner"].as_str().unwrap(),
        Some("fleet"),
        Some("one"),
    )
    .unwrap();
    assert_eq!(git(&f, &["rev-parse", "fleet"]), one);

    // The next one starts from the branch: it sees the first one's work.
    let second = Script::new("second", &[write("more", "x\n"), done()]);
    let mut t = task(&[]);
    // Another goal: the same one would replay the first run's calls.
    t.goal = "Write the file `more`.".to_string();
    t.verify = vec!["test -f answer".to_string()];
    let record = solve(&f.worlds, &f.project, &t, &[&second], 10, None).unwrap();
    let two = land(
        &f.worlds,
        record["winner"].as_str().unwrap(),
        Some("fleet"),
        Some("two"),
    )
    .unwrap();
    assert_eq!(git(&f, &["rev-parse", "fleet~1"]), one);
    assert_eq!(git(&f, &["rev-parse", "fleet"]), two);

    // Work started elsewhere is replayed on the branch and checked again.
    f.worlds.base = None;
    let third = Script::new("third", &[write("third", "3\n"), done()]);
    let mut t = task(&[]);
    t.goal = "Write the file `third`.".to_string();
    t.verify = vec!["test -f third".to_string()];
    let record = solve(&f.worlds, &f.project, &t, &[&third], 10, None).unwrap();
    land(
        &f.worlds,
        record["winner"].as_str().unwrap(),
        Some("fleet"),
        Some("three"),
    )
    .unwrap();
    assert_eq!(git(&f, &["rev-parse", "fleet~1"]), two);
    // The human's branch and folder never moved.
    assert_eq!(git(&f, &["rev-parse", "HEAD"]), before);
    assert!(!f.project.join("answer").exists());
}

#[test]
fn a_proof_clicks_like_a_user_and_fails_on_what_the_page_does_not_show() {
    if Command::new("chromium").arg("--version").output().is_err() {
        return; // no browser on this machine: nothing to drive
    }
    let f = fixture("clicks");
    let page = "<!doctype html><ul><li onclick=\"this.style.textDecoration='line-through'\">pain</li></ul>";
    let model = Script::new(
        "page",
        &[write("answer", "42\n"), write("index.html", page), done()],
    );
    let record = solve(&f.worlds, &f.project, &task(&[]), &[&model], 10, None).unwrap();
    let winner = record["winner"].as_str().unwrap().to_string();
    let serve = "serve: python3 -m http.server 8765\nshot: http://127.0.0.1:8765/\n";

    model
        .replies
        .lock()
        .unwrap()
        .push_back(json!({"content": format!("{serve}act: click `pain`\n")}));
    let shown = proof::gather(&f.worlds, &winner, Some("pain se barre"), &model).unwrap();
    let shot = &shown["shots"][0];
    assert_eq!(shot["ok"], true, "{shot}");
    assert!(shot["before"].is_string() && shot["file"].is_string());
    assert_eq!(shown["passed"], true);

    model
        .replies
        .lock()
        .unwrap()
        .push_back(json!({"content": format!("{serve}act: click beurre\n")}));
    let missing = proof::gather(&f.worlds, &winner, None, &model).unwrap();
    let shot = &missing["shots"][0];
    assert_eq!(shot["ok"], false);
    assert!(
        shot["reason"].as_str().unwrap().contains("« beurre »"),
        "{shot}"
    );
    assert_eq!(missing["passed"], false);
}

#[test]
fn a_page_without_a_browser_to_look_at_is_unverified_not_failed() {
    if Command::new("chromium").arg("--version").output().is_ok() {
        return; // this machine can look: the test above covers it
    }
    let f = fixture("nobrowser");
    let model = Script::new(
        "nobrowser",
        &[
            write("answer", "42\n"),
            write("index.html", "<p>ok</p>"),
            done(),
        ],
    );
    let record = solve(&f.worlds, &f.project, &task(&[]), &[&model], 10, None).unwrap();
    let winner = record["winner"].as_str().unwrap().to_string();
    let page = "serve: python3 -m http.server 8766\nshot: http://127.0.0.1:8766/\n";
    let prove = |reply: String| {
        model
            .replies
            .lock()
            .unwrap()
            .push_back(json!({"content": reply}));
        proof::gather(&f.worlds, &winner, None, &model).unwrap()
    };
    let shown = prove(format!("{page}try: grep -qx 42 answer\n"));
    assert_eq!(shown["shots"][0]["missing"], true, "{shown}");
    assert_eq!(
        shown["passed"], true,
        "the try proves it; the page is unverified"
    );
    assert_eq!(
        prove(page.to_string())["passed"],
        false,
        "a page unseen proves nothing"
    );
}

#[test]
fn a_proof_whose_reasoning_eats_the_reply_is_asked_again() {
    let f = fixture("rethink");
    let model = Script::new("think", &[write("answer", "42\n"), done()]);
    let record = solve(&f.worlds, &f.project, &task(&[]), &[&model], 10, None).unwrap();
    let winner = record["winner"].as_str().unwrap().to_string();
    let mut replies = model.replies.lock().unwrap();
    replies.push_back(json!({"content": "", "reasoning_content": "on and on"}));
    replies.push_back(json!({"content": "try: grep -qx 42 answer\n"}));
    drop(replies);
    let shown = proof::gather(&f.worlds, &winner, None, &model).unwrap();
    assert_eq!(shown["passed"], true, "{shown}");
    assert!(model.replies.lock().unwrap().is_empty());
}

#[test]
fn a_role_check_vetoes_the_proof_and_a_missing_tool_is_only_unverified() {
    let f = fixture("veto");
    let model = Script::new("veto", &[write("answer", "42\n"), done()]);
    let record = solve(&f.worlds, &f.project, &task(&[]), &[&model], 10, None).unwrap();
    let winner = record["winner"].as_str().unwrap().to_string();
    let team = f.project.join(".fennec-os/team");
    std::fs::create_dir_all(&team).unwrap();
    let role = |checks: &str| {
        let text = format!("---\nname: Sec\nstage: prove\n{checks}---\nAudit.\n");
        std::fs::write(team.join("sec.md"), text).unwrap();
    };
    let prove = || {
        let reply = json!({"content": "try: grep -qx 42 answer\n"});
        model.replies.lock().unwrap().push_back(reply);
        proof::gather(&f.worlds, &winner, None, &model).unwrap()
    };

    role("check: grep -q 42 answer\ncheck: no-such-tool-xyz\ncheck: test -e secrets\n");
    let vetoed = prove();
    assert_eq!(vetoed["passed"], false, "{vetoed}");
    let checks = vetoed["checks"].as_array().unwrap();
    let exits: Vec<(&Value, &Value, &Value)> = checks
        .iter()
        .map(|c| (&c["role"], &c["exit"], &c["missing"]))
        .collect();
    assert_eq!(
        exits,
        [
            (&json!("Sec"), &json!(0), &json!(false)),
            (&json!("Sec"), &json!(127), &json!(true)),
            (&json!("Sec"), &json!(1), &json!(false)),
        ]
    );

    role("check: no-such-tool-xyz\n");
    assert_eq!(prove()["passed"], true, "a missing tool does not block");

    // The role's checks alone are not a try of the product.
    role("check: true\n");
    model
        .replies
        .lock()
        .unwrap()
        .push_back(json!({"content": "unverified: everything\n"}));
    let alone = proof::gather(&f.worlds, &winner, None, &model).unwrap();
    assert_eq!(alone["passed"], false, "{alone}");
}

#[test]
fn editing_a_protected_check_fails_even_if_it_passes() {
    let f = fixture("protect");
    let cheat = Script::new("cheat", &[write("check.sh", "exit 0\n"), done()]);
    let record = solve(
        &f.worlds,
        &f.project,
        &task(&["check.sh"]),
        &[&cheat],
        10,
        None,
    )
    .unwrap();
    assert_eq!(record["winner"], Value::Null);
    assert_eq!(
        record["attempts"][0]["reason"],
        "rule task.protect violated: changed check.sh"
    );
    assert!(f.worlds.list().unwrap().is_empty());
}

#[test]
fn a_world_edited_after_its_verdict_lands_only_if_its_checks_pass_again() {
    let f = fixture("tamper");
    let tight = Script::new("tight", &[write("answer", "42\n"), done()]);
    let record = solve(&f.worlds, &f.project, &task(&[]), &[&tight], 10, None).unwrap();
    let id = record["winner"].as_str().unwrap();
    let world = f.worlds.open(id).unwrap();
    // The human edits the work in their editor and breaks it: refused.
    std::fs::write(world.path.join("answer"), "41\n").unwrap();
    assert!(land(&f.worlds, id, None, None).is_err());
    assert!(!f.project.join("answer").exists());
    // Fixed, with a file of their own: it lands, their file included.
    std::fs::write(world.path.join("answer"), "42\n").unwrap();
    std::fs::write(world.path.join("extra"), "mine\n").unwrap();
    land(&f.worlds, id, None, None).unwrap();
    assert!(f.project.join("extra").exists() && f.project.join("answer").exists());
}

#[test]
fn a_blank_reply_turns_reasoning_off_for_the_rest_of_the_run() {
    // The default policy is always: run this one under adaptive.
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
    unsafe {
        std::env::set_var("FENNEC_OS_THINK", "adaptive");
    }
    let f = fixture("blank");
    let model = Script::new("blank", &[write("answer", "42\n"), done()]);
    // First, a reply whose reasoning ate the whole budget.
    let mut replies = model.replies.lock().unwrap();
    replies.insert(0, json!({"content": "", "reasoning_content": "on and on"}));
    drop(replies);
    let record = solve(&f.worlds, &f.project, &task(&[]), &[&model], 10, None).unwrap();
    assert!(record["winner"].is_string(), "{record}");
    let think: Vec<Value> = record["attempts"][0]["turns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["think"].clone())
        .collect();
    // The first turn already skips reasoning (the server's prefix cache
    // only pays for non-thinking turns); a blank reply keeps it off.
    assert_eq!(think, [json!(false), json!(false), json!(false)]);
    // A tool error reasons the next turn, one turn only: a text-only turn
    // does not (a thinking turn would cost the server's prefix cache the
    // turn after it).
    let f = fixture("blank2");
    let model = Script::new(
        "blank",
        &[
            ("shell", json!({"command": "ls missing"})),
            write("answer", "42\n"),
            done(),
        ],
    );
    let record = solve(&f.worlds, &f.project, &task(&[]), &[&model], 10, None).unwrap();
    assert!(record["winner"].is_string(), "{record}");
    let think: Vec<Value> = record["attempts"][0]["turns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["think"].clone())
        .collect();
    assert_eq!(think, [json!(false), json!(true), json!(false)]);
}

#[test]
fn an_idle_model_is_told_twice_then_stopped_at_step_25() {
    let f = fixture("idle-hard");
    // Reading two different files: no repeat loop, no file changed.
    let mut steps: Vec<(&str, Value)> = Vec::new();
    for i in 0..30 {
        let file = if i % 2 == 0 { "check.sh" } else { "README" };
        steps.push(("read_file", json!({"path": file})));
    }
    let idle = Script::new("idle", &steps);
    let record = solve(&f.worlds, &f.project, &task(&[]), &[&idle], 30, None).unwrap();
    let attempt = &record["attempts"][0];
    assert!(
        attempt["reason"]
            .as_str()
            .unwrap()
            .starts_with("stuck: no file changed in 25 steps"),
        "{record:#}"
    );
    let counts = |needle: &str| {
        attempt["messages"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|m| m["content"].as_str().is_some_and(|c| c.contains(needle)))
            .count()
    };
    assert_eq!(counts("15 steps without changing any file"), 1);
    assert_eq!(counts("20 steps without a single file changed"), 1);
}

#[test]
fn an_edit_before_step_20_spares_the_harder_notice() {
    let f = fixture("idle-soft");
    let mut steps: Vec<(&str, Value)> = Vec::new();
    for i in 0..17 {
        let file = if i % 2 == 0 { "check.sh" } else { "README" };
        steps.push(("read_file", json!({"path": file})));
    }
    steps.push(write("answer", "42\n"));
    steps.push(done());
    let late = Script::new("late", &steps);
    let record = solve(&f.worlds, &f.project, &task(&[]), &[&late], 30, None).unwrap();
    let attempt = &record["attempts"][0];
    assert_eq!(attempt["passed"], true, "{record:#}");
    let counts = |needle: &str| {
        attempt["messages"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|m| m["content"].as_str().is_some_and(|c| c.contains(needle)))
            .count()
    };
    assert_eq!(counts("15 steps without changing any file"), 1);
    assert_eq!(counts("20 steps without a single file changed"), 0);
}

#[test]
fn the_discussion_reaches_the_producers_of_the_run() {
    let f = fixture("discussion");
    let mut with = task(&[]);
    with.discussion = Some("AGREED: the answer lives in `answer`, not `out`.".to_string());
    let spy = Spy(Mutex::new(String::new()));
    let record = solve(&f.worlds, &f.project, &with, &[&spy], 10, None).unwrap();
    assert!(record["winner"].is_null());
    let seen = spy.0.lock().unwrap();
    assert!(
        seen.contains("AGREED: the answer lives in `answer`, not `out`."),
        "{seen}"
    );
    assert!(seen.contains("do not re-litigate it"), "{seen}");
    let plain = Spy(Mutex::new(String::new()));
    let _ = solve(&f.worlds, &f.project, &task(&[]), &[&plain], 10, None).unwrap();
    assert!(!plain.0.lock().unwrap().contains("do not re-litigate it"));
}

#[test]
fn the_spec_discussion_reaches_the_solution_not_the_proof() {
    let f = fixture("spec-discussion");
    let request = "answer should say 42";
    let good = Script::new(
        "good",
        &[write("spec.sh", "grep -qx 42 answer\n"), finish(SPEC)],
    );
    let s = spec::compile(&f.worlds, &f.project, request, &good, 10).unwrap();
    let id = s["world"].as_str().unwrap().to_string();
    spec::set_discussion(
        &f.worlds,
        &id,
        "AGREED: the answer lives in `answer`, not `out`.",
    )
    .unwrap();
    let honest = Script::new("honest", &[write("answer", "42\n"), done()]);
    let record = spec::accept(&f.worlds, &id, &[&honest], 10, None).unwrap();
    let winner = record["winner"].as_str().unwrap().to_string();
    // The solution's system prompt carried the discussion...
    let role_prompt = record["attempts"][0]["roles"][0]["messages"][0]["content"]
        .as_str()
        .unwrap();
    assert!(
        role_prompt.contains("AGREED: the answer lives in `answer`, not `out`."),
        "{role_prompt}"
    );
    // ...the proof's request did not.
    let spy = Spy(Mutex::new(String::new()));
    proof::gather(&f.worlds, &winner, None, &spy).unwrap();
    let asked = spy.0.lock().unwrap();
    assert!(!asked.contains("AGREED:"), "{asked}");
    assert!(!asked.contains("do not re-litigate it"), "{asked}");
}

#[test]
fn a_chain_role_leaves_relay_notes_the_next_role_reads() {
    let f = fixture("relay");
    // Two roles in team order: senior-developer builds, tech-lead reviews.
    team::choose(
        &f.project,
        team::DEFAULT_TEAM,
        &["frontend-designer".into()],
    )
    .unwrap();
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
            "team",
        ],
    );
    let chain = Script::new(
        "chain",
        &[
            write("answer", "42\n"),
            write(
                "relay/senior-developer.md",
                "wrote the answer; nothing found\n",
            ),
            done(),
            ("read_file", json!({"path": "relay/senior-developer.md"})),
            write("relay/tech-lead.md", "the answer is right\n"),
            done(),
        ],
    );
    let record = solve(&f.worlds, &f.project, &task(&[]), &[&chain], 30, None).unwrap();
    assert!(record["winner"].is_string(), "{record:#}");
    let attempt = &record["attempts"][0];
    let roles = attempt["roles"].as_array().unwrap();
    assert_eq!(roles.len(), 2, "{record:#}");
    // The builder's transcript shows the note it left for the next role.
    assert!(
        roles[0]["messages"]
            .to_string()
            .contains("relay/senior-developer.md")
    );
    // The reviewer's goal names the notes, and its transcript carries the
    // builder's words.
    let goal = roles[1]["messages"][1]["content"].as_str().unwrap();
    assert!(
        goal.contains("Their notes are in relay/: senior-developer.md"),
        "{goal}"
    );
    assert!(
        roles[1]["messages"]
            .to_string()
            .contains("wrote the answer")
    );
    // The note is in the winner's world, for the journal and the card.
    let world = f.worlds.open(record["winner"].as_str().unwrap()).unwrap();
    let note =
        std::fs::read_to_string(world.path.join("relay").join("senior-developer.md")).unwrap();
    assert_eq!(note, "wrote the answer; nothing found\n");
}

/// Opt-in (`-- --ignored`): it needs a live server and a big, cacheable
/// prefix, and it is flaky under the parallel suite's CPU load, which evicts
/// its checkpoint from the server's shared KV pool mid-run.
#[test]
#[ignore]
fn a_run_reuses_the_servers_prefix_from_its_second_turn() {
    // Needs the configured server; every other test runs offline.
    let base = fennec_os::provider::DEFAULT_BASE_URL;
    let Ok(models) = fennec_os::provider::served(base, None) else {
        return;
    };
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
    unsafe {
        std::env::remove_var("FENNEC_OS_THINK");
    }
    let f = fixture("cache");
    // A big committed knowhow lands in the stable system prompt: measured,
    // ~12k prefill tokens survive the shared KV pool, small prompts (<~1k)
    // do not, so the test needs this to be cacheable.
    let knowhow = (0..2000)
        .map(|i| format!("- fact {i}: the answer file stays small"))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::create_dir_all(f.project.join(".fennec-os")).unwrap();
    std::fs::write(f.project.join(".fennec-os/knowhow.md"), knowhow).unwrap();
    git(&f, &["add", "-A"]);
    git(&f, &["commit", "-m", "knowhow"]);
    let model = fennec_os::provider::OpenAi::new(base, &models[0], None, 0.3);
    let hit = |r: &Value| {
        r["attempts"][0]["turns"]
            .as_array()
            .unwrap()
            .iter()
            .skip(1)
            .any(|t| {
                t["usage"]["prompt_tokens_details"]["cached_tokens"]
                    .as_u64()
                    .is_some_and(|c| c > 0)
            })
    };
    // The non-thinking turns store the prompt KV and the later ones read it
    // back: with the old always-thinking default every turn was 0 (measured:
    // 75 turns, 0 cached). The pool is shared by concurrent sessions, so a
    // run's checkpoint can be evicted between its own turns; try again when
    // it is eaten.
    let mut record = solve(&f.worlds, &f.project, &task(&[]), &[&model], 10, None).unwrap();
    for _ in 0..2 {
        if hit(&record) {
            break;
        }
        record = solve(&f.worlds, &f.project, &task(&[]), &[&model], 10, None).unwrap();
    }
    let turns = record["attempts"][0]["turns"].as_array().unwrap();
    assert!(turns.len() >= 2, "{record:#}");
    assert!(
        hit(&record),
        "no turn past the first reused any of the server's prefix cache ({record:#})"
    );
}

#[test]
fn a_model_that_stops_answering_is_a_failed_attempt() {
    let f = fixture("exhausted");
    let mute = Script::new("mute", &[write("answer", "42\n")]);
    let record = solve(&f.worlds, &f.project, &task(&[]), &[&mute], 10, None).unwrap();
    assert_eq!(record["attempts"][0]["reason"], "script exhausted");
    assert_eq!(record["winner"], Value::Null);
    // The turns before the failure stay, for the profile.
    assert_eq!(record["attempts"][0]["turns"].as_array().unwrap().len(), 2);
}

#[test]
fn a_model_in_a_loop_is_told_then_stopped() {
    let f = fixture("loop");
    let same = ("shell", json!({"command": "cat check.sh"}));
    let looping = Script::new("loop", &vec![same; 6]);
    let record = solve(&f.worlds, &f.project, &task(&[]), &[&looping], 20, None).unwrap();
    let attempt = &record["attempts"][0];
    let reason = attempt["reason"].as_str().unwrap();
    assert!(
        reason.starts_with("stuck: the same call gave the same result 5 times"),
        "{reason}"
    );
    assert!(reason.contains("`cat check.sh`"), "{reason}");
    let told = attempt["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| {
            m["content"]
                .as_str()
                .is_some_and(|c| c.contains("Try a different approach"))
        })
        .count();
    assert_eq!(told, 2, "the model is told twice before being stopped");
}

#[test]
fn a_world_rebases_and_reverifies_when_the_project_moved() {
    let f = fixture("rebase");
    let tight = Script::new("tight", &[write("answer", "42\n"), done()]);
    let record = solve(&f.worlds, &f.project, &task(&[]), &[&tight], 10, None).unwrap();
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

    let commit = land(&f.worlds, id, None, None).unwrap();
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
    let record = solve(&f.worlds, &f.project, &task(&[]), &[&tight], 10, None).unwrap();
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

    let err = land(&f.worlds, id, None, None).unwrap_err().to_string();
    assert!(err.contains("verify failed after rebase"), "{err}");
    assert_eq!(git(&f, &["rev-parse", "HEAD"]), head);
    assert!(land(&f.worlds, id, None, None).is_err());
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
        &[&bare, &cheat, &paired],
        10,
        None,
    )
    .unwrap();
    let a = record["attempts"].as_array().unwrap();
    assert_eq!(
        a[0]["reason"],
        "rule r1 violated: changed answer but nothing under notes"
    );
    assert_eq!(
        a[1]["reason"],
        "rule harness-dir violated: changed .fennec-os/rules.json"
    );
    assert_eq!(a[2]["passed"], true, "{:#}", a[2]);
    // A failing `warn` rule is evidence, not a verdict.
    assert!(
        a[2]["checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["rule"] == "r2" && c["exit"] == 1)
    );
    assert_eq!(record["winner"], a[2]["world"]);
}

#[test]
fn a_correction_said_twice_is_counted_and_promoted() {
    let f = fixture("review");
    let mut ids = Vec::new();
    for _ in 0..2 {
        let tight = Script::new("tight", &[write("answer", "42\n"), done()]);
        let record = solve(&f.worlds, &f.project, &task(&[]), &[&tight], 10, None).unwrap();
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
        *self.0.lock().unwrap() = messages
            .iter()
            .filter_map(|m| m["content"].as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let (tool, args) = done();
        Ok(
            json!({"tool_calls": [{"id": "c", "function": {"name": tool, "arguments": args.to_string()}}]}),
        )
    }
    fn label(&self) -> String {
        "spy".to_string()
    }
    fn variant<'s>(&'s self, _: &str) -> Box<dyn Model + 's> {
        Box::new(Spy(Mutex::new(String::new())))
    }
}

#[test]
fn a_new_run_starts_from_the_project_history() {
    let f = fixture("agenda");
    let tight = Script::new("tight", &[write("answer", "42\n"), done()]);
    let first = solve(&f.worlds, &f.project, &task(&[]), &[&tight], 10, None).unwrap();
    land(&f.worlds, first["winner"].as_str().unwrap(), None, None).unwrap();
    let again = Script::new(
        "again",
        &[write("answer", "42\n"), write("x", "1\n"), done()],
    );
    let second = solve(&f.worlds, &f.project, &task(&[]), &[&again], 10, None).unwrap();
    review::reject(
        &f.worlds,
        second["winner"].as_str().unwrap(),
        Some("no stray files"),
    )
    .unwrap();

    let spy = Spy(Mutex::new(String::new()));
    solve(&f.worlds, &f.project, &task(&["tests"]), &[&spy], 10, None).unwrap();
    let system = spy.0.lock().unwrap().clone();
    assert!(
        system.contains("landed \"Write 42 into the file `answer`.\": all good, trust me"),
        "{system}"
    );
    assert!(system.contains("correction: no stray files"), "{system}");
    assert!(system.contains("do not change tests"), "{system}");
}

/// Answers every distillation with the same facts, reasoning inline.
#[derive(Clone)]
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
    fn variant<'s>(&'s self, _: &str) -> Box<dyn Model + 's> {
        Box::new(self.clone())
    }
}

#[test]
fn sleep_proposes_know_how_that_a_human_keeps() {
    let f = fixture("sleep");
    let tight = Script::new("tight", &[write("answer", "42\n"), done()]);
    let run = solve(&f.worlds, &f.project, &task(&[]), &[&tight], 10, None).unwrap();
    land(&f.worlds, run["winner"].as_str().unwrap(), None, None).unwrap();

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
    solve(&f.worlds, &f.project, &next, &[&spy], 10, None).unwrap();
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
    let record = solve(&f.worlds, &f.project, &task(&[]), &[&first], 10, None).unwrap();
    land(&f.worlds, record["winner"].as_str().unwrap(), None, None).unwrap();
    commit_file(&f, "answer", "0\n");

    // Same goal again: the recipe passes, the model is never called.
    let mute = Script::new("mute", &[]);
    let record = solve(&f.worlds, &f.project, &task(&[]), &[&mute], 10, None).unwrap();
    let attempts = record["attempts"].as_array().unwrap();
    assert_eq!(attempts.len(), 1, "{record:#}");
    assert!(
        attempts[0]["model"]
            .as_str()
            .unwrap()
            .starts_with("recipe:")
    );
    land(&f.worlds, record["winner"].as_str().unwrap(), None, None).unwrap();

    // The project moved on: the replay fails its checks and the model runs.
    commit_file(&f, "check.sh", "grep -qx 43 answer\n");
    let fresh = Script::new("fresh", &[write("answer", "43\n"), done()]);
    let record = solve(&f.worlds, &f.project, &task(&[]), &[&fresh], 10, None).unwrap();
    let attempts = record["attempts"].as_array().unwrap();
    assert_eq!(attempts.len(), 2, "{record:#}");
    assert_eq!(attempts[0]["passed"], false);
    assert_eq!(record["winner"], attempts[1]["world"]);
    assert_eq!(f.worlds.list().unwrap().len(), 1);

    // A rejected latest run retires the recipe.
    review::reject(&f.worlds, record["winner"].as_str().unwrap(), None).unwrap();
    let again = Script::new("again", &[write("answer", "43\n"), done()]);
    let record = solve(&f.worlds, &f.project, &task(&[]), &[&again], 10, None).unwrap();
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
        &[
            write("spec.sh", "test -s answer\n"),
            write("probe.html", ""),
            finish(SPEC),
        ],
    );
    let s = spec::compile(&f.worlds, &f.project, request, &vague, 10).unwrap();
    assert_eq!(s["status"], "proposed", "{s:#}");
    // An empty file left by the framer is not a test to protect.
    assert_eq!(s["protect"], json!(["spec.sh"]), "{s:#}");
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

    // A stopped compile asks a question; the answer reaches the next one.
    let lost = Script::new("lost", &vec![("shell", json!({"command": "ls"})); 5]);
    lost.replies
        .lock()
        .unwrap()
        .push_back(json!({"content": "question: Which file holds the answer?"}));
    let s = spec::compile(&f.worlds, &f.project, request, &lost, 10).unwrap();
    assert_eq!(s["status"], "invalid", "{s:#}");
    assert_eq!(s["question"], "Which file holds the answer?");
    assert!(
        s["reason"]
            .as_str()
            .unwrap()
            .starts_with("stuck: the same call")
    );
    spec::answer(
        &f.worlds,
        s["world"].as_str().unwrap(),
        "Which file holds the answer?",
        "the file `answer`",
    )
    .unwrap();
    let spy = Spy(Mutex::new(String::new()));
    spec::compile(&f.worlds, &f.project, request, &spy, 10).unwrap();
    assert!(
        spy.0
            .lock()
            .unwrap()
            .contains("Q: Which file holds the answer?\n    A: the file `answer`")
    );

    // A request too vague to test is asked about, not invented.
    let wary = Script::new("wary", &[finish("question: What should it do?")]);
    let s = spec::compile(&f.worlds, &f.project, "ceci est un test", &wary, 10).unwrap();
    assert_eq!(s["question"], "What should it do?", "{s:#}");
    assert_eq!(s["reason"], "asks before writing tests");

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
    let record = spec::accept(&f.worlds, &id, &[&cheat, &honest], 10, None).unwrap();
    let attempts = record["attempts"].as_array().unwrap();
    assert_eq!(
        attempts[0]["reason"],
        "rule task.protect violated: changed spec.sh"
    );
    assert_eq!(record["winner"], attempts[1]["world"]);
    assert_eq!(record["spec"], json!(id));
    assert_eq!(attempts[1]["changed_lines"], 1);

    // The winner's product failed later: retried from the tests, told why.
    let first = record["winner"].as_str().unwrap().to_string();
    let spy = Spy(Mutex::new(String::new()));
    let again = spec::retry(
        &f.worlds,
        &id,
        "the page renders no text",
        &[&spy],
        10,
        None,
    )
    .unwrap();
    assert!(
        again["goal"]
            .as_str()
            .unwrap()
            .ends_with("An earlier solution failed: the page renders no text\nDo not repeat it."),
        "{again:#}"
    );
    assert_eq!(
        review::load(&f.worlds, &first).unwrap()["status"],
        "rejected"
    );
    assert!(f.worlds.open(&first).is_err());
    let honest = Script::new("honest", &[write("answer", "42\n"), done()]);
    let record = spec::retry(&f.worlds, &id, "x", &[&honest], 10, None).unwrap();
    assert!(record["winner"].is_string(), "{record:#}");

    // The human asks for a change on the ready run: a failed change leaves it
    // ready; a passing one starts from its work and replaces it.
    let ready = record["winner"].as_str().unwrap().to_string();
    let broken = Script::new("broken", &[write("answer", "41\n"), done()]);
    let failed = spec::amend(&f.worlds, &id, "add a note", &[&broken], 10, None).unwrap();
    assert!(failed["winner"].is_null(), "{failed:#}");
    assert!(f.worlds.open(&ready).is_ok());
    assert_eq!(spec::load(&f.worlds, &id).unwrap()["run"], json!(ready));
    // It writes only the note: `answer` is already there, from the ready run.
    let noter = Script::new("noter", &[write("note", "hi\n"), done()]);
    let record = spec::amend(&f.worlds, &id, "add a note", &[&noter], 10, None).unwrap();
    assert!(
        record["goal"].as_str().unwrap().contains("add a note"),
        "{record:#}"
    );
    let amended = record["winner"].as_str().unwrap().to_string();
    assert_eq!(record["spec"], json!(id));
    assert_eq!(
        review::load(&f.worlds, &ready).unwrap()["status"],
        "rejected"
    );
    assert_eq!(spec::load(&f.worlds, &id).unwrap()["run"], json!(amended));
    // The proof failed on it: fixed from its work, told what failed, and the
    // findings are no human correction.
    let fixer = Script::new("fixer", &[write("note", "hi!\n"), done()]);
    let record = spec::fix(
        &f.worlds,
        &id,
        "`cat note` printed hi, not hi!",
        &[&fixer],
        10,
        None,
    )
    .unwrap();
    assert!(
        record["goal"]
            .as_str()
            .unwrap()
            .contains("printed hi, not hi!")
    );
    let fixed = record["winner"].as_str().unwrap().to_string();
    assert_eq!(
        review::load(&f.worlds, &amended).unwrap()["status"],
        "rejected"
    );
    assert!(review::load(&f.worlds, &amended).unwrap()["correction"].is_null());
    assert_eq!(spec::load(&f.worlds, &id).unwrap()["run"], json!(fixed));
    let amended = fixed;
    // The commit message is written from all it carries, not the correction alone.
    let spy = Spy(Mutex::new(String::new()));
    let _ = plan(&f.worlds, &amended, &spy);
    let asked = spy.0.lock().unwrap().clone();
    assert!(asked.contains("+42") && asked.contains("+hi"), "{asked}");

    land(&f.worlds, &amended, None, None).unwrap();
    assert!(f.project.join("note").exists() && f.project.join("answer").exists());
    assert!(f.project.join("spec.sh").exists());
    assert!(f.worlds.list().unwrap().is_empty());
    assert_eq!(spec::load(&f.worlds, &id).unwrap()["status"], "accepted");
}

/// A host config file, a task that edits it, and a check that reads it.
fn host_task(f: &Fixture) -> (std::path::PathBuf, Task) {
    let conf = f.worlds.home.join("conf");
    std::fs::create_dir_all(&conf).unwrap();
    std::fs::write(conf.join("app.cfg"), "v1\n").unwrap();
    let task = Task {
        goal: "Set the app config to v2.".to_string(),
        verify: vec![format!("grep -qx v2 {}/app.cfg", conf.display())],
        protect: Vec::new(),
        base: None,
        grant: vec![conf.clone()],
        skills: None,
        roles: None,
        discussion: None,
    };
    (conf, task)
}

#[test]
fn a_granted_run_edits_the_host_until_the_human_decides() {
    let f = fixture("solve-grant");
    let (conf, task) = host_task(&f);
    let cfg = || std::fs::read_to_string(conf.join("app.cfg")).unwrap();
    let path = format!("{}/app.cfg", conf.display());

    let good = Script::new("good", &[write(&path, "v2\n"), done()]);
    let record = solve(&f.worlds, &f.project, &task, &[&good], 10, None).unwrap();
    assert_eq!(record["status"], "ready", "{record:#}");
    assert_eq!(record["grant"], json!([conf.display().to_string()]));
    // The host changed and waits for the human; the winner's world waits too.
    assert_eq!(cfg(), "v2\n");
    let winner = record["winner"].as_str().unwrap();
    assert!(
        grant::diff(&f.worlds, &f.worlds.open(winner).unwrap())
            .unwrap()
            .contains("+v2")
    );
    grant::revert(&f.worlds, &f.worlds.open(winner).unwrap()).unwrap();
    assert_eq!(cfg(), "v1\n");
    f.worlds.drop(f.worlds.open(winner).unwrap()).unwrap();
}

#[test]
fn a_failed_granted_run_leaves_the_host_as_it_found_it() {
    let f = fixture("solve-grant-fail");
    let (conf, task) = host_task(&f);
    let path = format!("{}/app.cfg", conf.display());

    let bad = Script::new("bad", &[write(&path, "v3\n"), done()]);
    let record = solve(&f.worlds, &f.project, &task, &[&bad], 10, None).unwrap();
    assert_eq!(record["status"], "failed", "{record:#}");
    assert_eq!(
        std::fs::read_to_string(conf.join("app.cfg")).unwrap(),
        "v1\n"
    );
    assert!(f.worlds.list().unwrap().is_empty());
    assert!(
        !f.worlds
            .root
            .join("grants")
            .read_dir()
            .unwrap()
            .any(|_| true)
    );
}

#[test]
fn a_grant_refuses_parallel_attempts_and_bad_paths() {
    let f = fixture("solve-grant-refuse");
    let (_, mut task) = host_task(&f);
    let (a, b) = (Script::new("a", &[done()]), Script::new("b", &[done()]));
    assert!(solve(&f.worlds, &f.project, &task, &[&a, &b], 10, None).is_err());

    task.grant = vec![f.worlds.home.clone()];
    assert!(solve(&f.worlds, &f.project, &task, &[&a], 10, None).is_err());
    assert!(f.worlds.list().unwrap().is_empty());
}

#[test]
fn a_rule_said_once_reaches_every_agent_which_reports_using_it() {
    let f = fixture("said");
    // Uncommitted on purpose: a confirmed rule applies to the next run.
    rules::say(&f.project, "project", "use uv, never .venv").unwrap();
    rules::say(&f.project, "project", "use uv, never .venv").unwrap();
    let agents = std::fs::read_to_string(f.project.join("AGENTS.md")).unwrap();
    assert_eq!(agents, "## Rules\n\n- use uv, never .venv\n");

    let spy = Spy(Mutex::new(String::new()));
    solve(&f.worlds, &f.project, &task(&[]), &[&spy], 10, None).unwrap();
    let system = spy.0.lock().unwrap().clone();
    // The user's own rules come first; find this one's number.
    let n = system
        .lines()
        .find_map(|l| l.trim().strip_suffix(". use uv, never .venv"))
        .unwrap_or_else(|| panic!("{system}"));

    let tight = Script::new(
        "tight",
        &[
            write("answer", "42\n"),
            finish(&format!("wrote it\nrules: {n}")),
        ],
    );
    let record = solve(&f.worlds, &f.project, &task(&[]), &[&tight], 10, None).unwrap();
    assert_eq!(
        record["attempts"][0]["rules"],
        json!(["use uv, never .venv"])
    );
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

#[test]
fn every_turn_is_timed_for_the_profile() {
    let f = fixture("profile");
    let fast = Script::new("fast", &[write("answer", "42\n"), done()]);
    fast.replies.lock().unwrap()[0]["usage"] = json!({"prompt_tokens": 900, "completion_tokens": 40,
               "prompt_tokens_details": {"cached_tokens": 800}});
    let record = solve(&f.worlds, &f.project, &task(&[]), &[&fast], 40, None).unwrap();
    let attempts = record["attempts"].as_array().unwrap();

    // Usage feeds the profile and is never sent back to the model.
    let turns = attempts[0]["turns"].as_array().unwrap();
    assert_eq!(turns.len(), 2);
    assert_eq!(turns[0]["tools"][0]["name"], "write_file");
    assert!(turns[0]["tools"][0]["ms"].is_u64());
    assert!(attempts[0]["checks"][0]["ms"].is_u64());
    assert!(
        !attempts[0]["messages"]
            .to_string()
            .contains("prompt_tokens")
    );
    let p = review::profile(&attempts[0]);
    assert_eq!(p["prompt_tokens"], 900);
    assert_eq!(p["cached_tokens"], 800);
    assert_eq!(p["turns"], 2);
    let parts = ["model_ms", "tool_ms", "verify_ms", "harness_ms"]
        .iter()
        .map(|k| p[k].as_u64().unwrap())
        .sum::<u64>();
    assert_eq!(parts, p["wall_ms"].as_u64().unwrap());
}

#[test]
fn a_failed_attempt_teaches_the_next_one() {
    let f = fixture("lesson");
    let wrong = Script::new(
        "wrong",
        // `done` is refused twice while the check fails, then the
        // verification judges.
        &[write("answer", "41\n"), done(), done(), done()],
    );
    let spy = Spy(Mutex::new(String::new()));
    let record = solve(&f.worlds, &f.project, &task(&[]), &[&wrong, &spy], 10, None).unwrap();
    assert_eq!(record["attempts"].as_array().unwrap().len(), 2);
    let seen = spy.0.lock().unwrap();
    assert!(
        seen.contains("Earlier attempts at this task failed."),
        "{seen}"
    );
    assert!(seen.contains("- It failed: verify failed\n  It changed: answer\n"));
    assert!(seen.contains("`sh check.sh` exited 1"));
    // Told in the goal: the system prompt stays the first attempt's.
    let system = seen.split("Write 42").next().unwrap();
    assert!(!system.contains("Earlier attempts"));
}

/// The project folder as the world: the human's default.
fn here(name: &str) -> Fixture {
    let f = fixture(name);
    git(&f, &["config", "--unset", "fennec.workspace"]);
    f
}

#[test]
fn a_rule_said_in_the_folder_does_not_hold_it() {
    let f = here("here-rule");
    std::fs::write(f.project.join("notes"), "mine\n").unwrap();
    rules::say(&f.project, "project", "use uv").unwrap();
    // AGENTS.md alone is committed; the human's own change stays theirs.
    assert_eq!(git(&f, &["status", "--porcelain"]), "?? notes");
    assert_eq!(
        git(&f, &["log", "-1", "--format=%s"]),
        "docs: add a rule for fennec"
    );
    rules::unsay(&f.project, "project", "use uv").unwrap();
    assert_eq!(git(&f, &["status", "--porcelain"]), "?? notes");
    assert!(
        rules::said(&f.project)
            .unwrap()
            .iter()
            .all(|s| s.text != "use uv")
    );
}

#[test]
fn work_in_the_project_folder_is_seen_there_and_lands_on_its_branch() {
    let f = here("here");
    let project = f.project.to_string_lossy().to_string();
    std::fs::write(f.project.join(".gitignore"), ".env\n").unwrap();
    git(&f, &["add", "-A"]);
    git(&f, &["commit", "-qm", "ignore .env"]);
    std::fs::write(f.project.join(".env"), "KEY=1\n").unwrap();
    // A failed attempt is undone by going back: nothing of the human's may be in the way.
    std::fs::write(f.project.join("draft"), "x\n").unwrap();
    let Err(e) = f.worlds.fork(&project) else {
        panic!("forked a dirty folder");
    };
    assert!(e.to_string().contains("uncommitted changes"), "{e}");
    std::fs::remove_file(f.project.join("draft")).unwrap();

    let wrong = Script::new(
        "wrong",
        &[write("answer", "41\n"), write("junk", "j\n"), done()],
    );
    let right = Script::new("right", &[write("answer", "42\n"), done()]);
    let record = solve(
        &f.worlds,
        &f.project,
        &task(&[]),
        &[&wrong, &right],
        10,
        None,
    )
    .unwrap();
    let id = record["winner"].as_str().unwrap().to_string();
    // The work is in the folder; the failed attempt left nothing, the human's file stayed.
    assert_eq!(
        git(&f, &["branch", "--show-current"]),
        format!("world/{id}")
    );
    assert_eq!(
        std::fs::read_to_string(f.project.join("answer")).unwrap(),
        "42\n"
    );
    assert!(!f.project.join("junk").exists());
    assert!(f.project.join(".env").exists());
    // One world at a time, and the agent cannot write the store from inside.
    assert!(f.worlds.fork(&project).is_err());
    let w = f.worlds.open(&id).unwrap();
    assert!(!f.sh(&w, "touch .git/x").0);
    assert!(f.sh(&w, "touch made && rm made").0);

    // The human edits in place; it all lands on their branch, the folder back on it.
    std::fs::write(f.project.join("mine"), "m\n").unwrap();
    land(&f.worlds, &id, None, None).unwrap();
    assert_eq!(git(&f, &["branch", "--show-current"]), "main");
    assert_eq!(git(&f, &["status", "--porcelain"]), "");
    let files = git(&f, &["ls-files"]);
    assert!(
        files.contains("mine") && files.contains("answer"),
        "{files}"
    );
    assert_eq!(git(&f, &["branch", "--list", "world/*"]), "");
    assert!(f.worlds.list().unwrap().is_empty());
}

#[test]
fn a_request_is_framed_solved_and_landed_in_the_project_folder() {
    let f = here("here-spec");
    let good = Script::new(
        "good",
        &[write("spec.sh", "grep -qx 42 answer\n"), finish(SPEC)],
    );
    let s = spec::compile(&f.worlds, &f.project, "answer 42", &good, 10).unwrap();
    assert_eq!(s["status"], "proposed", "{s:#}");
    let id = s["world"].as_str().unwrap().to_string();
    // The tests wait in the folder for the human to validate.
    assert!(f.project.join("spec.sh").exists());
    let honest = Script::new("honest", &[write("answer", "42\n"), done()]);
    let record = spec::accept(&f.worlds, &id, &[&honest], 10, None).unwrap();
    let run = record["winner"].as_str().unwrap().to_string();
    assert_eq!(
        git(&f, &["branch", "--show-current"]),
        format!("world/{run}")
    );
    land(&f.worlds, &run, None, None).unwrap();
    assert_eq!(git(&f, &["branch", "--show-current"]), "main");
    assert!(f.project.join("spec.sh").exists() && f.project.join("answer").exists());
    assert_eq!(git(&f, &["branch", "--list", "world/*"]), "");
    assert_eq!(git(&f, &["log", "--format=%s"]).lines().count(), 4);
}

#[test]
fn a_failed_change_gives_the_folder_back_to_the_ready_work() {
    let f = here("here-amend");
    let good = Script::new(
        "good",
        &[write("spec.sh", "grep -qx 42 answer\n"), finish(SPEC)],
    );
    let s = spec::compile(&f.worlds, &f.project, "answer 42", &good, 10).unwrap();
    let id = s["world"].as_str().unwrap().to_string();
    let honest = Script::new("honest", &[write("answer", "42\n"), done()]);
    let run = spec::accept(&f.worlds, &id, &[&honest], 10, None).unwrap()["winner"]
        .as_str()
        .unwrap()
        .to_string();
    let broken = Script::new("broken", &[write("answer", "41\n"), done()]);
    let failed = spec::amend(&f.worlds, &id, "x", &[&broken], 10, None).unwrap();
    assert!(failed["winner"].is_null());
    // Back on the ready work, not on the human's branch: it still lands.
    assert_eq!(
        git(&f, &["branch", "--show-current"]),
        format!("world/{run}")
    );
    assert_eq!(
        std::fs::read_to_string(f.project.join("answer")).unwrap(),
        "42\n"
    );
    land(&f.worlds, &run, None, None).unwrap();
    assert_eq!(git(&f, &["branch", "--show-current"]), "main");
    // A world whose branch left the folder is never written through.
    let w = f.worlds.fork(&f.project.to_string_lossy()).unwrap();
    git(&f, &["checkout", "-q", "main"]);
    assert!(f.worlds.commit(&w, "x").is_err());
}

#[test]
fn a_change_that_won_gives_the_folder_back_to_the_human_when_dropped() {
    let f = here("here-amend-drop");
    let good = Script::new(
        "good",
        &[write("spec.sh", "grep -qx 42 answer\n"), finish(SPEC)],
    );
    let s = spec::compile(&f.worlds, &f.project, "answer 42", &good, 10).unwrap();
    let id = s["world"].as_str().unwrap().to_string();
    let honest = Script::new("honest", &[write("answer", "42\n"), done()]);
    spec::accept(&f.worlds, &id, &[&honest], 10, None).unwrap();
    let noter = Script::new("noter", &[write("note", "hi\n"), done()]);
    let amended = spec::amend(&f.worlds, &id, "add a note", &[&noter], 10, None).unwrap()["winner"]
        .as_str()
        .unwrap()
        .to_string();
    // The ready work it started from is gone: the folder goes to the human's branch.
    review::reject(&f.worlds, &amended, None).unwrap();
    spec::reject(&f.worlds, &id, None).unwrap();
    assert_eq!(git(&f, &["branch", "--show-current"]), "main");
    assert_eq!(git(&f, &["branch", "--list", "world/*"]), "");
    assert!(!f.project.join("note").exists() && !f.project.join("answer").exists());
}

#[test]
fn a_direct_request_is_solved_from_the_project_and_lands() {
    let f = here("here-direct");
    let s = spec::direct(&f.worlds, &f.project, "answer 42", None, None).unwrap();
    let id = s["world"].as_str().unwrap().to_string();
    let honest = Script::new("honest", &[write("answer", "42\n"), done()]);
    let record = spec::accept(&f.worlds, &id, &[&honest], 10, None).unwrap();
    let run = record["winner"].as_str().unwrap().to_string();
    // No tests first: nothing protected, the attempt forked from the project.
    assert_eq!(record["protect"], json!([]));
    assert!(record["spec"].is_null(), "{record:#}");
    assert_eq!(
        git(&f, &["branch", "--show-current"]),
        format!("world/{run}")
    );
    land(&f.worlds, &run, None, None).unwrap();
    assert_eq!(git(&f, &["branch", "--show-current"]), "main");
    assert_eq!(
        std::fs::read_to_string(f.project.join("answer")).unwrap(),
        "42\n"
    );
}
