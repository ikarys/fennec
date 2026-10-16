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
Read the project first and follow its test conventions. Write tests only: no implementation, \
no stubs outside test files. Cover the edge cases the request implies; where it is ambiguous, \
pick the reading a careful maintainer would and say so in the goal. \
When finished, call `done` with a summary in exactly this form:\n\
done_when: <what the person using the product will see or be able to do once it is done, in one or two sentences, in the request's language; product terms only, no file, tag, function or test names; it describes what your tests check, not what the request says: where you chose a reading, it states that reading>\n\
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
    let answers: Vec<Value> = specs(worlds)?
        .into_iter()
        .filter(|(_, s)| {
            s["project"] == name.as_str()
                && s["request"].as_str().map(normalise) == Some(key.clone())
        })
        .flat_map(|(_, s)| s["answers"].as_array().cloned().unwrap_or_default())
        .collect();
    context.push_str(&answered(&answers));
    let said = rules::said(&project)?;
    context.push_str(&rules::brief(&said));
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
    spec["stuck"] = json!(outcome.stuck);
    spec["question"] = json!(outcome.question);
    let summary = outcome.summary.unwrap_or_default();
    spec["summary"] = json!(summary);

    let commit = worlds.commit(world, &format!("fennec-os spec: {}", first_line(request)))?;
    spec["diff"] = json!(worlds.git(world, &["diff", &base, &commit])?);
    let changed = worlds.git(world, &["diff", "--name-only", &base, &commit])?;
    let changed: Vec<&str> = changed.lines().collect();
    spec["protect"] = json!(changed);

    let mut form = summary.clone();
    let (mut goal, mut verify) = parse(&form);
    if (goal.is_empty() || verify.is_empty()) && !changed.is_empty() {
        // The tests are written; only the summary missed the form. One more
        // question costs far less than writing the tests again.
        form = reformat(model, request, &summary, &changed)?;
        (goal, verify) = parse(&form);
    }
    spec["goal"] = json!(goal);
    spec["verify"] = json!(verify);
    // What the human validates; the goal stays for the implementer.
    spec["done_when"] = json!(done_when(&form).unwrap_or(&goal));

    let reason = if let (true, Some(why)) = (changed.is_empty(), &outcome.stuck) {
        Some(format!("{why}; no test was written"))
    } else if changed.is_empty() {
        Some("no test was written".to_string())
    } else if goal.is_empty() || verify.is_empty() {
        Some("the summary has no `goal:` or no `verify:` line".to_string())
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
/// line, and its commands may follow on the next lines, fenced or bulleted.
fn parse(summary: &str) -> (String, Vec<String>) {
    let mut goal = Vec::new();
    let mut verify = Vec::new();
    let (mut in_goal, mut in_verify) = (false, false);
    let lines = summary.lines().flat_map(|line| match line.find("verify:") {
        Some(i) if i > 0 && !line[..i].trim().is_empty() => vec![&line[..i], &line[i..]],
        _ => vec![line],
    });
    for line in lines {
        let trimmed = line.trim();
        if trimmed.starts_with("rules:") || trimmed.starts_with("done_when:") {
            (in_goal, in_verify) = (false, false);
        } else if let Some(cmd) = trimmed.strip_prefix("verify:") {
            in_goal = false;
            let cmd = cmd.trim().trim_matches('`');
            in_verify = cmd.is_empty();
            if !cmd.is_empty() {
                verify.push(cmd.to_string());
            }
        } else if let Some(g) = trimmed.strip_prefix("goal:") {
            (in_goal, in_verify) = (true, false);
            goal.push(g.trim());
        } else if in_goal {
            goal.push(trimmed);
        } else if in_verify {
            let cmd = trimmed.trim_start_matches("- ").trim_matches('`');
            if trimmed.is_empty() && !verify.is_empty() {
                in_verify = false;
            } else if !cmd.is_empty() && !trimmed.starts_with("```") {
                verify.push(cmd.to_string());
            }
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
    solve_spec(worlds, id, "", models, max_steps)
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
    solve_spec(worlds, id, &why, models, max_steps)
}

fn solve_spec(
    worlds: &Worlds,
    id: &str,
    why: &str,
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
    let answers = spec["answers"].as_array().cloned().unwrap_or_default();
    let task = Task {
        goal: format!(
            "{}{}{why}",
            spec["goal"].as_str().unwrap_or(""),
            answered(&answers)
        ),
        verify: strings("verify"),
        protect: strings("protect"),
        base: Some(id.to_string()),
        grant: Vec::new(),
    };
    let project = PathBuf::from(spec["project"].as_str().unwrap_or(""));
    let record = solve::solve(worlds, &project, &task, models, max_steps)?;
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
    use super::{done_when, parse, reformat};
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
    }
}
