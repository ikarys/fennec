//! The intent compiler: a request becomes acceptance tests and a task before
//! any implementation. The human validates the spec, not the code; the
//! harness checks what it can decide: tests were written, they fail now, and
//! they touch nothing the harness or the project rules forbid.

use std::io;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::agent;
use crate::provider::Model;
use crate::review::normalise;
use crate::rules::{self, HARNESS_DIR, Kind, Rule};
use crate::solve::{self, Task};
use crate::world::{World, Worlds};
use crate::{schedule, sleep};

const BRIEF: &str = "Turn the request below into acceptance tests, before anyone implements it. \
Read the project first and follow its test conventions. Write tests only: no implementation, \
no stubs outside test files. Cover the edge cases the request implies; where it is ambiguous, \
pick the reading a careful maintainer would and say so in the goal. \
When finished, call `done` with a summary in exactly this form:\n\
goal: <the request restated precisely for an implementer, naming the tests that must pass>\n\
verify: <a shell command that runs the new tests; one line per command>\n\nRequest:\n";

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
    std::fs::write(spec_path(worlds, id)?, text)
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
    let mut context = String::new();
    let knowhow = sleep::knowhow(&project)?;
    if !knowhow.is_empty() {
        context.push_str("\n\nKnow-how kept for this project:\n");
        knowhow
            .iter()
            .for_each(|k| context.push_str(&format!("  - {k}\n")));
    }
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
    if !corrections.is_empty() {
        context.push_str("\n\nEarlier specs for this request were rejected with:\n");
        corrections
            .iter()
            .for_each(|c| context.push_str(&format!("  - {c}\n")));
    }
    let criteria = [
        "tests only: the request itself stays unimplemented".to_string(),
        "the `verify` commands fail now and pass once the request is implemented".to_string(),
        format!("do not change {HARNESS_DIR}"),
    ];

    let world = worlds.fork(&name)?;
    let mut spec = json!({
        "project": name,
        "request": request,
        "world": world.id,
        "model": model.label(),
        "status": "invalid",
    });
    if let Err(e) = check(
        worlds, &world, &project, request, model, &criteria, &context, max_steps, &mut spec,
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
    request: &str,
    model: &dyn Model,
    criteria: &[String],
    context: &str,
    max_steps: usize,
    spec: &mut Value,
) -> io::Result<()> {
    let base = worlds.git(world, &["rev-parse", "HEAD"])?;
    let outcome = agent::run(
        worlds,
        world,
        model,
        &format!("{BRIEF}{request}"),
        criteria,
        context,
        max_steps,
    )?;
    spec["steps"] = json!(outcome.steps);
    let summary = outcome.summary.unwrap_or_default();
    let (goal, verify) = parse(&summary);
    spec["goal"] = json!(goal);
    spec["verify"] = json!(verify);

    let commit = worlds.commit(world, &format!("fennec-os spec: {}", first_line(request)))?;
    spec["diff"] = json!(worlds.git(world, &["diff", &base, &commit])?);
    let changed = worlds.git(world, &["diff", "--name-only", &base, &commit])?;
    let changed: Vec<&str> = changed.lines().collect();
    spec["protect"] = json!(changed);

    let reason = if goal.is_empty() || verify.is_empty() {
        Some("the summary has no `goal:` or no `verify:` line".to_string())
    } else if changed.is_empty() {
        Some("no test was written".to_string())
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

/// `goal:` starts the goal, which runs until the first `verify:` line.
fn parse(summary: &str) -> (String, Vec<String>) {
    let mut goal = Vec::new();
    let mut verify = Vec::new();
    let mut in_goal = false;
    for line in summary.lines() {
        let trimmed = line.trim();
        if let Some(cmd) = trimmed.strip_prefix("verify:") {
            in_goal = false;
            if !cmd.trim().is_empty() {
                verify.push(cmd.trim().trim_matches('`').to_string());
            }
        } else if let Some(g) = trimmed.strip_prefix("goal:") {
            in_goal = true;
            goal.push(g.trim());
        } else if in_goal {
            goal.push(trimmed);
        }
    }
    (goal.join("\n").trim().to_string(), verify)
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
        grant: Vec::new(),
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
    }
}
