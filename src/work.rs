//! A request's life, driven in the background: fennec-os compiles it into
//! "done when" (goal + failing tests), the human validates or corrects once,
//! then fennec-os solves it. Each step runs on its own thread; the request
//! file is the only state, so the view reads it like any other.

use std::io;
use std::path::Path;

use fennec_os::proof;
use fennec_os::provider::{Model, OpenAi};
use fennec_os::spec;
use fennec_os::world::Worlds;
use serde_json::{Value, json};

use crate::state::Store;

/// Agent steps per compile or attempt, as in the fennec-os CLI.
const MAX_STEPS: usize = 40;

/// `FENNEC_LEAD_ATTEMPTS` (default 1): parallel attempts once validated.
fn attempts() -> usize {
    std::env::var("FENNEC_LEAD_ATTEMPTS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1usize)
        .max(1)
}

/// Runs `step` in the background; an error fails the request with its reason.
fn spawn(id: String, step: fn(&Store, &str) -> io::Result<()>) {
    std::thread::spawn(move || {
        let Ok(store) = Store::from_env() else { return };
        if let Err(e) = step(&store, &id) {
            let _ = store.update(&id, |r| {
                r["status"] = json!("failed");
                r["reason"] = json!(e.to_string());
            });
        }
    });
}

pub fn frame(id: String) {
    spawn(id, frame_now);
}

fn frame_now(store: &Store, id: &str) -> io::Result<()> {
    let request = store.load(id)?;
    let worlds = Worlds::from_env()?;
    let model = OpenAi::variants_from_env(1)?.remove(0);
    let s = spec::compile(
        &worlds,
        Path::new(request["project"].as_str().unwrap_or("")),
        request["text"].as_str().unwrap_or(""),
        &model,
        MAX_STEPS,
    )?;
    store.update(id, |r| {
        r["spec"] = s["world"].clone();
        if s["status"] == "proposed" {
            r["status"] = json!("question");
            r["done_when"] = s["goal"].clone();
            r["verify"] = s["verify"].clone();
        } else if let Some(q) = s["question"].as_str() {
            block(r, "framing", q, s["reason"].as_str().unwrap_or(""));
        } else {
            r["status"] = json!("failed");
            r["reason"] = json!(format!(
                "« terminé quand » impossible à établir : {}",
                s["reason"].as_str().unwrap_or("raison inconnue")
            ));
        }
    })
}

/// The human's one answer: validate starts the work, a correction re-frames.
pub fn answer(store: &Store, id: &str, accept: bool, correction: &str) -> io::Result<()> {
    let request = store.load(id)?;
    if request["status"] != "question" {
        return Err(io::Error::other("cette demande n'attend pas de réponse"));
    }
    if accept {
        store.update(id, |r| r["status"] = json!("running"))?;
        spawn(id.to_string(), solve_now);
    } else {
        let correction = correction.trim();
        if correction.is_empty() {
            return Err(io::Error::other("dis ce qui ne va pas"));
        }
        // fennec-os feeds the correction to the next compile of this request.
        let worlds = Worlds::from_env()?;
        spec::reject(
            &worlds,
            request["spec"].as_str().unwrap_or(""),
            Some(correction),
        )?;
        store.update(id, |r| r["status"] = json!("framing"))?;
        frame(id.to_string());
    }
    Ok(())
}

/// A stopped agent's question goes to the human instead of a failure.
fn block(request: &mut Value, stage: &str, question: &str, why: &str) {
    request["status"] = json!("blocked");
    request["stage"] = json!(stage);
    request["question"] = json!(question);
    request["reason"] = json!(why);
}

/// The human's answer to a blocked request: recorded in fennec-os, then the
/// stopped step starts again with it.
pub fn reply(store: &Store, id: &str, text: &str) -> io::Result<()> {
    let request = store.load(id)?;
    if request["status"] != "blocked" {
        return Err(io::Error::other("cette demande n'attend pas de réponse"));
    }
    let text = text.trim();
    if text.is_empty() {
        return Err(io::Error::other("réponse vide"));
    }
    spec::answer(
        &Worlds::from_env()?,
        request["spec"].as_str().unwrap_or(""),
        request["question"].as_str().unwrap_or(""),
        text,
    )?;
    // ponytail: the step restarts from scratch with the answer; resuming the
    // stopped conversation would save its steps.
    if request["stage"] == "framing" {
        store.update(id, |r| r["status"] = json!("framing"))?;
        frame(id.to_string());
    } else {
        store.update(id, |r| r["status"] = json!("running"))?;
        spawn(id.to_string(), solve_now);
    }
    Ok(())
}

fn solve_now(store: &Store, id: &str) -> io::Result<()> {
    let request = store.load(id)?;
    let worlds = Worlds::from_env()?;
    let variants = OpenAi::variants_from_env(attempts())?;
    let models: Vec<&dyn Model> = variants.iter().map(|m| m as &dyn Model).collect();
    let record = spec::accept(
        &worlds,
        request["spec"].as_str().unwrap_or(""),
        &models,
        MAX_STEPS,
    )?;
    store.update(id, |r| apply_run(r, &record))?;
    let Some(run) = record["winner"].as_str() else {
        return Ok(());
    };
    // Tests passing is not "ready": the product is tried first.
    store.update(id, |r| r["status"] = json!("proving"))?;
    let proof = proof::gather(&worlds, run, &variants[0])?;
    store.update(id, |r| apply_proof(r, &proof))
}

fn apply_proof(request: &mut Value, proof: &Value) {
    request["proof"] = proof.clone();
    let failed = proof["tries"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|t| t["exit"] != 0);
    if let Some(t) = failed {
        request["status"] = json!("failed");
        request["reason"] = json!(format!(
            "les tests passent, mais l'essai a échoué : {}",
            t["command"].as_str().unwrap_or("")
        ));
    } else if let Some(s) = proof["shots"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|s| s["ok"] != true)
    {
        request["status"] = json!("failed");
        request["reason"] = json!(format!(
            "les tests passent, mais la page {} ne s'affiche pas : {}",
            s["url"].as_str().unwrap_or(""),
            s["reason"].as_str().unwrap_or("")
        ));
    } else if proof["passed"] == false {
        // Rule 3: tests alone do not make a request ready.
        request["status"] = json!("failed");
        request["reason"] =
            json!("les tests passent, mais fennec n'a pas su essayer le produit : aucune preuve");
    } else {
        request["status"] = json!("ready");
    }
}

fn apply_run(request: &mut Value, record: &Value) {
    if record["winner"].is_string() {
        request["status"] = json!("ready");
        request["run"] = record["winner"].clone();
    } else if let Some(t) = record["attempts"]
        .as_array()
        .and_then(|a| a.iter().find(|t| t["question"].is_string()))
    {
        block(
            request,
            "solving",
            t["question"].as_str().unwrap_or(""),
            t["reason"].as_str().unwrap_or(""),
        );
    } else {
        request["status"] = json!("failed");
        request["reason"] = record["attempts"]
            .as_array()
            .and_then(|a| a.iter().find_map(|t| t["reason"].as_str()))
            .map(|why| json!(why))
            .unwrap_or(json!("aucune tentative n'a passé les tests"));
    }
}

/// Work that was running when the server stopped will not resume: say so
/// instead of showing it in progress forever.
pub fn recover(store: &Store) -> io::Result<()> {
    for (id, r) in store.requests()? {
        if matches!(
            r["status"].as_str(),
            Some("framing" | "running" | "proving")
        ) {
            store.update(&id, |r| {
                r["status"] = json!("failed");
                r["reason"] = json!("interrompu : fennec a été arrêté pendant le travail");
            })?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stopped_attempt_with_a_question_asks_the_human() {
        let mut r = json!({"status": "running"});
        apply_run(
            &mut r,
            &json!({"winner": null, "attempts": [{"reason": "tests fail"}, {"reason": "stuck: x", "question": "Which port?"}]}),
        );
        assert_eq!(r["status"], "blocked");
        assert_eq!(r["stage"], "solving");
        assert_eq!(r["question"], "Which port?");
        assert_eq!(r["reason"], "stuck: x");
    }

    #[test]
    fn a_run_without_winner_fails_with_its_first_reason() {
        let mut r = json!({"status": "running"});
        apply_run(
            &mut r,
            &json!({"winner": null, "attempts": [{"passed": false}, {"reason": "tests fail"}]}),
        );
        assert_eq!(
            (r["status"].as_str(), r["reason"].as_str()),
            (Some("failed"), Some("tests fail"))
        );
        apply_run(&mut r, &json!({"winner": "w1", "attempts": []}));
        assert_eq!(
            (r["status"].as_str(), r["run"].as_str()),
            (Some("ready"), Some("w1"))
        );
    }

    #[test]
    fn one_failed_try_fails_the_request() {
        let mut r = json!({"status": "proving"});
        apply_proof(
            &mut r,
            &json!({"tries": [{"command": "a", "exit": 0}, {"command": "b", "exit": 1}]}),
        );
        assert_eq!(r["status"], "failed");
        assert!(r["reason"].as_str().unwrap().ends_with(": b"));
        apply_proof(&mut r, &json!({"tries": [{"command": "a", "exit": 0}]}));
        assert_eq!(r["status"], "ready");
        apply_proof(&mut r, &json!({"tries": [], "passed": false}));
        assert_eq!(r["status"], "failed");
        let blank = json!({"tries": [], "shots": [{"url": "u", "ok": false, "reason": "no text"}]});
        apply_proof(&mut r, &blank);
        assert_eq!(r["status"], "failed");
        assert!(
            r["reason"]
                .as_str()
                .unwrap()
                .ends_with("u ne s'affiche pas : no text")
        );
    }
}
