//! The review stage: after the work passes its tests and before the proof
//! tries the product, the team's review roles look at it, in the winner's
//! world. The model judges; the harness reads the verdict from the report
//! the reviewer leaves in relay/, the way the work is judged from exit
//! codes. A team without review roles skips the stage: its loop runs solve
//! to proof as before, and no model is called.

use std::io;
use std::path::Path;

use serde_json::{Value, json};

use crate::provider::Model;
use crate::{agent, review, team, world::Worlds};

/// The team's review roles run in the winner's world of the run `id`.
/// `done_when` is what the human validated: the work must meet it. Returns
/// `{"passed", "skipped", "findings", "question", "team", "steps",
/// "exchange"}` and keeps it in the run record as `review`.
pub fn run(
    worlds: &Worlds,
    id: &str,
    done_when: Option<&str>,
    model: &dyn Model,
    max_steps: usize,
) -> io::Result<Value> {
    let mut record = review::load(worlds, id)?;
    let world = worlds.open(id)?;
    let diff = record["attempts"]
        .as_array()
        .and_then(|a| a.iter().find(|t| t["world"] == id))
        .and_then(|t| t["diff"].as_str())
        .unwrap_or("");
    let project = Path::new(record["project"].as_str().unwrap_or(""));
    let roles = team::stage_roles(project, "review")?;
    if roles.is_empty() {
        let res = json!({"passed": true, "skipped": true, "findings": "",
                         "question": null, "team": Vec::<String>::new(), "steps": 0});
        record["review"] = res.clone();
        review::save(worlds, id, &record)?;
        return Ok(res);
    }
    let done = done_when.map_or_else(String::new, |d| format!("Done when:\n{d}\n\n"));
    let ask = format!(
        "{done}Goal:\n{}\n\nThe change to review (diff):\n{diff}\n\n\
         Review the work in this world against its goal and the done when.",
        record["goal"].as_str().unwrap_or("")
    );
    let mut context = String::new();
    let names: Vec<String> = roles.iter().map(|r| r.name.clone()).collect();
    for r in &roles {
        context.push_str(&format!("\n\nAs {} on the team:\n{}\n", r.name, r.brief));
    }
    let mut messages: Vec<Value> = Vec::new();
    let mut turns: Vec<Value> = Vec::new();
    let mut steps = 0;
    let mut summary: Option<String> = None;
    let mut question: Option<String> = None;
    let mut error: Option<String> = None;
    let mut remaining = max_steps;
    for role in &roles {
        if remaining == 0 {
            break;
        }
        // A role's own model; the stage keeps its own.
        let boxed = role.model.as_ref().map(|m| model.variant(m));
        let role_model = boxed.as_deref().unwrap_or(model);
        let outcome = agent::run(
            worlds,
            &world,
            role_model,
            &ask,
            Vec::new(),
            &context,
            remaining,
            Some(&role.tools),
            false,
            None,
            agent::Watch::default(),
        )?;
        remaining = remaining.saturating_sub(outcome.steps);
        steps += outcome.steps;
        messages.extend(outcome.messages.iter().cloned());
        turns.extend(outcome.turns.iter().cloned());
        if let Some(s) = outcome.summary {
            summary = Some(s);
        }
        if question.is_none() {
            question = outcome.question;
        }
        if let Some(e) = outcome.error {
            error = Some(e);
        }
    }
    // The report, then the verdict the model ended with: the harness reads
    // the words, it does not judge.
    let report_path = world.path.join("relay").join("review.md");
    let report = std::fs::read_to_string(&report_path).ok();
    // The reviewer's note does not change the work: it goes with the world
    // back to what the solver left. It is git-ignored, so it is read out,
    // not rolled back.
    let _ = std::fs::remove_file(&report_path);
    worlds.rollback(&world)?;
    let (rejected, findings) = verdict(report.as_deref(), summary.as_deref(), error.as_deref());
    let judged = report.is_some() || summary.is_some();
    let (passed, question) = conclude(rejected, judged, error, question);
    let res = json!({
        "passed": passed,
        "skipped": false,
        "findings": findings,
        "question": question,
        "team": names,
        "steps": steps,
        "exchange": {"model": model.label(), "messages": messages, "turns": turns},
    });
    record["review"] = res.clone();
    review::save(worlds, id, &record)?;
    Ok(res)
}

/// The verdict the harness reads: the report the reviewer left, then the
/// verdict the model ended with. `(rejected, findings)`.
fn verdict(report: Option<&str>, summary: Option<&str>, error: Option<&str>) -> (bool, String) {
    let text = report
        .or(summary)
        .unwrap_or_else(|| error.unwrap_or_default());
    let rejected = text.contains("à refaire");
    (
        rejected,
        if rejected {
            text.to_string()
        } else {
            String::new()
        },
    )
}

/// How the stage ends from what the reviewer left: a verdict passes or
/// rejects, and a review that never ended with one — no report, no summary,
/// the step budget spent — surfaces the stop, it does not pass silently.
fn conclude(
    rejected: bool,
    judged: bool,
    error: Option<String>,
    question: Option<String>,
) -> (bool, Option<String>) {
    if !rejected && !judged && error.is_some() {
        return (false, question.or(error));
    }
    (!rejected, question)
}

#[cfg(test)]
mod tests {
    use super::{conclude, verdict};

    #[test]
    fn the_verdict_is_read_from_the_report_then_the_model() {
        assert_eq!(
            verdict(Some("validé\n"), None, None),
            (false, String::new())
        );
        let (rejected, findings) = verdict(Some("le titre manque\nà refaire\n"), None, None);
        assert!(rejected, "{rejected}");
        assert!(findings.contains("le titre manque"), "{findings}");
        assert!(
            verdict(None, Some("à refaire"), None).0,
            "the model's verdict stands"
        );
        assert!(
            verdict(None, None, Some("timeout: à refaire")).0,
            "an error keeps the stage from passing silently"
        );
        assert_eq!(
            verdict(None, None, None),
            (false, String::new()),
            "no report, no verdict: the work is not judged against"
        );
    }

    #[test]
    fn a_review_that_never_ended_with_a_verdict_does_not_pass() {
        assert_eq!(
            conclude(false, true, Some("stopped".into()), None),
            (true, None)
        );
        assert_eq!(conclude(true, true, None, None), (false, None));
        let (passed, question) =
            conclude(false, false, Some("stopped after 10 steps".into()), None);
        assert!(!passed, "{passed}");
        assert!(question.is_some(), "the stop is surfaced, not silent");
        assert_eq!(conclude(false, false, None, None), (true, None));
        let (passed, question) = conclude(true, true, Some("stopped".into()), None);
        assert!(!passed, "{passed}");
        assert!(question.is_none(), "a rejection keeps the findings path");
    }
}
