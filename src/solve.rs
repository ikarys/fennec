//! A task end to end: up to N worlds forked from the project, one agent in
//! each, tried in turn until one passes; the verdict is read from exit codes.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Instant;

use serde_json::{Value, json};

use crate::agent;
use crate::grant;
use crate::provider::Model;
use crate::review;
use crate::rules::{self, HARNESS_DIR, Kind, Rule};
use crate::team;
use crate::world::{World, Worlds, git};
use crate::{recipe, schedule, skill, sleep};

const VERIFY_TIMEOUT_SECS: u32 = 900;

pub struct Task {
    pub goal: String,
    /// Shell commands run in the world after the agent; all must exit 0.
    pub verify: Vec<String>,
    /// Paths the agent must not change, so it cannot pass by editing the checks.
    pub protect: Vec<String>,
    /// The world attempts fork from (an accepted spec); `None` is the
    /// project's HEAD. Set by the harness, never read from a task file.
    pub base: Option<String>,
    /// Host paths the human lets the one attempt write, backed up first.
    /// Set by the harness from the command line, never read from a task file.
    pub grant: Vec<PathBuf>,
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
            base: None,
            grant: Vec::new(),
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

/// Runs one attempt per model in turn, until one passes. Keeps its world
/// for `land`, drops the others, and returns the run record, also saved
/// under `<root>/runs/`.
pub fn solve(
    worlds: &Worlds,
    project: &Path,
    task: &Task,
    models: &[&dyn Model],
    max_steps: usize,
) -> io::Result<Value> {
    // Parallel attempts would write the same host paths.
    if !task.grant.is_empty() && models.len() != 1 {
        return Err(io::Error::other("a grant needs exactly one attempt"));
    }
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
    let said = rules::said(&project)?;
    context.push_str(&rules::brief(&said));
    let (roles, team) = team::brief(&project, "solve")?;
    context.push_str(&roles);
    let skills = skill::chosen(Some(&project))?;
    context.push_str(&skill::brief(&skills));
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
        said: &said,
        context: &context,
        skills: &skills,
        max_steps,
    };
    let source = match &task.base {
        Some(world) => world.clone(),
        None => project.to_string_lossy().to_string(),
    };
    // A recorded way to this goal is tried first, alone: if it passes, no
    // model is called. Never with a grant: a replay must not write host paths.
    let (mut forked, mut attempts) = (Vec::new(), Vec::new());
    let recipe = match task.grant.is_empty() {
        true => recipe::find(worlds, &project, &task.goal)?,
        false => None,
    };
    if let Some(recipe) = recipe {
        let (f, a) = attempts_for(worlds, &source, &[&recipe], brief)?;
        forked = f;
        attempts = a;
    }
    if !attempts.iter().any(|a| a["passed"] == true) {
        let (f, a) = attempts_for(worlds, &source, models, brief)?;
        forked.extend(f);
        attempts.extend(a);
    }

    // The verifier chose: attempts stop at the first pass.
    let winner = attempts
        .iter()
        .find(|a| a["passed"] == true)
        .and_then(|a| a["world"].as_str())
        .map(str::to_string);
    // Only the winner is left (attempts stop at the first pass); its grant
    // stays until the human keeps or reverts it.
    for world in forked {
        if Some(&world.id) == winner.as_ref() {
            let commit = attempts
                .iter()
                .find(|a| a["world"] == world.id.as_str())
                .and_then(|a| a["commit"].as_str())
                .unwrap_or("");
            let marker = worlds.root.join("passed");
            std::fs::create_dir_all(&marker)?;
            let verdict = json!({
                "commit": commit,
                "project": project.display().to_string(),
                "verify": blocking_commands(rules),
            });
            std::fs::write(marker.join(&world.id), verdict.to_string())?;
        }
    }

    let mut record = json!({
        "project": project.display().to_string(),
        "goal": task.goal,
        "spec": task.base,
        "verify": task.verify,
        "protect": task.protect,
        "grant": task.grant,
        "team": team,
        "winner": winner,
        "status": if winner.is_some() { "ready" } else { "failed" },
        "attempts": attempts,
    });
    std::fs::create_dir_all(worlds.root.join("runs"))?;
    let name = winner.clone().unwrap_or_else(|| {
        record["attempts"][0]["world"]
            .as_str()
            .unwrap_or("run")
            .to_string()
    });
    // The name it is kept under, for whoever shows it later.
    record["id"] = json!(name);
    review::save(worlds, &name, &record)?;
    Ok(record)
}

/// One attempt per model, one after the other, each in a fresh world forked
/// when it starts; the first pass ends the run. On a server that decodes
/// speculatively, attempts running at once split its speed (measured: 165
/// tok/s alone, 90 each for two), so a later attempt runs only if the
/// earlier ones failed.
// ponytail: sequential only; a server that batches for free would want
// parallel attempts back, behind a flag, once one is in use.
fn attempts_for(
    worlds: &Worlds,
    source: &str,
    models: &[&dyn Model],
    brief: &Brief,
) -> io::Result<(Vec<World>, Vec<Value>)> {
    let (mut forked, mut attempts) = (Vec::new(), Vec::new());
    let mut lessons = String::new();
    for model in models {
        let world = worlds.fork(source)?;
        skill::give(worlds, &world, brief.skills)?;
        // One world: `solve` refuses a grant with several attempts.
        if !brief.task.grant.is_empty()
            && let Err(e) = grant::give(worlds, &world, &brief.task.grant)
        {
            worlds.drop(world)?;
            return Err(e);
        }
        let record = attempt(worlds, &world, *model, brief, &lessons);
        let passed = record["passed"] == true;
        if !passed {
            if lessons.is_empty() {
                lessons.push_str(
                    "Earlier attempts at this task failed. You start again from a fresh \
                     copy of the project: their changes are gone, what they learnt is not.\n",
                );
            }
            lessons.push_str(&lesson(&record));
            // Gone before the next attempt forks: in the project folder, the
            // next one needs the folder back. A failed attempt leaves the
            // host as it found it.
            if !grant::paths(worlds, &world.id)?.is_empty() {
                grant::revert(worlds, &world)?;
            }
            worlds.drop(world)?;
        } else {
            forked.push(world);
        }
        attempts.push(record);
        if passed {
            break;
        }
    }
    Ok((forked, attempts))
}

/// What a failed attempt teaches the next one, from the harness's own
/// findings, not the agent's account: why it failed, what it touched, what
/// the failing checks printed.
fn lesson(record: &Value) -> String {
    let mut text = format!(
        "- It failed: {}\n",
        record["reason"].as_str().unwrap_or("no reason recorded")
    );
    let files: Vec<&str> = record["diff"]
        .as_str()
        .unwrap_or("")
        .lines()
        .filter_map(|l| l.strip_prefix("+++ b/"))
        .collect();
    if !files.is_empty() {
        text.push_str(&format!("  It changed: {}\n", files.join(", ")));
    }
    for c in record["checks"].as_array().into_iter().flatten() {
        if c["block"] != true || c["exit"] == 0 {
            continue;
        }
        let output = c["output"].as_str().unwrap_or("");
        let lines: Vec<&str> = output.lines().collect();
        let tail = lines[lines.len().saturating_sub(20)..].join("\n    ");
        text.push_str(&format!(
            "  `{}` exited {}; end of its output:\n    {tail}\n",
            c["command"].as_str().unwrap_or(""),
            c["exit"]
        ));
    }
    // ponytail: cut at 3000 chars per attempt; a sidekick could summarise.
    text.chars().take(3000).collect()
}

/// What every attempt of a run is given.
struct Brief<'a> {
    task: &'a Task,
    rules: &'a [Rule],
    /// Rules said in words, numbered in `context` as given.
    said: &'a [rules::Said],
    /// Know-how, rules and history, appended to the system prompt.
    context: &'a str,
    /// Mounted in each attempt's sandbox; listed in `context`.
    skills: &'a [skill::Skill],
    max_steps: usize,
}

/// Never fails: any error becomes a failed attempt in the record.
/// `lessons` is what earlier failed attempts of the run teach; empty for the
/// first.
fn attempt(
    worlds: &Worlds,
    world: &World,
    model: &dyn Model,
    brief: &Brief,
    lessons: &str,
) -> Value {
    let started = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);
    let clock = Instant::now();
    let mut record = json!({"world": world.id, "model": model.label(), "passed": false,
                            "started_ms": started});
    if let Err(e) = attempt_inner(worlds, world, model, brief, lessons, &mut record) {
        record["reason"] = json!(e.to_string());
    }
    record["ms"] = json!(clock.elapsed().as_millis() as u64);
    record
}

fn attempt_inner(
    worlds: &Worlds,
    world: &World,
    model: &dyn Model,
    brief: &Brief,
    lessons: &str,
    record: &mut Value,
) -> io::Result<()> {
    let Brief {
        task,
        rules,
        said,
        context,
        max_steps,
        ..
    } = *brief;
    let mut context = context.to_string();
    let granted = grant::paths(worlds, &world.id)?;
    if !granted.is_empty() {
        context.push_str("\n\nHost paths you may edit with absolute paths (backed up; the human reviews the diff):\n");
        granted
            .iter()
            .for_each(|p| context.push_str(&format!("  - {}\n", p.display())));
        record["grant"] = json!(granted);
    }
    let base = worlds.git(world, &["rev-parse", "HEAD"])?;
    let criteria: Vec<String> = rules
        .iter()
        .filter(|r| r.block)
        .map(Rule::describe)
        .collect();
    // In the goal message, not the system prompt: the system prompt stays
    // the same across attempts and keeps the server's prefix cache.
    let goal = match lessons {
        "" => task.goal.clone(),
        _ => format!("{}\n\n{lessons}", task.goal),
    };
    let outcome = agent::run(worlds, world, model, &goal, &criteria, &context, max_steps)?;
    record["steps"] = json!(outcome.steps);
    record["rules"] = json!(rules::used(outcome.summary.as_deref().unwrap_or(""), said));
    record["summary"] = json!(outcome.summary.as_deref().map(rules::shown));
    record["skills"] = json!(skill::used(&outcome.messages));
    record["messages"] = json!(outcome.messages);
    record["stuck"] = json!(outcome.stuck);
    record["question"] = json!(outcome.question);
    record["turns"] = json!(outcome.turns);
    if let Some(e) = outcome.error {
        return Err(io::Error::other(e));
    }

    let title: String = task
        .goal
        .lines()
        .next()
        .unwrap_or("task")
        .chars()
        .take(72)
        .collect();
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
        // A stopped agent's failure is explained by why it was stopped. Work
        // that passes anyway is kept: the verdict is the checks, not the stop.
        record["reason"] = json!(match &outcome.stuck {
            Some(why) => format!("{why}; then {reason}"),
            None => reason.to_string(),
        });
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
pub(crate) fn verify(
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
            // Models write bash (`<(...)`, `[[`): under dash a right product
            // failed its proof (UX test 2026-10-18). ponytail: bash assumed
            // on the machine; `sh` where it is missing.
            "bash".to_string(),
            "-c".to_string(),
            command.clone(),
        ];
        let ran = Instant::now();
        let out = worlds
            .sandbox_command(world, &argv)?
            .stdin(Stdio::null())
            .output()?;
        let ms = ran.elapsed().as_millis() as u64;
        let code = out.status.code().unwrap_or(-1);
        all_pass &= code == 0 || !rule.block;
        let output = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        let lines: Vec<&str> = output.lines().collect();
        let tail = lines[lines.len().saturating_sub(40)..].join("\n");
        checks.push(json!({"rule": rule.id, "block": rule.block, "command": command, "exit": code, "ms": ms, "output": tail}));
    }
    // What lands must be exactly what was checked.
    if !worlds.status(world)?.is_empty() {
        return Ok((checks, Some("verify changed the world")));
    }
    Ok((checks, (!all_pass).then_some("verify failed")))
}

const PLAN: &str = "Decide how the change below lands in this project, the way this project \
does it: follow its conventions and rules (branch naming, protected branches, commit style) and \
the style of its recent subjects. First line: `branch: <name>`, the branch the commit goes on; \
`branch: current` when the rules say nothing about branches. Then a blank line, then the commit \
message. The message covers the whole Change: when the goal ends with a change the human \
asked for on a first solution, that change is one part of it, not the subject. No fences, no \
commentary.";

/// Where and how a run lands: shown to the human before they click.
pub struct Landing {
    /// The branch the commit goes on; the project's current one by default.
    pub branch: String,
    /// `None` when the model gives nothing usable: the goal's first line.
    pub message: Option<String>,
    /// The remote the branch is pushed to; `None` when the project has none.
    pub remote: Option<String>,
}

/// The branch and commit message for landing run `id`, written by the model
/// from the project's conventions. The harness only checks the branch is a
/// valid name; whether it is the right one is the human's call on screen.
pub fn plan(worlds: &Worlds, id: &str, model: &dyn Model) -> io::Result<Landing> {
    let record = review::load(worlds, id)?;
    let project = Path::new(record["project"].as_str().unwrap_or(""));
    let current = current_branch(worlds, id, project)?;
    let remotes = git(project, &["remote"])?;
    let remote = remotes
        .lines()
        .find(|r| *r == "origin")
        .or_else(|| remotes.lines().next())
        .map(str::to_string);
    // Everything the commit will carry, from where it left the human's
    // branch: an amended run's own diff holds only the correction, and the
    // message then named that alone (UX test 2026-10-18).
    let passed = std::fs::read_to_string(worlds.root.join("passed").join(id))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v["commit"].as_str().map(str::to_string));
    let whole =
        passed.and_then(|c| git(project, &["diff", &format!("{current}...{c}"), "--"]).ok());
    let diff: String = whole
        .as_deref()
        .or_else(|| {
            record["attempts"]
                .as_array()
                .and_then(|a| a.iter().find(|t| t["world"] == id))
                .and_then(|t| t["diff"].as_str())
        })
        .unwrap_or("")
        .chars()
        .take(8000)
        .collect();
    let agents: String = std::fs::read_to_string(project.join("AGENTS.md"))
        .unwrap_or_default()
        .chars()
        .take(6000)
        .collect();
    let said: Vec<String> = rules::said(project)?.into_iter().map(|s| s.text).collect();
    // From the human's branch: in the folder itself, HEAD is the world's
    // checkpoint ("fennec-os: …"), whose style the model then copied.
    let recent = git(project, &["log", "-10", "--format=%s", &current, "--"]).unwrap_or_default();
    // A world's branch is fennec's scaffolding, never a place to land: shown
    // to the model, it was picked as the target (UX test 2026-10-18).
    let branches: String = git(project, &["branch", "--format=%(refname:short)"])
        .unwrap_or_default()
        .lines()
        .filter(|b| !b.starts_with("world/"))
        .map(|b| format!("{b}\n"))
        .collect();
    let request = format!(
        "Project conventions (AGENTS.md):\n{agents}\n\nRules:\n{}\n\nCurrent branch: {current}\nBranches:\n{branches}\n\nRecent subjects:\n{recent}\n\nGoal:\n{}\n\nChange:\n{diff}",
        said.join("\n"),
        record["goal"].as_str().unwrap_or("")
    );
    let messages = [
        json!({"role": "system", "content": PLAN}),
        json!({"role": "user", "content": request}),
    ];
    let reply = model.chat(&messages, &json!([]))?;
    let content = reply["content"].as_str().unwrap_or("");
    let (branch, message) = parse_plan(content.rsplit("</think>").next().unwrap_or(content));
    let branch = branch
        .filter(|b| !b.starts_with("world/"))
        .filter(|b| git(project, &["check-ref-format", "--branch", b]).is_ok())
        .unwrap_or(current);
    Ok(Landing {
        branch,
        message,
        remote,
    })
}

fn parse_plan(reply: &str) -> (Option<String>, Option<String>) {
    let text = reply
        .trim()
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    let (first, rest) = text.split_once('\n').unwrap_or((text, ""));
    let Some(branch) = first.trim().strip_prefix("branch:") else {
        return (None, (!text.is_empty()).then(|| text.to_string()));
    };
    let branch = branch.trim().trim_matches('`');
    let branch = (!branch.is_empty() && branch != "current").then(|| branch.to_string());
    let message = rest.trim();
    (branch, (!message.is_empty()).then(|| message.to_string()))
}

/// The project's branch: for work in the project folder, the one the folder
/// was on before, not the world's.
fn current_branch(worlds: &Worlds, id: &str, project: &Path) -> io::Result<String> {
    match worlds.open(id).and_then(|w| worlds.home(&w)) {
        Ok(Some(home)) => Ok(home),
        _ => git(project, &["branch", "--show-current"]),
    }
}

/// Pushes `branch` to `remote` with the human's own credentials. Never
/// prompts: a push that needs a password fails with git's reason instead
/// of hanging in the background.
pub fn push(project: &Path, remote: &str, branch: &str) -> io::Result<()> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(project)
        .args(["-c", "core.hooksPath=/dev/null", "push", "--quiet", remote])
        .arg(format!("refs/heads/{branch}:refs/heads/{branch}"))
        // As every harness git call: never a parent folder's repository.
        .env(
            "GIT_CEILING_DIRECTORIES",
            project.parent().unwrap_or(Path::new("")),
        )
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes")
        .output()?;
    if !out.status.success() {
        return Err(io::Error::other(format!(
            "git push {remote} {branch}: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(())
}

/// Lands a world that passed onto the project as one commit, then drops the
/// world. If the project moved since the fork, the world is rebased and
/// verified again: a rebased tree is a new tree. A world the human edited
/// since its verdict is verified again too, and lands with their edits. The commit is the human's: the project's own git
/// identity, `message` or else the goal's first line; the harness's
/// checkpoints stay behind. On `branch` other than the current one, the
/// commit goes on a new branch and the human's worktree is left alone.
pub fn land(
    worlds: &Worlds,
    id: &str,
    branch: Option<&str>,
    message: Option<&str>,
) -> io::Result<String> {
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
    // The human may have edited the work in their editor: it lands too, once
    // its checks pass again on the edited tree.
    let edited =
        worlds.git(&world, &["rev-parse", "HEAD"])? != commit || !worlds.status(&world)?.is_empty();
    if edited {
        let now = worlds.commit(&world, "human edits")?;
        if worlds
            .git(&world, &["merge-base", "--is-ancestor", &commit, &now])
            .is_err()
        {
            return Err(io::Error::other(format!(
                "world {id} changed since it passed and no longer holds its work"
            )));
        }
        commit = now;
    }
    let home = worlds.home(&world)?;
    let current = current_branch(worlds, id, project)?;
    let head = git(project, &["rev-parse", &format!("refs/heads/{current}")])?;
    let moved = git(project, &["merge-base", "--is-ancestor", &head, &commit]).is_err();
    if moved {
        // The verdict no longer holds for any tree but the old one. Edits
        // alone keep it: the human may fix what broke the checks and retry.
        std::fs::remove_file(&marker)?;
        commit = worlds.rebase(&world, &head).map_err(|e| {
            io::Error::other(format!(
                "{e}\nworld {id} conflicts with the project; solve again"
            ))
        })?;
    }
    if moved || edited {
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
            if moved {
                review::set_status(worlds, id, "failed")?;
            }
            let after = if moved { "after rebase" } else { "after edits" };
            return Err(io::Error::other(format!(
                "world {id} {reason} {after} onto {head}: {}",
                serde_json::to_string_pretty(&checks).unwrap_or_default()
            )));
        }
    }
    let goal = review::load(worlds, id)?["goal"]
        .as_str()
        .unwrap_or("")
        .lines()
        .next()
        .unwrap_or("")
        .to_string();
    let message = message
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .unwrap_or(&goal);
    // commit-tree, not commit: no hook runs, the project's worktree is not
    // touched until the fast-forward below.
    let squashed = git(
        project,
        &[
            "commit-tree",
            &format!("{commit}^{{tree}}"),
            "-p",
            &head,
            "-m",
            message,
        ],
    )?;
    match branch.filter(|b| *b != current) {
        // The folder holds the world: its files are the commit's already, so
        // the branch moves to it and the folder goes back on that branch.
        None if home.is_some() => {
            let target = format!("refs/heads/{current}");
            git(project, &["update-ref", &target, &squashed, &head])?;
            git(project, &["symbolic-ref", "HEAD", &target])?;
        }
        None => {
            git(project, &["merge", "-q", "--ff-only", &squashed])?;
        }
        // `git branch` without -f refuses an existing branch: the commit was
        // verified on the current branch's tree, not on that one.
        Some(b) => {
            git(project, &["branch", "--", b, &squashed])?;
        }
    }
    let commit = squashed;
    let _ = std::fs::remove_file(&marker);
    worlds.drop(world)?;
    // The spec's world is kept for a retry until its run lands.
    if let Some(spec) = review::load(worlds, id)?["spec"].as_str()
        && let Ok(tests) = worlds.open(spec)
    {
        worlds.drop(tests)?;
    }
    review::set_status(worlds, id, "landed")?;
    Ok(commit)
}
