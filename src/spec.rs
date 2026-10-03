//! The intent compiler: a request becomes acceptance tests and a task before
//! any implementation. The human validates the spec, not the code; the
//! harness checks what it can decide: tests were written, they fail now, and
//! they touch nothing the harness or the project rules forbid.

use std::io;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::agent;
use crate::provider::Model;
use crate::review::{self, normalise};
use crate::rules::{self, HARNESS_DIR, Kind, Rule};
use crate::solve::{self, Task};
use crate::world::{World, Worlds};
use crate::{schedule, sleep};

const BRIEF: &str = "Turn the request below into acceptance tests, before anyone implements it. \
Read the project first and follow its test conventions. Write tests only, in new files: they are \
protected whole once accepted, so the implementation must stay free to edit every existing file \
(in Rust, an integration test under `tests/`). No implementation, no stubs. Cover the edge cases the request implies; where it is ambiguous, \
pick the reading a careful maintainer would and say so in the goal. \
When finished, call `done` with a summary in exactly this form:\n\
goal: <the request restated precisely for an implementer, naming the tests that must pass>\n\
verify: <a shell command that runs the new tests; one line per command>\n\nRequest:\n";

const SPAR: &str = "You are a sparring partner for this project: find one real defect, a behaviour \
that contradicts its documentation, its names or the plain intent of its code (invalid input, \
edge cases, overflow, off-by-one, lost errors). Read the code first. Then prove the defect with \
one new test that fails now, in a new file, following the project's test conventions; do not \
change existing files. A style issue or a missing feature is not a defect. If you find \
no real defect, call `done` with the summary `none` and write nothing. Otherwise call `done` with \
a summary in exactly this form:\n\
goal: <the defect and the expected behaviour, for whoever fixes it, naming the failing test>\n\
verify: <a shell command that runs the new test; one line per command>";

fn spec_path(worlds: &Worlds, id: &str) -> io::Result<PathBuf> {
    if id.is_empty() || id.contains('/') {
        return Err(io::Error::other(format!("no spec {id}")));
    }
    Ok(worlds.root.join("specs").join(format!("{id}.json")))
}

pub fn load(worlds: &Worlds, id: &str) -> io::Result<Value> {
    let text = std::fs::read_to_string(spec_path(worlds, id)?)
        .map_err(|_| io::Error::other(format!("no spec {id}")))?;
    serde_json::from_str(&text).map_err(io::Error::other)
}

fn save(worlds: &Worlds, id: &str, spec: &Value) -> io::Result<()> {
    std::fs::create_dir_all(worlds.root.join("specs"))?;
    let text = serde_json::to_string_pretty(spec).map_err(io::Error::other)?;
    review::write_atomic(&spec_path(worlds, id)?, &text)
}

/// Every spec, oldest first.
pub fn specs(worlds: &Worlds) -> io::Result<Vec<(String, Value)>> {
    let Ok(entries) = std::fs::read_dir(worlds.root.join("specs")) else {
        return Ok(Vec::new());
    };
    let mut ids: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            e.file_name()
                .to_str()
                .and_then(|n| n.strip_suffix(".json"))
                .map(str::to_string)
        })
        .collect();
    ids.sort();
    ids.into_iter()
        .map(|id| load(worlds, &id).map(|s| (id, s)))
        .collect()
}

/// Writes the spec in a fresh world and checks it. A valid spec keeps its
/// world (status `proposed`) for the human; an invalid one is dropped.
pub fn compile(
    worlds: &Worlds,
    project: &Path,
    request: &str,
    model: &dyn Model,
    max_steps: usize,
) -> io::Result<Value> {
    let project = std::fs::canonicalize(project)?;
    let _turn = schedule::interactive(worlds)?;
    let name = project.display().to_string();
    // Corrections to earlier specs of the same request.
    let key = normalise(request);
    let corrections: Vec<String> = specs(worlds)?
        .into_iter()
        .filter(|(_, s)| {
            s["project"] == name.as_str()
                && s["request"].as_str().map(normalise) == Some(key.clone())
        })
        .filter_map(|(_, s)| s["correction"].as_str().map(str::to_string))
        .collect();
    let mut context = String::new();
    if !corrections.is_empty() {
        context.push_str("\n\nEarlier specs for this request were rejected with:\n");
        corrections
            .iter()
            .for_each(|c| context.push_str(&format!("  - {c}\n")));
    }
    let brief = format!("{BRIEF}{request}");
    write_spec(
        worlds, &project, &brief, request, "request", &context, model, max_steps,
    )
}

/// Sparring: background work that hunts for one defect and proves it with a
/// failing test. A finding is a spec like any other: its test is a new file,
/// so it cannot fail by breaking what was there.
pub fn spar(
    worlds: &Worlds,
    project: &Path,
    model: &dyn Model,
    max_steps: usize,
) -> io::Result<Value> {
    let project = std::fs::canonicalize(project)?;
    let name = project.display().to_string();
    // Earlier findings, whatever the human decided, are not proposed again.
    let earlier: Vec<String> = specs(worlds)?
        .into_iter()
        // An invalid finding never reached the human: a sound one may come back.
        .filter(|(_, s)| {
            s["project"] == name.as_str() && s["origin"] == "spar" && s["status"] != "invalid"
        })
        .filter_map(|(_, s)| {
            let goal = first_line(s["goal"].as_str()?);
            if goal.is_empty() {
                return None;
            }
            Some(match (s["status"].as_str(), s["correction"].as_str()) {
                (_, Some(c)) => format!("{goal} (rejected: {c})"),
                (Some(status), None) => format!("{goal} ({status})"),
                _ => return None,
            })
        })
        .collect();
    let mut context = String::new();
    if !earlier.is_empty() {
        context.push_str("\n\nEarlier findings on this project; do not report them again:\n");
        // ponytail: the latest 20 only; a long history needs a summary.
        earlier[earlier.len().saturating_sub(20)..]
            .iter()
            .for_each(|f| context.push_str(&format!("  - {f}\n")));
    }
    let model = schedule::Background { worlds, model };
    schedule::yield_to_interactive(worlds)?;
    let mut spec = write_spec(
        worlds, &project, SPAR, "spar", "spar", &context, &model, max_steps,
    )?;
    if let Some(goal) = spec["goal"].as_str().filter(|g| !g.is_empty()) {
        spec["request"] = json!(format!("spar: {}", first_line(goal)));
    } else if spec["summary"]
        .as_str()
        .is_some_and(|s| s.trim().eq_ignore_ascii_case("none"))
    {
        spec["reason"] = json!("no defect found");
    }
    save(worlds, spec["world"].as_str().unwrap_or("spec"), &spec)?;
    Ok(spec)
}

/// An agent writes tests in a fresh world; the harness checks them. A valid
/// spec keeps its world (status `proposed`) for the human; an invalid one is
/// dropped.
#[allow(clippy::too_many_arguments)]
fn write_spec(
    worlds: &Worlds,
    project: &Path,
    brief: &str,
    request: &str,
    origin: &str,
    extra: &str,
    model: &dyn Model,
    max_steps: usize,
) -> io::Result<Value> {
    let name = project.display().to_string();
    let mut context = String::new();
    let knowhow = sleep::knowhow(project)?;
    if !knowhow.is_empty() {
        context.push_str("\n\nKnow-how kept for this project:\n");
        knowhow
            .iter()
            .for_each(|k| context.push_str(&format!("  - {k}\n")));
    }
    context.push_str(extra);
    let criteria = [
        "tests only, in new files: existing files stay untouched".to_string(),
        "the behaviour the tests describe stays unimplemented".to_string(),
        "the `verify` commands fail now and pass once the behaviour is implemented".to_string(),
        format!("do not change {HARNESS_DIR}"),
    ];

    let world = worlds.fork(&name)?;
    let mut spec = json!({
        "project": name,
        "request": request,
        "origin": origin,
        "world": world.id,
        "model": model.label(),
        "status": "invalid",
    });
    if let Err(e) = check(
        worlds, &world, project, brief, model, &criteria, &context, max_steps, &mut spec,
    ) {
        spec["reason"] = json!(e.to_string());
    }
    if spec["status"] != "proposed" {
        worlds.drop(world)?;
    }
    save(worlds, spec["world"].as_str().unwrap_or("spec"), &spec)?;
    Ok(spec)
}

#[allow(clippy::too_many_arguments)]
fn check(
    worlds: &Worlds,
    world: &World,
    project: &Path,
    brief: &str,
    model: &dyn Model,
    criteria: &[String],
    context: &str,
    max_steps: usize,
    spec: &mut Value,
) -> io::Result<()> {
    let base = worlds.git(world, &["rev-parse", "HEAD"])?;
    let outcome = agent::run(worlds, world, model, brief, criteria, context, max_steps)?;
    spec["steps"] = json!(outcome.steps);
    let summary = outcome.summary.clone().unwrap_or_default();
    // Kept so the human sees what the model said when parsing finds nothing.
    spec["summary"] = json!(summary);
    let (mut goal, mut verify) = parse(&summary);
    // `none` is a sparring answer, not a format slip: asking again would
    // invite a made-up finding.
    let none = summary.trim().eq_ignore_ascii_case("none");
    if (goal.is_empty() || verify.is_empty())
        && outcome.summary.is_some()
        && !none
        && let Ok(again) = reformat(model, outcome.messages)
    {
        (goal, verify) = parse(&again);
        spec["reformatted"] = json!(again);
    }
    spec["goal"] = json!(goal);
    spec["verify"] = json!(verify);

    let title = first_line(spec["request"].as_str().unwrap_or("")).to_string();
    let commit = worlds.commit(world, &format!("fennec-os spec: {title}"))?;
    spec["diff"] = json!(worlds.git(world, &["diff", &base, &commit])?);
    let changed = worlds.git(world, &["diff", "--name-only", &base, &commit])?;
    let changed: Vec<&str> = changed.lines().collect();
    spec["protect"] = json!(changed);
    let added = worlds.git(
        world,
        &["diff", "--name-only", "--diff-filter=A", &base, &commit],
    )?;
    let added: Vec<&str> = added.lines().collect();

    let reason = if goal.is_empty() || verify.is_empty() {
        let said: String = summary.chars().take(300).collect();
        Some(format!(
            "the summary has no `goal:` or no `verify:` line; it said: {said}"
        ))
    } else if changed.is_empty() {
        Some("no test was written".to_string())
    } else if let Some(file) = changed.iter().find(|f| !added.contains(f)) {
        // Tests are protected whole; tests mixed into a file the
        // implementation needs would make every attempt fail.
        Some(format!("tests go in new files; {file} already existed"))
    } else {
        // Path rules only: the others judge implementations, not tests.
        let mut forbidden = vec![Rule {
            id: "harness-dir".to_string(),
            kind: Kind::ForbiddenPath(HARNESS_DIR.to_string()),
            block: true,
        }];
        forbidden.extend(
            rules::load(project)?
                .into_iter()
                .filter(|r| r.block && matches!(r.kind, Kind::ForbiddenPath(_))),
        );
        forbidden.iter().find_map(|r| {
            r.violation(&changed)
                .map(|why| format!("rule {} violated: {why}", r.id))
        })
    };
    if let Some(reason) = reason {
        spec["reason"] = json!(reason);
        return Ok(());
    }

    // Tests that already pass do not test the request.
    let commands: Vec<Rule> = verify
        .iter()
        .map(|c| Rule {
            id: "spec.verify".to_string(),
            kind: Kind::StandingCommand(c.clone()),
            block: true,
        })
        .collect();
    let (checks, failure) = solve::verify(worlds, world, &commands)?;
    spec["checks"] = json!(checks);
    if failure.is_none() {
        spec["reason"] = json!("the tests already pass: they do not test the request");
        return Ok(());
    }
    // ponytail: "fails" is any non-zero exit, so a test that does not even
    // compile passes this check; the human reading the diff catches it.
    spec["status"] = json!("proposed");
    Ok(())
}

fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or("")
}

/// Models often end with prose. One more turn, without tools, asks for the
/// form; whatever comes back is still parsed and checked by the harness.
fn reformat(model: &dyn Model, mut messages: Vec<Value>) -> io::Result<String> {
    let done = messages
        .last()
        .and_then(|m| m["tool_calls"].as_array())
        .and_then(|calls| calls.iter().find(|c| c["function"]["name"] == "done"))
        .map(|c| c["id"].clone())
        .unwrap_or(json!("done"));
    messages.push(json!({"role": "tool", "tool_call_id": done,
        "content": "Not accepted: the summary needs `goal:` and `verify:` lines."}));
    messages.push(json!({"role": "user", "content": FORM}));
    let reply = model.chat(&messages, &json!([]))?;
    let content = reply["content"].as_str().unwrap_or("");
    Ok(content
        .rsplit("</think>")
        .next()
        .unwrap_or(content)
        .trim()
        .to_string())
}

const FORM: &str = "Reply with only these lines, nothing else:\n\
goal: <what must be implemented or fixed, precisely, naming the tests that must pass>\n\
verify: <a shell command that runs the new tests; one line per command>";

/// `goal:` starts the goal, which runs until the first `verify:` line. A
/// `verify:` with nothing after it takes the next lines as commands, up to a
/// blank line; code fences are dropped.
fn parse(summary: &str) -> (String, Vec<String>) {
    let mut goal = Vec::new();
    let mut verify = Vec::new();
    let (mut in_goal, mut in_verify) = (false, false);
    for line in summary.lines() {
        let trimmed = line.trim();
        if let Some(cmd) = key(trimmed, "verify") {
            in_goal = false;
            in_verify = cmd.is_empty();
            if !cmd.is_empty() {
                verify.push(cmd.trim_matches('`').to_string());
            }
        } else if let Some(g) = key(trimmed, "goal") {
            (in_goal, in_verify) = (true, false);
            goal.push(g);
        } else if in_verify {
            if trimmed.is_empty() && !verify.is_empty() {
                in_verify = false;
            } else if !trimmed.is_empty() && !trimmed.starts_with("```") {
                verify.push(
                    trimmed
                        .trim_start_matches("$ ")
                        .trim_matches('`')
                        .to_string(),
                );
            }
        } else if in_goal {
            goal.push(trimmed);
        }
    }
    (goal.join("\n").trim().to_string(), verify)
}

/// The value after `name:`, however a model dresses the key up:
/// `goal: x`, `Goal: x`, `**goal:** x`, `- **verify**: x`.
fn key<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let line = line.trim_start_matches(['-', '*', '#', '>', ' ']);
    if !line.get(..name.len())?.eq_ignore_ascii_case(name) {
        return None;
    }
    let rest = line[name.len()..]
        .trim_start_matches('*')
        .strip_prefix(':')?;
    Some(rest.trim_start_matches('*').trim())
}

/// The human validates the spec: its tests become the task's protected
/// checks and the attempts fork from its world. Returns the run record.
pub fn accept(
    worlds: &Worlds,
    id: &str,
    models: &[&dyn Model],
    max_steps: usize,
) -> io::Result<Value> {
    let mut spec = load(worlds, id)?;
    if spec["status"] != "proposed" {
        return Err(io::Error::other(format!("spec {id} is {}", spec["status"])));
    }
    let strings = |k: &str| -> Vec<String> {
        spec[k]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|s| s.as_str().map(str::to_string))
            .collect()
    };
    let task = Task {
        goal: spec["goal"].as_str().unwrap_or("").to_string(),
        verify: strings("verify"),
        protect: strings("protect"),
        base: Some(id.to_string()),
    };
    let project = PathBuf::from(spec["project"].as_str().unwrap_or(""));
    let record = solve::solve(worlds, &project, &task, models, max_steps)?;
    spec["status"] = json!("accepted");
    spec["run"] = record["winner"].clone();
    // The winner carries the tests; without one, the spec stays to retry.
    if record["winner"].is_string() {
        worlds.drop(worlds.open(id)?)?;
    } else {
        spec["status"] = json!("proposed");
    }
    save(worlds, id, &spec)?;
    Ok(record)
}

const ASK: &str = "You wrote the acceptance tests below for the request below. A human reviewing \
them asks you a question. Answer briefly and precisely. You cannot change files here: if a \
change is needed, say what the human should write as a correction.";

/// A question to the compiler about a spec. The answer is advice: it changes
/// nothing; a change goes through `reject` with a correction. Returns it.
pub fn ask(worlds: &Worlds, id: &str, question: &str, model: &dyn Model) -> io::Result<String> {
    let spec = load(worlds, id)?;
    let text = |k: &str| spec[k].as_str().unwrap_or("").to_string();
    let verify: Vec<&str> = spec["verify"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    // ponytail: the diff is cut at 40k chars; huge specs need excerpts.
    let diff: String = text("diff").chars().take(40_000).collect();
    let mut messages = vec![json!({"role": "system", "content": format!(
        "{ASK}\n\nRequest:\n{}\n\ngoal: {}\nverify: {}\n\nTests:\n{diff}",
        text("request"), text("goal"), verify.join("\nverify: ")
    )})];
    for turn in spec["thread"].as_array().into_iter().flatten() {
        messages.push(json!({"role": "user", "content": turn["question"]}));
        messages.push(json!({"role": "assistant", "content": turn["answer"]}));
    }
    messages.push(json!({"role": "user", "content": question}));
    let reply = model.chat(&messages, &json!([]))?;
    let content = reply["content"].as_str().unwrap_or("");
    let answer = content.rsplit("</think>").next().unwrap_or(content).trim();
    // Reloaded: the spec may have been decided while the model answered.
    let mut spec = load(worlds, id)?;
    if !spec["thread"].is_array() {
        spec["thread"] = json!([]);
    }
    if let Some(thread) = spec["thread"].as_array_mut() {
        thread.push(json!({"question": question, "answer": answer}));
    }
    save(worlds, id, &spec)?;
    Ok(answer.to_string())
}

pub fn reject(worlds: &Worlds, id: &str, correction: Option<&str>) -> io::Result<()> {
    let mut spec = load(worlds, id)?;
    if let Ok(world) = worlds.open(id) {
        worlds.drop(world)?;
    }
    spec["status"] = json!("rejected");
    if let Some(c) = correction.filter(|c| !c.trim().is_empty()) {
        spec["correction"] = json!(c);
    }
    save(worlds, id, &spec)
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn summary_parsing() {
        let (goal, verify) = parse(
            "Here you go.\ngoal: add `f`\n  that doubles.\nverify: `cargo test -q`\nverify: sh t.sh\n",
        );
        assert_eq!(goal, "add `f`\nthat doubles.");
        assert_eq!(verify, ["cargo test -q", "sh t.sh"]);

        let (goal, verify) = parse(
            "**Goal:** add `f`.\n\n- **Verify**:\n```sh\n$ cargo test\nsh t.sh\n```\n\nDone.",
        );
        assert_eq!(goal, "add `f`.");
        assert_eq!(verify, ["cargo test", "sh t.sh"]);
    }
}
