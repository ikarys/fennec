//! A task end to end: N worlds forked from the project, one agent in each,
//! a verdict read from exit codes, the smallest passing diff kept.

use std::io;
use std::path::Path;
use std::process::Stdio;

use serde_json::{Value, json};

use crate::agent;
use crate::provider::Model;
use crate::review;
use crate::rules::{self, HARNESS_DIR, Kind, Rule};
use crate::world::{World, Worlds, git};
use crate::{recipe, schedule, sleep};

const VERIFY_TIMEOUT_SECS: u32 = 900;

pub struct Task {
    pub goal: String,
    /// Shell commands run in the world after the agent; all must exit 0.
    pub verify: Vec<String>,
    /// Paths the agent must not change, so it cannot pass by editing the checks.
    pub protect: Vec<String>,
}

impl Task {
    /// `{"goal": "...", "verify": ["cmd", ...], "protect": ["path", ...]}`
    pub fn from_json(text: &str) -> io::Result<Self> {
        let v: Value = serde_json::from_str(text).map_err(io::Error::other)?;
        let strings = |k: &str| -> Vec<String> {
            v[k].as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|s| s.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default()
        };
        let task = Self {
            goal: v["goal"].as_str().unwrap_or("").to_string(),
            verify: strings("verify"),
            protect: strings("protect"),
        };
        if task.goal.is_empty() {
            return Err(io::Error::other("task has no goal"));
        }
        // Without a check there is no verdict, and the model never judges.
        if task.verify.is_empty() {
            return Err(io::Error::other("task has no verify command"));
        }
        Ok(task)
    }
}

/// Runs one attempt per model, in parallel. Keeps the winner's world (if any)
/// for `land`, drops the others, and returns the run record, also saved
/// under `<root>/runs/`.
pub fn solve(
    worlds: &Worlds,
    project: &Path,
    task: &Task,
    models: &[&dyn Model],
    max_steps: usize,
) -> io::Result<Value> {
    let project = std::fs::canonicalize(project)?;
    let _turn = schedule::interactive(worlds)?;
    // Everything an attempt is judged on, as rules: the harness files
    // (a run must not edit what judges it), the task's own checks, then the
    // project's rules.
    let mut all_rules = vec![Rule {
        id: "harness-dir".to_string(),
        kind: Kind::ForbiddenPath(HARNESS_DIR.to_string()),
        block: true,
    }];
    all_rules.extend(task.protect.iter().map(|p| Rule {
        id: "task.protect".to_string(),
        kind: Kind::ForbiddenPath(p.clone()),
        block: true,
    }));
    all_rules.extend(task.verify.iter().map(|c| Rule {
        id: "task.verify".to_string(),
        kind: Kind::StandingCommand(c.clone()),
        block: true,
    }));
    all_rules.extend(rules::load(&project)?);
    let rules = &all_rules;
    let mut context = String::new();
    let knowhow = sleep::knowhow(&project)?;
    if !knowhow.is_empty() {
        context.push_str("\n\nKnow-how kept for this project:\n");
        knowhow
            .iter()
            .for_each(|k| context.push_str(&format!("  - {k}\n")));
    }
    let history = review::agenda(worlds, &project.display().to_string(), 12)?;
    if !history.is_empty() {
        context.push_str("\n\nThis project's history, from earlier runs, oldest first:\n");
        history
            .iter()
            .for_each(|h| context.push_str(&format!("  - {h}\n")));
    }
    let brief = &Brief {
        task,
        rules,
        context: &context,
        max_steps,
    };
    // A recorded way to this goal is tried first, alone: if it passes, no
    // model is called.
    let (mut forked, mut attempts) = (Vec::new(), Vec::new());
    if let Some(recipe) = recipe::find(worlds, &project, &task.goal)? {
        let (f, a) = attempts_for(worlds, &project, &[&recipe], brief)?;
        forked = f;
        attempts = a;
    }
    if !attempts.iter().any(|a| a["passed"] == true) {
        let (f, a) = attempts_for(worlds, &project, models, brief)?;
        forked.extend(f);
        attempts.extend(a);
    }

    // Selection by the verifier: among passing worlds, the smallest change.
    let winner = attempts
        .iter()
        .filter(|a| a["passed"] == true)
        .min_by_key(|a| a["changed_lines"].as_u64().unwrap_or(u64::MAX))
        .and_then(|a| a["world"].as_str())
        .map(str::to_string);
    for (world, record) in forked.into_iter().zip(&mut attempts) {
        if Some(&world.id) == winner.as_ref() {
            let commit = record["commit"].as_str().unwrap_or("");
            let marker = worlds.root.join("passed");
            std::fs::create_dir_all(&marker)?;
            let verdict = json!({
                "commit": commit,
                "project": project.display().to_string(),
                "verify": blocking_commands(rules),
            });
            std::fs::write(marker.join(&world.id), verdict.to_string())?;
        } else {
            worlds.drop(world)?;
        }
    }

    let record = json!({
        "project": project.display().to_string(),
        "goal": task.goal,
        "verify": task.verify,
        "protect": task.protect,
        "winner": winner,
        "status": if winner.is_some() { "ready" } else { "failed" },
        "attempts": attempts,
    });
    let runs = worlds.root.join("runs");
    std::fs::create_dir_all(&runs)?;
    let name = winner.clone().unwrap_or_else(|| {
        record["attempts"][0]["world"]
            .as_str()
            .unwrap_or("run")
            .to_string()
    });
    std::fs::write(
        runs.join(format!("{name}.json")),
        serde_json::to_string_pretty(&record).map_err(io::Error::other)?,
    )?;
    Ok(record)
}

/// One attempt per model, in parallel, each in a fresh world.
fn attempts_for(
    worlds: &Worlds,
    project: &Path,
    models: &[&dyn Model],
    brief: &Brief,
) -> io::Result<(Vec<World>, Vec<Value>)> {
    // Forks are sequential: concurrent `git worktree add` contend for locks.
    let forked: Vec<World> = models
        .iter()
        .map(|_| worlds.fork(&project.to_string_lossy()))
        .collect::<io::Result<_>>()?;
    let attempts = std::thread::scope(|s| {
        let handles: Vec<_> = forked
            .iter()
            .zip(models)
            .map(|(world, model)| s.spawn(move || attempt(worlds, world, *model, brief)))
            .collect();
        handles
            .into_iter()
            .zip(&forked)
            .map(|(h, world)| match h.join() {
                Ok(record) => record,
                Err(_) => json!({"world": world.id, "passed": false, "reason": "attempt panicked"}),
            })
            .collect()
    });
    Ok((forked, attempts))
}

/// What every attempt of a run is given.
struct Brief<'a> {
    task: &'a Task,
    rules: &'a [Rule],
    /// Know-how and history, appended to the system prompt.
    context: &'a str,
    max_steps: usize,
}

/// Never fails: any error becomes a failed attempt in the record.
fn attempt(worlds: &Worlds, world: &World, model: &dyn Model, brief: &Brief) -> Value {
    let mut record = json!({"world": world.id, "model": model.label(), "passed": false});
    if let Err(e) = attempt_inner(worlds, world, model, brief, &mut record) {
        record["reason"] = json!(e.to_string());
    }
    record
}

fn attempt_inner(
    worlds: &Worlds,
    world: &World,
    model: &dyn Model,
    brief: &Brief,
    record: &mut Value,
) -> io::Result<()> {
    let Brief {
        task,
        rules,
        context,
        max_steps,
    } = *brief;
    let base = worlds.git(world, &["rev-parse", "HEAD"])?;
    let criteria: Vec<String> = rules
        .iter()
        .filter(|r| r.block)
        .map(Rule::describe)
        .collect();
    let outcome = agent::run(
        worlds, world, model, &task.goal, &criteria, context, max_steps,
    )?;
    record["steps"] = json!(outcome.steps);
    record["summary"] = json!(outcome.summary);
    record["messages"] = json!(outcome.messages);

    let title = task.goal.lines().next().unwrap_or("task");
    let commit = worlds.commit(world, &format!("fennec-os: {title}"))?;
    let diff = worlds.git(world, &["diff", &base, &commit])?;
    record["commit"] = json!(commit);
    record["changed_lines"] = json!(
        diff.lines()
            .filter(|l| (l.starts_with('+') || l.starts_with('-'))
                && !l.starts_with("+++")
                && !l.starts_with("---"))
            .count()
    );
    record["diff"] = json!(diff);

    let changed = worlds.git(world, &["diff", "--name-only", &base, &commit])?;
    let changed: Vec<&str> = changed.lines().collect();
    let mut violations = Vec::new();
    let mut blocked = None;
    for rule in rules {
        if let Some(why) = rule.violation(&changed) {
            violations.push(json!({"rule": rule.id, "block": rule.block, "violation": why}));
            if rule.block && blocked.is_none() {
                blocked = Some(format!("rule {} violated: {why}", rule.id));
            }
        }
    }
    record["violations"] = json!(violations);
    if let Some(reason) = blocked {
        record["reason"] = json!(reason);
        return Ok(());
    }

    let (checks, failure) = verify(worlds, world, rules)?;
    record["checks"] = json!(checks);
    if let Some(reason) = failure {
        record["reason"] = json!(reason);
        return Ok(());
    }
    record["passed"] = json!(true);
    Ok(())
}

fn blocking_commands(rules: &[Rule]) -> Vec<&str> {
    rules
        .iter()
        .filter_map(|r| match &r.kind {
            Kind::StandingCommand(c) if r.block => Some(c.as_str()),
            _ => None,
        })
        .collect()
}

/// Runs the rules' commands in the world's sandbox. Returns their results
/// and, if a blocking one failed, the reason.
fn verify(
    worlds: &Worlds,
    world: &World,
    rules: &[Rule],
) -> io::Result<(Vec<Value>, Option<&'static str>)> {
    let mut checks = Vec::new();
    let mut all_pass = true;
    for rule in rules {
        let Kind::StandingCommand(command) = &rule.kind else {
            continue;
        };
        let argv = [
            "timeout".to_string(),
            "-k".to_string(),
            "5".to_string(),
            VERIFY_TIMEOUT_SECS.to_string(),
            "sh".to_string(),
            "-c".to_string(),
            command.clone(),
        ];
        let out = worlds
            .sandbox_command(world, &argv)?
            .stdin(Stdio::null())
            .output()?;
        let code = out.status.code().unwrap_or(-1);
        all_pass &= code == 0 || !rule.block;
        let output = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        let lines: Vec<&str> = output.lines().collect();
        let tail = lines[lines.len().saturating_sub(40)..].join("\n");
        checks.push(json!({"rule": rule.id, "block": rule.block, "command": command, "exit": code, "output": tail}));
    }
    // What lands must be exactly what was checked.
    if !worlds.git(world, &["status", "--porcelain"])?.is_empty() {
        return Ok((checks, Some("verify changed the world")));
    }
    Ok((checks, (!all_pass).then_some("verify failed")))
}

/// Lands a world that passed onto the project, then drops the world. If the
/// project moved since the fork, the world is rebased and verified again:
/// a rebased tree is a new tree. Refuses a world changed since its verdict.
pub fn land(worlds: &Worlds, id: &str) -> io::Result<String> {
    let world = worlds.open(id)?;
    let marker = worlds.root.join("passed").join(id);
    let text = std::fs::read_to_string(&marker)
        .map_err(|_| io::Error::other(format!("world {id} has no passing verdict")))?;
    let verdict: Value = serde_json::from_str(&text).map_err(io::Error::other)?;
    let (Some(mut commit), Some(project)) = (
        verdict["commit"].as_str().map(str::to_string),
        verdict["project"].as_str().map(Path::new),
    ) else {
        return Err(io::Error::other(format!(
            "bad verdict file {}",
            marker.display()
        )));
    };
    if worlds.git(&world, &["rev-parse", "HEAD"])? != commit
        || !worlds.git(&world, &["status", "--porcelain"])?.is_empty()
    {
        return Err(io::Error::other(format!(
            "world {id} changed since it passed"
        )));
    }
    let head = git(project, &["rev-parse", "HEAD"])?;
    if git(project, &["merge-base", "--is-ancestor", &head, &commit]).is_err() {
        // The verdict no longer holds for any tree but the old one.
        std::fs::remove_file(&marker)?;
        commit = worlds.rebase(&world, &head).map_err(|e| {
            io::Error::other(format!(
                "{e}\nworld {id} conflicts with the project; solve again"
            ))
        })?;
        let commands: Vec<Rule> = verdict["verify"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|c| c.as_str())
            .map(|c| Rule {
                id: "verdict".to_string(),
                kind: Kind::StandingCommand(c.to_string()),
                block: true,
            })
            .collect();
        let (checks, failure) = verify(worlds, &world, &commands)?;
        if let Some(reason) = failure {
            review::set_status(worlds, id, "failed")?;
            return Err(io::Error::other(format!(
                "world {id} {reason} after rebase onto {head}: {}",
                serde_json::to_string_pretty(&checks).unwrap_or_default()
            )));
        }
    }
    git(project, &["merge", "-q", "--ff-only", &commit])?;
    let _ = std::fs::remove_file(&marker);
    worlds.drop(world)?;
    review::set_status(worlds, id, "landed")?;
    Ok(commit)
}
