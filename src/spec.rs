//! The intent compiler: a request becomes acceptance tests and a task before
//! any implementation. The human validates the spec, not the code; the
//! harness checks what it can decide: tests were written, they fail now, and
//! they touch nothing the harness or the project rules forbid.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::agent;
use crate::provider::Model;
use crate::review::{self, normalise};
use crate::rules::{self, HARNESS_DIR, Kind, Rule};
use crate::solve::{self, Follow, Task};
use crate::team;
use crate::world::{World, Worlds};
use crate::{schedule, sleep};

const BRIEF: &str = "Turn the request below into acceptance tests, before anyone implements it. \
Read the project first and follow its test conventions. Read the data model and entry points \
before writing tests: the goal you write must be specific enough that an implementer never \
needs to ask about code structure — if the request says 'two fields' and the code has one, \
say which one to split and how. Write tests only, in new files: they are \
protected whole once accepted, so the implementation must stay free to edit every existing file \
(in Rust, an integration test under `tests/`). No implementation, no stubs. Cover the edge cases the request implies; where it is ambiguous, \
pick the reading a careful maintainer would and say so in the goal. \
A careful maintainer does not invent the product: when the request does not say what to build \
or change, or the project is empty and the request leaves open what kind of program it is \
(language, command line or web page), write nothing and call `done` with the summary \
`question: <the one question whose answer lets you write the tests, in the request's language>`. \
Only the product is a question: where a file goes, its name, and how the tests reach it are yours \
to decide. Something the request asks for that is not the project's kind (a web page in a library) \
is a new file at the root, which the tests read or run as the project's tests can. \
When finished, call `done` with a summary in exactly this form:\n\
done_when: <what the person using the product will see or be able to do once it is done, in one or two sentences, in the request's language; product terms only, no file, tag, function or test names; it describes what your tests check, not what the request says: where you chose a reading, it states that reading>\n\
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

/// Framing without tests: one model call, no world to explore. The person \
/// validates "done when"; the work is judged by the project's own tests, \
/// named here. `compile` keeps the full path (a sparring finding still \
/// needs its failing test).
const FRAME: &str = "Turn the request below into the words the person will check it with, \
before anyone implements it. Read the project first: what it is, and how its tests are run. \
Write nothing. \
A careful project lead does not invent the product: when the request does not say what to \
build or change, or the project is empty and the request leaves open what kind of program it \
is (language, command line or web page), reply with exactly one line and nothing else:\n\
`question: <the one question whose answer lets you frame it, in the request's language>`\n\
Otherwise reply in exactly this form, nothing else:\n\
done_when: <what the person using the product will see or be able to do once it is done, in \
one or two sentences, in the request's language; product terms only, no file, tag, function \
or test names, and never the state of git (a diff, a commit): the work is committed as it \
goes>\n\
verify: <the shell command that runs the project's whole test suite, from its root; one line \
per command, at most two; `none` if the project has no tests>";

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

pub fn save(worlds: &Worlds, id: &str, spec: &Value) -> io::Result<()> {
    std::fs::create_dir_all(worlds.root.join("specs"))?;
    let text = serde_json::to_string_pretty(spec).map_err(io::Error::other)?;
    review::write_atomic(&spec_path(worlds, id)?, &text)
}

/// The conversation that preceded the work; the solve reads it into every
/// attempt, the proof never sees it.
pub fn set_discussion(worlds: &Worlds, id: &str, discussion: &str) -> io::Result<()> {
    let mut spec = load(worlds, id)?;
    spec["discussion"] = json!(discussion);
    save(worlds, id, &spec)
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
    let answers: Vec<Value> = specs(worlds)?
        .into_iter()
        .filter(|(_, s)| {
            s["project"] == name.as_str()
                && s["request"].as_str().map(normalise) == Some(key.clone())
        })
        .flat_map(|(_, s)| s["answers"].as_array().cloned().unwrap_or_default())
        .collect();
    context.push_str(&answered(&answers));
    let brief = format!("{BRIEF}{request}");
    write_spec(
        worlds, &project, &brief, request, "request", &context, model, max_steps,
    )
}

/// Frames a request without compiling tests: one model call writes
/// "done when", the words the human validates, and the command that runs
/// the project's tests, the check every attempt of the work is judged on.
/// The spec's world is forked with no test of its own: attempts fork from
/// it, as from `compile`'s world. A question goes to the human, whose
/// answer reaches the next framing.
pub fn frame(
    worlds: &Worlds,
    project: &Path,
    request: &str,
    model: &dyn Model,
) -> io::Result<Value> {
    let project = std::fs::canonicalize(project)?;
    let _turn = schedule::interactive(worlds)?;
    let name = project.display().to_string();
    let key = normalise(request);
    let answers: Vec<Value> = specs(worlds)?
        .into_iter()
        .filter(|(_, s)| {
            s["project"] == name.as_str()
                && s["request"].as_str().map(normalise) == Some(key.clone())
        })
        .flat_map(|(_, s)| s["answers"].as_array().cloned().unwrap_or_default())
        .collect();
    let files = context(&project);
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
    let (roles, team) = team::brief(&project, "frame")?;
    context.push_str(&roles);
    let messages = vec![
        json!({"role": "system", "content": format!("{FRAME}\n\n{files}\n\n{context}")}),
        json!({"role": "user", "content": answered(&answers) + request}),
    ];
    let (content, exchange) = crate::provider::ask(model, &messages)?;
    let (done, verify, question) = frame_parse(&content);
    let world = worlds.fork(&name)?;
    let id = world.id.clone();
    let mut spec = json!({
        "project": name,
        "request": request,
        "origin": "frame",
        "world": world.id,
        "model": model.label(),
        "team": team,
        "status": "invalid",
        "summary": content,
        "rules": rules::used(request, &said),
        // What the lead's journal and stats read the framing's cost from.
        "messages": exchange["messages"],
        "turns": exchange["turns"],
    });
    match (done, question) {
        (Some(d), _) => {
            // The goal the work builds to: the request itself, no one
            // restating it; the tests the developer writes are its own.
            spec["goal"] = json!(request);
            spec["done_when"] = json!(d);
            spec["verify"] = json!(verify);
            spec["protect"] = json!(Vec::<String>::new());
            spec["diff"] = json!("");
            spec["checks"] = json!(Vec::<Value>::new());
            spec["status"] = json!("proposed");
        }
        (None, Some(q)) => {
            spec["question"] = json!(q);
            spec["reason"] = json!("asks before framing");
        }
        (None, None) => {
            let said: String = content.chars().take(300).collect();
            spec["reason"] = json!(format!(
                "the reply has no `done_when:` line; it said: {said}"
            ));
        }
    }
    if spec["status"] != "proposed" {
        worlds.drop(world)?;
    }
    save(worlds, &id, &spec)?;
    Ok(spec)
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
    let said = rules::said(project)?;
    context.push_str(&rules::brief(&said));
    let (roles, team) = team::brief(project, "frame")?;
    context.push_str(&roles);
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
        "team": team,
        "status": "invalid",
    });
    if let Err(e) = check(
        worlds, &world, project, brief, model, &criteria, &context, max_steps, &mut spec,
    ) {
        spec["reason"] = json!(e.to_string());
    }
    spec["rules"] = json!(rules::used(spec["summary"].as_str().unwrap_or(""), &said));
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
    let criteria = criteria.iter().map(|c| agent::criterion(c, None)).collect();
    let outcome = agent::run(
        worlds,
        world,
        model,
        brief,
        criteria,
        context,
        max_steps,
        None,
        false,
        None,
        agent::Watch::default(),
    )?;
    spec["steps"] = json!(outcome.steps);
    spec["stuck"] = json!(outcome.stuck);
    spec["question"] = json!(outcome.question);
    spec["turns"] = json!(outcome.turns);
    spec["messages"] = json!(outcome.messages);
    if let Some(e) = outcome.error {
        return Err(io::Error::other(e));
    }
    let summary = outcome.summary.unwrap_or_default();
    // Kept so the human sees what the model said when parsing finds nothing.
    spec["summary"] = json!(summary);
    // A request too vague to test: the question goes to the human, whose
    // answer reaches the next framing.
    if spec["question"].is_null()
        && let Some(q) = agent::parse_question(&summary)
    {
        spec["question"] = json!(q);
    }

    // An empty new file is a leftover of the framer probing the project (a
    // `touch index.html` to see the tests pass), never a test: kept, it would
    // be protected and forbid the implementation its own file.
    let untracked = worlds.git(world, &["ls-files", "--others", "--exclude-standard"])?;
    for file in untracked.lines() {
        let path = world.path.join(file);
        if fs::metadata(&path).is_ok_and(|m| m.is_file() && m.len() == 0) {
            fs::remove_file(&path)?;
        }
    }
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

    let mut form = summary.clone();
    let (mut goal, mut verify) = parse(&form);
    if (goal.is_empty() || verify.is_empty()) && !changed.is_empty() {
        // The tests are written; only the summary missed the form. One more
        // question costs far less than writing the tests again.
        form = reformat(model, brief, &summary, &changed)?;
        (goal, verify) = parse(&form);
        spec["reformatted"] = json!(form);
    }
    spec["goal"] = json!(goal);
    spec["verify"] = json!(verify);
    // What the human validates; the goal stays for the implementer.
    spec["done_when"] = json!(done_when(&form).unwrap_or(&goal));

    let reason = if let (true, Some(why)) = (changed.is_empty(), &outcome.stuck) {
        Some(format!("{why}; no test was written"))
    } else if changed.is_empty() && spec["question"].is_string() {
        Some("asks before writing tests".to_string())
    } else if changed.is_empty() {
        Some("no test was written".to_string())
    } else if goal.is_empty() || verify.is_empty() {
        let said: String = summary.chars().take(300).collect();
        Some(format!(
            "the summary has no `goal:` or no `verify:` line; it said: {said}"
        ))
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

/// Asks the model to restate a summary that does not follow the form. The
/// commands it names are still run below: a wrong one fails the spec there.
fn reformat(
    model: &dyn Model,
    request: &str,
    summary: &str,
    changed: &[&str],
) -> io::Result<String> {
    let ask = format!(
        "Request:\n{request}\n\nTest files written:\n{}\n\nSummary of the work:\n{}\n\n\
         Reply in exactly this form, nothing else:\n\
         done_when: <what the person using the product will see or be able to do once it is done, in one or two sentences, in the request's language; product terms only, no file, tag, function or test names; it describes what your tests check, not what the request says: where you chose a reading, it states that reading>\n\
         goal: <the request restated precisely for an implementer, naming the tests that must pass>\n\
         verify: <a shell command that runs the new tests; one line per command>",
        changed.join("\n"),
        if summary.trim().is_empty() {
            "(none)"
        } else {
            summary
        }
    );
    let reply = model.chat(&[json!({"role": "user", "content": ask})], &json!([]))?;
    let content = reply["content"].as_str().unwrap_or("");
    // Some servers leave the reasoning inline.
    Ok(content
        .rsplit("</think>")
        .next()
        .unwrap_or(content)
        .to_string())
}

/// The one `done_when:` line, if the model wrote it.
fn done_when(summary: &str) -> Option<&str> {
    summary
        .lines()
        .find_map(|l| l.trim().strip_prefix("done_when:"))
        .map(str::trim)
        .filter(|d| !d.is_empty())
}

fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or("")
}

/// `goal:` starts the goal, which runs until the first `verify:`. A local
/// model does not keep the format exactly: `verify:` may close the goal's
/// line, keys may be bulleted or bold, and a bare `verify:` takes the next
/// lines as commands, up to a blank line; code fences are dropped.
fn parse(summary: &str) -> (String, Vec<String>) {
    let mut goal = Vec::new();
    let mut verify = Vec::new();
    let (mut in_goal, mut in_verify) = (false, false);
    let lines = summary.lines().flat_map(|line| match line.find("verify:") {
        Some(i)
            if !line[..i]
                .trim_start_matches(['-', '*', '#', '>', ' '])
                .is_empty() =>
        {
            vec![&line[..i], &line[i..]]
        }
        _ => vec![line],
    });
    for line in lines {
        let trimmed = line.trim();
        if key(trimmed, "rules").is_some() || key(trimmed, "done_when").is_some() {
            (in_goal, in_verify) = (false, false);
        } else if let Some(cmd) = key(trimmed, "verify") {
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
                        .trim_start_matches("- ")
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

/// `done_when:`, `verify:`, and the `question:` a framing reply may carry
/// instead, with the same tolerance as `parse`. `verify` is `none` for a
/// project without tests.
fn frame_parse(content: &str) -> (Option<String>, Vec<String>, Option<String>) {
    let (mut done, mut verify, mut question) = (None, Vec::new(), None);
    let mut in_verify = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(d) = key(trimmed, "done_when") {
            if !d.is_empty() {
                done = Some(d.to_string());
            }
            in_verify = false;
        } else if let Some(q) = key(trimmed, "question") {
            if !q.is_empty() {
                question = Some(q.to_string());
            }
            in_verify = false;
        } else if let Some(cmd) = key(trimmed, "verify") {
            in_verify = cmd.is_empty();
            if !cmd.is_empty() && !cmd.eq_ignore_ascii_case("none") {
                verify.push(cmd.trim_matches('`').to_string());
            }
        } else if in_verify {
            if trimmed.is_empty() && !verify.is_empty() {
                in_verify = false;
            } else if !trimmed.is_empty() && !trimmed.starts_with("```") {
                verify.push(
                    trimmed
                        .trim_start_matches("- ")
                        .trim_start_matches("$ ")
                        .trim_matches('`')
                        .to_string(),
                );
            }
        }
    }
    (done, verify, question)
}

/// The human validates the spec: its tests become the task's protected
/// checks and the attempts fork from its world. Returns the run record.
pub fn accept(
    worlds: &Worlds,
    id: &str,
    models: &[&dyn Model],
    max_steps: usize,
    log: Option<&Path>,
) -> io::Result<Value> {
    solve_spec(worlds, id, "", models, max_steps, log)
}

/// Solves a spec again after its run failed, possibly after it was called a
/// winner (its product failed the proof): that run is dropped, and attempts
/// start from the tests again, told why the last one failed.
pub fn retry(
    worlds: &Worlds,
    id: &str,
    why: &str,
    models: &[&dyn Model],
    max_steps: usize,
    log: Option<&Path>,
) -> io::Result<Value> {
    let mut spec = load(worlds, id)?;
    if spec["status"] == "accepted" {
        if let Some(run) = spec["run"].as_str() {
            review::reject(worlds, run, None)?;
        }
        spec["status"] = json!("proposed");
        save(worlds, id, &spec)?;
    }
    let why = format!("\n\nAn earlier solution failed: {why}\nDo not repeat it.");
    solve_spec(worlds, id, &why, models, max_steps, log)
}

/// The human tried a ready run and asks for a change: attempts start from
/// that run's work, not from the tests. A winner replaces the run, which is
/// rejected with the correction (so one said twice is offered as a rule);
/// without one, the run stays ready as it was.
pub fn amend(
    worlds: &Worlds,
    id: &str,
    correction: &str,
    models: &[&dyn Model],
    max_steps: usize,
    log: Option<&Path>,
) -> io::Result<Value> {
    let ask = format!(
        "\n\nA solution already passed and is in your world: start from it. \
         The human tried it and asks for this change:\n{correction}\n\
         Make that change and keep the rest."
    );
    rework(worlds, id, &ask, Some(correction), models, max_steps, log)
}

/// The proof tried a ready run and it failed: attempts start from that run's
/// work, told what failed, as `amend` does for the human. The findings are
/// not the human's correction, so they never become a rule offer.
pub fn fix(
    worlds: &Worlds,
    id: &str,
    findings: &str,
    models: &[&dyn Model],
    max_steps: usize,
    log: Option<&Path>,
) -> io::Result<Value> {
    let ask = format!(
        "\n\nA solution is in your world: start from it. It was then tried as \
         its user would, and this failed:\n{findings}\n\
         Find the cause and fix it, keep the rest. If what failed is the way it \
         was tried (a command not installed here, a wrong path), make the \
         product easy to try: a script or a README line that works here."
    );
    rework(worlds, id, &ask, None, models, max_steps, log)
}

/// A run stopped by the person back at work: attempts start from its world
/// (its changes stay), told why it was stopped, as `fix` does for a proof's
/// findings. It is not the person's correction, so it never becomes a rule
/// offer.
pub fn restart(
    worlds: &Worlds,
    id: &str,
    why: &str,
    models: &[&dyn Model],
    max_steps: usize,
    log: Option<&Path>,
) -> io::Result<Value> {
    let ask = format!(
        "\n\nAn earlier run of this work was stopped: {why}\nIts changes are \
         already in the world. Continue from where it stopped; do not redo what \
         it did."
    );
    rework(worlds, id, &ask, None, models, max_steps, log)
}

fn rework(
    worlds: &Worlds,
    id: &str,
    ask: &str,
    correction: Option<&str>,
    models: &[&dyn Model],
    max_steps: usize,
    log: Option<&Path>,
) -> io::Result<Value> {
    let mut spec = load(worlds, id)?;
    let Some(run) = spec["run"].as_str().map(str::to_string) else {
        return Err(io::Error::other(format!("spec {id} has no ready run")));
    };
    // Its roles go on in their own conversations: what they read and did is
    // still there, and the server still caches it.
    let follow = review::load(worlds, &run).ok().and_then(|r| {
        let attempts = r["attempts"].as_array()?;
        let attempt = attempts
            .iter()
            .find(|a| a["world"] == r["winner"])
            .or(attempts.last())?;
        Some(Follow {
            ask: ask.trim().to_string(),
            roles: attempt["roles"].as_array().cloned().unwrap_or_default(),
        })
    });
    let mut record = run_spec(worlds, &spec, ask, &run, follow, models, max_steps, log)?;
    // Forked from the run, but still this spec's: landing drops its world.
    record["spec"] = json!(id);
    let name = record["id"].as_str().unwrap_or("run").to_string();
    review::save(worlds, &name, &record)?;
    if let Some(winner) = record["winner"].as_str() {
        review::reject(worlds, &run, correction)?;
        spec["run"] = json!(winner);
        save(worlds, id, &spec)?;
    }
    Ok(record)
}

fn solve_spec(
    worlds: &Worlds,
    id: &str,
    why: &str,
    models: &[&dyn Model],
    max_steps: usize,
    log: Option<&Path>,
) -> io::Result<Value> {
    let mut spec = load(worlds, id)?;
    if spec["status"] != "proposed" {
        return Err(io::Error::other(format!("spec {id} is {}", spec["status"])));
    }
    let record = run_spec(worlds, &spec, why, id, None, models, max_steps, log)?;
    spec["status"] = json!("accepted");
    spec["run"] = record["winner"].clone();
    // The spec's world stays until the run lands (`solve::land`): a winner
    // whose product fails later is retried from the tests.
    // ponytail: a run never landed nor retried keeps its spec world; a sweep
    // can drop them once requests can be closed.
    if !record["winner"].is_string() {
        spec["status"] = json!("proposed");
    }
    save(worlds, id, &spec)?;
    Ok(record)
}

/// The spec's tests as a task whose attempts fork from `base`.
#[allow(clippy::too_many_arguments)]
fn run_spec(
    worlds: &Worlds,
    spec: &Value,
    why: &str,
    base: &str,
    follow: Option<Follow>,
    models: &[&dyn Model],
    max_steps: usize,
    log: Option<&Path>,
) -> io::Result<Value> {
    let strings = |k: &str| -> Vec<String> {
        spec[k]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|s| s.as_str().map(str::to_string))
            .collect()
    };
    let answers = spec["answers"].as_array().cloned().unwrap_or_default();
    let task = Task {
        goal: format!(
            "{}{}{why}",
            spec["goal"].as_str().unwrap_or(""),
            answered(&answers)
        ),
        verify: strings("verify"),
        protect: strings("protect"),
        base: Some(base.to_string()),
        grant: Vec::new(),
        skills: spec["skills"].is_array().then(|| strings("skills")),
        roles: spec["roles"].is_array().then(|| strings("roles")),
        discussion: spec["discussion"].as_str().map(str::to_string),
        follow,
    };
    let project = PathBuf::from(spec["project"].as_str().unwrap_or(""));
    solve::solve(worlds, &project, &task, models, max_steps, log)
}

/// Records the human's answer to a question a stopped agent asked. The next
/// compile of the same request, or the next attempt on this spec, reads it.
pub fn answer(worlds: &Worlds, id: &str, question: &str, answer: &str) -> io::Result<()> {
    let mut spec = load(worlds, id)?;
    let entry = json!({"question": question, "answer": answer});
    match spec["answers"].as_array_mut() {
        Some(list) => list.push(entry),
        None => spec["answers"] = json!([entry]),
    }
    save(worlds, id, &spec)
}

fn answered(answers: &[Value]) -> String {
    if answers.is_empty() {
        return String::new();
    }
    let mut text =
        "\n\nAn earlier attempt was stopped and asked the human; their answers:\n".to_string();
    for a in answers {
        text.push_str(&format!(
            "  - Q: {}\n    A: {}\n",
            a["question"].as_str().unwrap_or(""),
            a["answer"].as_str().unwrap_or("")
        ));
    }
    text
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

const DISCUSS: &str = "You help a person shape a request for a team of AI developers, before \
any work starts. Talk with them in their language, briefly, like a colleague. Most requests are \
clear enough: if a careful developer could build a sound version of it, it is clear. Ask only \
when you could not start at all (what kind of thing to build is open), in one or two sentences \
of what you understood and the one or two questions that matter, with concrete options; never \
invent the product. Layout, style, update rate, file names and other details are the \
developer's to settle, not questions. Once it is clear, a clear first message included, reply \
with exactly one line and nothing else:\n\
`brief: <the whole request restated precisely, with everything agreed in this conversation, \
in their language>`\n\
then one more line naming it the way a person titles a ticket:\n\
`title: <a short label, at most eight words, in their language>`";

/// Asked after `DISCUSS` when the person keeps skills: the work then sees
/// only the ones that serve this request.
const PICK: &str = "\n\nThe person keeps these skills for the developers. With the brief, add \
one more line naming the ones this request needs, or none:\n`skills: <names, comma separated>`\n";

/// Asked with `PICK` when the team has solve roles: which of them this
/// request needs. The work then runs exactly those, in team order.
const MARK: &str = "\n\nThe team's developers are:\n{}With the brief, add one more line naming \
the ones this request needs, or none:\n`roles: <keys, comma separated>`\n";

/// One turn of shaping a request before work starts: either a message for
/// the human, or the brief the work takes.
/// A brief comes with the skills picked for it; `None` when the person
/// keeps none or the model did not say, and the work then sees them all.
/// It also marks the solve roles the work needs; `None` runs the team's
/// usual chain. Last, its short title; `None` when the model gave none.
#[derive(Debug, PartialEq)]
pub enum Discussed {
    Reply(String),
    Brief(
        String,
        Option<Vec<String>>,
        Option<Vec<String>>,
        Option<String>,
    ),
}

/// `thread`: `{"who": "human" | "fennec", "text"}` in order. One model call,
/// no world: the conversation must answer in seconds, not minutes. The
/// exchange comes back too, as `provider::ask` records it.
pub fn discuss(
    project: &Path,
    thread: &[Value],
    model: &dyn Model,
) -> io::Result<(Discussed, Value)> {
    let context = context(project);
    // The roles that shaped requests before work starts speak here now.
    let (roles, team) = team::brief(project, "frame")?;
    let kept = crate::skill::chosen(Some(project))?;
    let pick = if kept.is_empty() {
        String::new()
    } else {
        let list: String = kept
            .iter()
            .map(|s| format!("  - {}: {}\n", s.name, s.description))
            .collect();
        format!("{PICK}{list}")
    };
    // The team's solve roles, so the PM marks which of them this request
    // needs; the work then runs exactly those.
    // All of them, the usual chain included: the PM marks what the request
    // needs, and what it does not mark stays out of the work.
    let chain = team::members(project)?
        .into_iter()
        .filter(|r| r.stage == "solve")
        .collect::<Vec<_>>();
    let mark = if chain.is_empty() {
        String::new()
    } else {
        let list: String = chain
            .iter()
            .map(|r| format!("  - {}: {}\n", r.key, r.name))
            .collect();
        format!("{MARK}{list}")
    };
    let keys: Vec<String> = chain.iter().map(|r| r.key.clone()).collect();
    let mut messages = vec![
        json!({"role": "system", "content": format!("{DISCUSS}{pick}{mark}\n\n{context}{roles}")}),
    ];
    for turn in thread {
        let role = if turn["who"] == "human" {
            "user"
        } else {
            "assistant"
        };
        messages.push(json!({"role": role, "content": turn["text"]}));
    }
    let (mut content, mut exchange) = crate::provider::ask(model, &messages)?;
    // A model trained on tools reaches for them here, where it has none
    // (seen: `<tool_call><function=Read>` as its whole reply, shown to the
    // person as a question). That is no message for them: asked once more.
    if calls_as_text(&content) {
        let mut again = messages.clone();
        again.push(json!({"role": "assistant", "content": content}));
        again.push(json!({"role": "user", "content": NO_TOOLS}));
        let (said, mut second) = crate::provider::ask(model, &again)?;
        let mut turns = exchange["turns"].as_array().cloned().unwrap_or_default();
        turns.extend(second["turns"].as_array().cloned().unwrap_or_default());
        second["turns"] = json!(turns);
        (content, exchange) = (said, second);
        if calls_as_text(&content) {
            return Err(io::Error::other(
                "the model tried to read the project instead of answering",
            ));
        }
    }
    exchange["team"] = json!(team);
    if content.is_empty() {
        return Err(io::Error::other("the model gave no answer"));
    }
    let brief = content
        .lines()
        .find_map(|l| l.trim().trim_matches('`').strip_prefix("brief:"))
        .map(|b| b.trim().trim_matches('`').trim().to_string())
        .filter(|b| !b.is_empty());
    let title = content
        .lines()
        .find_map(|l| l.trim().trim_matches('`').strip_prefix("title:"))
        .map(|t| t.trim().trim_matches('`').trim().to_string())
        .filter(|t| !t.is_empty());
    let names: Vec<String> = kept.into_iter().map(|s| s.name).collect();
    let said = match brief {
        Some(b) => Discussed::Brief(b, picked(&content, &names), marked(&content, &keys), title),
        None => Discussed::Reply(content),
    };
    Ok((said, exchange))
}

/// Said when the model wrote tool calls in a conversation that has none.
const NO_TOOLS: &str = "You have no tools in this conversation: you cannot read files or run \
anything. Answer from what you were told: the brief line if the request is clear, otherwise \
your question.";

/// Tool calls written as text, the way Qwen-style templates spell them.
fn calls_as_text(content: &str) -> bool {
    content.contains("<tool_call>") || content.contains("<function=")
}

/// The `skills:` line of a reply, kept to the names the person keeps.
fn picked(content: &str, kept: &[String]) -> Option<Vec<String>> {
    if kept.is_empty() {
        return None;
    }
    let line = content
        .lines()
        .find_map(|l| l.trim().trim_matches('`').strip_prefix("skills:"))?;
    Some(
        line.split(',')
            .map(|n| n.trim().trim_matches('`').to_string())
            .filter(|n| kept.contains(n))
            .collect(),
    )
}

/// The `roles:` line of a reply, kept to the keys of the team's solve
/// roles. Not said: the team's usual chain works.
fn marked(content: &str, keys: &[String]) -> Option<Vec<String>> {
    if keys.is_empty() {
        return None;
    }
    let line = content
        .lines()
        .find_map(|l| l.trim().trim_matches('`').strip_prefix("roles:"))?;
    Some(
        line.split(',')
            .map(|n| n.trim().trim_matches('`').to_string())
            .filter(|n| keys.contains(n))
            .collect(),
    )
}

/// What the model knows of the project: its files and README, enough to
/// talk about this project rather than a generic one.
fn context(project: &Path) -> String {
    // ponytail: 200 files and 3 kB of README; a big project needs a summary.
    let files = crate::world::git(project, &["ls-files"]).unwrap_or_default();
    let files: Vec<&str> = files.lines().take(200).collect();
    let readme: String = ["README.md", "README", "AGENTS.md"]
        .iter()
        .find_map(|f| std::fs::read_to_string(project.join(f)).ok())
        .unwrap_or_default()
        .chars()
        .take(3000)
        .collect();
    if files.is_empty() {
        "The project is empty.".to_string()
    } else {
        format!("Project files:\n{}\n\nREADME:\n{readme}", files.join("\n"))
    }
}

const SPLIT: &str = "You split a task for a team of AI developers into smaller tasks, each one \
delivered and tested alone, in the order they should be done. Two to six tasks; fewer is better. \
Each one says in a few sentences, in the task's language, what to build and how to know it \
works. A task needs an earlier one only when it builds on its code. Reply with JSON only:\n\
`[{\"task\": \"...\", \"after\": [numbers of the earlier tasks it needs, from 0]}]`";

/// A task cut into smaller ones, in order, each with the earlier ones it
/// builds on (their positions). One model call, no world.
pub fn split(
    project: &Path,
    task: &str,
    model: &dyn Model,
) -> io::Result<Vec<(String, Vec<usize>)>> {
    let messages = [
        json!({"role": "system", "content": format!("{SPLIT}\n\n{}", context(project))}),
        json!({"role": "user", "content": task}),
    ];
    let (content, _) = crate::provider::ask(model, &messages)?;
    parse_split(&content)
}

/// The model's list, read leniently (fenced or not); a task can only need
/// earlier ones, so there is no cycle.
fn parse_split(content: &str) -> io::Result<Vec<(String, Vec<usize>)>> {
    let bad = || io::Error::other("the model did not give a list of tasks");
    let (start, end) = (
        content.find('[').ok_or_else(bad)?,
        content.rfind(']').ok_or_else(bad)?,
    );
    let list: Vec<Value> =
        serde_json::from_str(content.get(start..=end).ok_or_else(bad)?).map_err(|_| bad())?;
    let tasks: Vec<(String, Vec<usize>)> = list
        .iter()
        .enumerate()
        .filter_map(|(i, t)| {
            let text = t["task"].as_str()?.trim().to_string();
            let after = t["after"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_u64)
                .map(|a| a as usize)
                .filter(|a| *a < i)
                .collect();
            (!text.is_empty()).then_some((text, after))
        })
        .collect();
    if tasks.len() < 2 || tasks.len() != list.len() {
        return Err(bad());
    }
    Ok(tasks)
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
    use super::{done_when, marked, parse, parse_split, picked, reformat};
    use crate::provider::Model;
    use serde_json::{Value, json};
    use std::io;

    struct Fixed(&'static str);
    impl Model for Fixed {
        fn chat(&self, _: &[Value], _: &Value) -> io::Result<Value> {
            Ok(json!({"content": self.0}))
        }
        fn label(&self) -> String {
            "fixed".into()
        }
        fn variant<'s>(&'s self, _: &str) -> Box<dyn Model + 's> {
            Box::new(Fixed(self.0))
        }
    }

    #[test]
    fn a_discussion_ends_on_a_brief() {
        use super::{Discussed, discuss};
        let empty = std::env::temp_dir();
        let thread = [json!({"who": "human", "text": "un test"})];
        let (said, exchange) = discuss(
            &empty,
            &thread,
            &Fixed("<think>vague</think>Tu veux tester quoi ? A) un script B) une page"),
        )
        .unwrap();
        // What was sent and what came back, reasoning included, for the human.
        let messages = exchange["messages"].as_array().unwrap();
        assert_eq!(messages[1]["content"], "un test");
        assert!(messages[2]["content"].as_str().unwrap().contains("vague"));
        assert_eq!(exchange["turns"].as_array().unwrap().len(), 1);
        // The framing roles are briefed into the conversation, and named.
        let system = messages[0]["content"].as_str().unwrap();
        for name in exchange["team"].as_array().unwrap() {
            assert!(system.contains(&format!("As {} on the team", name.as_str().unwrap())));
        }
        assert_eq!(
            said,
            Discussed::Reply("Tu veux tester quoi ? A) un script B) une page".into())
        );
        assert_eq!(
            discuss(
                &empty,
                &thread,
                &Fixed("OK.\n`brief: un script bonjour.py qui affiche Bonjour`")
            )
            .unwrap()
            .0,
            Discussed::Brief(
                "un script bonjour.py qui affiche Bonjour".into(),
                None,
                None,
                None
            )
        );
        assert_eq!(
            discuss(
                &empty,
                &thread,
                &Fixed(
                    "`brief: un script bonjour.py qui affiche Bonjour`\n`title: Script bonjour`"
                )
            )
            .unwrap()
            .0,
            Discussed::Brief(
                "un script bonjour.py qui affiche Bonjour".into(),
                None,
                None,
                Some("Script bonjour".into())
            )
        );
        assert!(discuss(&empty, &thread, &Fixed("<think>only this</think>")).is_err());
        // Tool calls written as text are asked again, without tools.
        struct Seq(std::sync::Mutex<Vec<&'static str>>);
        impl Model for Seq {
            fn chat(&self, _: &[Value], _: &Value) -> io::Result<Value> {
                Ok(json!({"content": self.0.lock().unwrap().remove(0)}))
            }
            fn label(&self) -> String {
                "seq".into()
            }
            fn variant<'s>(&'s self, _: &str) -> Box<dyn Model + 's> {
                unreachable!()
            }
        }
        let read = "Je regarde.\n<tool_call>\n<function=Read>\n</function>\n</tool_call>";
        let seq = Seq(std::sync::Mutex::new(vec![read, "`brief: un script`"]));
        let (said, exchange) = discuss(&empty, &thread, &seq).unwrap();
        assert_eq!(said, Discussed::Brief("un script".into(), None, None, None));
        assert_eq!(exchange["turns"].as_array().unwrap().len(), 2);
        let stubborn = Seq(std::sync::Mutex::new(vec![read, read]));
        assert!(discuss(&empty, &thread, &stubborn).is_err());
    }

    #[test]
    fn a_summary_off_the_form_is_reformatted() {
        let model = Fixed("<think>hmm</think>goal: count items\nverify: node --test test/");
        let text = reformat(
            &model,
            "count",
            "I wrote tests in test/app.test.js.",
            &["test/app.test.js"],
        )
        .unwrap();
        let (goal, verify) = parse(&text);
        assert_eq!(goal, "count items");
        assert_eq!(verify, ["node --test test/"]);
    }

    #[test]
    fn done_when_is_for_the_human_and_stays_out_of_the_goal() {
        let summary =
            "goal: add `count()`\ndone_when: la liste affiche « 3 articles »\nverify: sh t.sh";
        assert_eq!(parse(summary).0, "add `count()`");
        assert_eq!(done_when(summary), Some("la liste affiche « 3 articles »"));
        assert_eq!(done_when("goal: g\ndone_when:  \n"), None);
    }

    #[test]
    fn framing_reads_done_when_verify_and_the_question() {
        use super::frame_parse;
        let (done, verify, question) =
            frame_parse("Clair.\ndone_when: la page s'ouvre avec le titre\nverify: `npm test`\n");
        assert_eq!(done.as_deref(), Some("la page s'ouvre avec le titre"));
        assert_eq!(verify, ["npm test"]);
        assert_eq!(question, None);
        // `none` for a project without tests, and a bare key takes the lines.
        let (_, verify, _) = frame_parse("done_when: ok\nverify:\nnpm test\ncargo test\n");
        assert_eq!(verify, ["npm test", "cargo test"]);
        let (_, verify, _) = frame_parse("done_when: ok\nverify: none\n");
        assert!(verify.is_empty());
        // A question is the reply, not the framing.
        let (done, _, question) = frame_parse("question: une page ou un script ?\n");
        assert_eq!(done, None);
        assert_eq!(question.as_deref(), Some("une page ou un script ?"));
    }

    #[test]
    fn summary_parsing() {
        let (goal, verify) = parse(
            "Here you go.\ngoal: add `f`\n  that doubles.\nverify: `cargo test -q`\nverify: sh t.sh\n",
        );
        assert_eq!(goal, "add `f`\nthat doubles.");
        assert_eq!(verify, ["cargo test -q", "sh t.sh"]);
        // Seen live with qwen3.8-27b: `verify:` at the end of the goal line.
        let (goal, verify) = parse("goal: add divide. verify: python3 -m unittest test_calc -v");
        assert_eq!(goal, "add divide.");
        assert_eq!(verify, ["python3 -m unittest test_calc -v"]);
        let (_, verify) = parse("goal: g\nverify:\n```\npython3 -m unittest\n```\n\nDone.");
        assert_eq!(verify, ["python3 -m unittest"]);

        let (goal, verify) = parse(
            "**Goal:** add `f`.\n\n- **Verify**:\n```sh\n$ cargo test\nsh t.sh\n```\n\nDone.",
        );
        assert_eq!(goal, "add `f`.");
        assert_eq!(verify, ["cargo test", "sh t.sh"]);
    }

    #[test]
    fn a_split_needs_two_tasks_and_only_earlier_ones() {
        let got = parse_split(
            "Here:\n```json\n[{\"task\": \"api\", \"after\": [1]}, {\"task\": \"ui\", \"after\": [0]}]\n```",
        )
        .unwrap();
        assert_eq!(got, vec![("api".into(), vec![]), ("ui".into(), vec![0])]);
        assert!(
            parse_split("[{\"task\": \"one\"}]").is_err(),
            "nothing to split"
        );
        assert!(parse_split("no list").is_err());
        assert!(parse_split("[{\"task\": \"a\"}, {\"x\": 1}]").is_err());
    }

    #[test]
    fn the_roles_marked_are_the_team_s_solve_roles() {
        let keys = ["senior-developer".to_string(), "tech-lead".to_string()];
        let reply = "brief: une page\n`roles: senior-developer, design`";
        assert_eq!(
            marked(reply, &keys),
            Some(vec!["senior-developer".to_string()])
        );
        assert_eq!(
            marked("brief: x", &keys),
            None,
            "not said: the usual chain works"
        );
        assert_eq!(marked(reply, &[]), None);
    }

    #[test]
    fn the_skills_picked_are_among_those_kept() {
        let kept = ["frontend".to_string(), "rust".to_string()];
        let reply = "brief: a page\n`skills: frontend, design`";
        assert_eq!(picked(reply, &kept), Some(vec!["frontend".to_string()]));
        assert_eq!(picked("brief: x\nskills: none", &kept), Some(vec![]));
        assert_eq!(picked("brief: x", &kept), None, "not said: all of them");
        assert_eq!(picked(reply, &[]), None);
    }
}
