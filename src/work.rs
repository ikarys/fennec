//! A request's life, driven in the background: fennec-os compiles it into
//! "done when" (goal + failing tests), the human validates or corrects once,
//! then fennec-os solves it. Each step runs on its own thread; the request
//! file is the only state, so the view reads it like any other.

use std::io;
use std::path::Path;

use fennec_os::provider::{Live, Model, OpenAi};
use fennec_os::world::{self, Worlds};
use fennec_os::{proof, review, rules, solve, spec};
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
        // The roles the human left out of this request stay out of every step.
        let off: Vec<String> = store
            .load(&id)
            .map(|r| {
                r["off"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|k| k.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        if let Err(e) = fennec_os::team::skipping(&off, || step(&store, &id)) {
            let _ = store.update(&id, |r| {
                r["status"] = json!("failed");
                r["reason"] = json!(e.to_string());
            });
        }
        // What the step cost, kept for the card and "Stats": the journal
        // reads every record, too slow for each refresh of the page.
        if let Ok(s) = stats(&store, &id) {
            let _ = store.update(&id, |r| r["stats"] = s);
        }
    });
}

pub fn frame(id: String) {
    spawn(id, frame_now);
}

/// One turn of the conversation that shapes a request: fennec answers the
/// human, or, once the request is clear, frames it.
pub fn talk(id: String) {
    spawn(id, talk_now);
}

fn say_in(request: &mut Value, who: &str, text: &str) {
    if !request["thread"].is_array() {
        request["thread"] = json!([]);
    }
    if let Some(t) = request["thread"].as_array_mut() {
        t.push(json!({"who": who, "text": text}));
    }
}

/// Each step the request went through, pointing at what fennec-os keeps of
/// it; `journal` turns these into what the human reads.
fn note(request: &mut Value, step: Value) {
    if !request["journal"].is_array() {
        request["journal"] = json!([]);
    }
    if let Some(j) = request["journal"].as_array_mut() {
        j.push(step);
    }
}

/// The model a stage runs on: the one a role of the stage names, else the
/// settings' (temperature kept, so attempts still differ).
fn staffed(mut model: OpenAi, project: &Path, stage: &str) -> io::Result<OpenAi> {
    if let Some(m) = fennec_os::team::model(project, stage)? {
        model.model = m;
    }
    Ok(model)
}

fn talk_now(store: &Store, id: &str) -> io::Result<()> {
    let request = store.load(id)?;
    let project = Path::new(request["project"].as_str().unwrap_or(""));
    // The conversation is framing's first part: it runs on framing's model.
    let model = staffed(OpenAi::variants_from_env(1)?.remove(0), project, "frame")?;
    offer_rule(store, id, &request, &model)?;
    let thread = request["thread"].as_array().cloned().unwrap_or_default();
    let (said, exchange) = spec::discuss(project, &thread, &model)?;
    // ponytail: each turn keeps the whole exchange, the thread resent included;
    // a long conversation could keep only the reply.
    let step = json!({"stage": "talk", "exchange": exchange});
    match said {
        spec::Discussed::Reply(text) => store.update(id, |r| {
            note(r, step);
            say_in(r, "fennec", &text);
            r["status"] = json!("talk");
        }),
        spec::Discussed::Brief(brief) => {
            store.update(id, |r| {
                note(r, step);
                r["brief"] = json!(brief);
                r["status"] = json!("framing");
            })?;
            frame_now(store, id)
        }
    }
}

/// The human speaks in the conversation: an answer, a precision, or a
/// correction of the proposed "done when", which is then dropped.
pub fn say(store: &Store, id: &str, text: &str) -> io::Result<()> {
    let request = store.load(id)?;
    let open = match request["status"].as_str() {
        Some("talk" | "question") => true,
        Some("blocked") => request["stage"] == "framing",
        _ => false,
    };
    if !open {
        return Err(io::Error::other("cette demande n'attend pas de réponse"));
    }
    let text = text.trim();
    if text.is_empty() {
        return Err(io::Error::other("message vide"));
    }
    if request["status"] == "question" {
        spec::reject(
            &Worlds::from_env()?,
            request["spec"].as_str().unwrap_or(""),
            Some(text),
        )?;
    }
    store.update(id, |r| {
        say_in(r, "human", text);
        r["status"] = json!("thinking");
        r["heard"] = json!(text);
        if let Some(m) = r.as_object_mut() {
            m.remove("done_when");
            m.remove("brief");
        }
    })?;
    talk(id.to_string());
    Ok(())
}

/// "Cadrer maintenant": the human finds it clear enough; the whole
/// conversation becomes the brief.
pub fn frame_anyway(store: &Store, id: &str) -> io::Result<()> {
    let request = store.load(id)?;
    if request["status"] != "talk" {
        return Err(io::Error::other("cette demande n'est pas en discussion"));
    }
    let brief = transcript(&request);
    store.update(id, |r| {
        r["brief"] = json!(brief);
        r["done_when"] = json!(human_words(&request));
        r["status"] = json!("framing");
    })?;
    frame(id.to_string());
    Ok(())
}

/// What the human reads as "done when" when they started the work
/// themselves: their own words, not fennec's.
fn human_words(request: &Value) -> String {
    let replies = request["thread"].as_array().into_iter().flatten().skip(1);
    std::iter::once(&request["text"])
        .chain(replies.filter(|t| t["who"] == "human").map(|t| &t["text"]))
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join(" — ")
}

fn transcript(request: &Value) -> String {
    let mut text = request["text"].as_str().unwrap_or("").to_string();
    let thread = request["thread"].as_array().cloned().unwrap_or_default();
    if thread.len() > 1 {
        text.push_str("\n\nAgreed in conversation:\n");
        for t in &thread[1..] {
            let who = if t["who"] == "human" {
                "human"
            } else {
                "fennec"
            };
            text.push_str(&format!("- {who}: {}\n", t["text"].as_str().unwrap_or("")));
        }
    }
    text
}

/// Once the request is clear, the work starts: no tests are written first
/// (that is a rule the human may say, not a stage); the proof of the product
/// is what makes it ready.
fn frame_now(store: &Store, id: &str) -> io::Result<()> {
    let request = store.load(id)?;
    let project = Path::new(request["project"].as_str().unwrap_or(""));
    crate::state::start(project)?;
    let brief = request["brief"]
        .as_str()
        .or(request["text"].as_str())
        .unwrap_or("")
        .to_string();
    let s = spec::direct(&Worlds::from_env()?, project, &brief)?;
    // "Done when" stays unset: the request already says it, and the human
    // would read it twice. The proof falls back to the brief.
    store.update(id, |r| {
        r["spec"] = s["world"].clone();
        r["status"] = json!("running");
    })?;
    solve_now(store, id)
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
        store.update(id, |r| {
            r["status"] = json!("framing");
            r["heard"] = json!(correction);
        })?;
        frame(id.to_string());
    }
    Ok(())
}

/// What the human just said (a correction, an answer) may hold beyond this
/// request: if the model thinks so, it is offered as a rule in "À moi". The
/// human decides; a model error only means no offer.
fn offer_rule(store: &Store, id: &str, request: &Value, model: &dyn Model) -> io::Result<()> {
    let Some(heard) = request["heard"].as_str() else {
        return Ok(());
    };
    let project = Path::new(request["project"].as_str().unwrap_or(""));
    let known = rules::said(project).unwrap_or_default();
    let offer = rules::propose(model, request["text"].as_str().unwrap_or(""), heard)
        .ok()
        .flatten()
        .filter(|rule| !known.iter().any(|k| &k.text == rule));
    store.update(id, |r| {
        if let Some(m) = r.as_object_mut() {
            m.remove("heard");
        }
        if let Some(rule) = offer {
            r["offer"] = json!({"rule": rule, "heard": heard});
        }
    })
}

/// The human's one word on an offered rule: `user` (every project),
/// `project`, or `no`.
pub fn rule(store: &Store, id: &str, scope: &str) -> io::Result<()> {
    let request = store.load(id)?;
    let Some(text) = request["offer"]["rule"].as_str() else {
        return Err(io::Error::other("aucune règle proposée"));
    };
    match scope {
        "user" | "project" => rules::say(
            Path::new(request["project"].as_str().unwrap_or("")),
            scope,
            text,
        )?,
        "no" => {}
        _ => return Err(io::Error::other("choix inconnu")),
    }
    store.update(id, |r| {
        if let Some(m) = r.as_object_mut() {
            m.remove("offer");
        }
    })
}

/// A stopped agent's question goes to the human instead of a failure.
fn block(request: &mut Value, stage: &str, question: &str, why: &str) {
    // In the conversation too: the whole story reads in one place.
    say_in(request, "fennec", question);
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
    store.update(id, |r| say_in(r, "human", text))?;
    // ponytail: the step restarts from scratch with the answer; resuming the
    // stopped conversation would save its steps.
    if request["stage"] == "framing" {
        store.update(id, |r| {
            r["status"] = json!("framing");
            r["heard"] = json!(text);
        })?;
        frame(id.to_string());
    } else {
        store.update(id, |r| {
            r["status"] = json!("running");
            r["heard"] = json!(text);
        })?;
        spawn(id.to_string(), solve_now);
    }
    Ok(())
}

/// Projects whose folder a request just took: two waiting requests must not
/// both see it free. Held while the work runs; by then the folder says it.
static TAKEN: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

struct Taken(String);

impl Drop for Taken {
    fn drop(&mut self) {
        if let Ok(mut t) = TAKEN.lock() {
            t.retain(|p| p != &self.0);
        }
    }
}

/// When fennec works in the project folder itself, one request at a time:
/// the next one waits its turn ("waiting") instead of failing.
/// ponytail: polls every 3 s and the first to look goes first, not the oldest.
fn wait_turn(store: &Store, id: &str, project: &str) -> io::Result<Taken> {
    let mine = store.load(id)?["run"].as_str().map(str::to_string);
    loop {
        let held = match world::held(Path::new(project))? {
            // Its own last try still holds the folder: retrying drops it.
            Some(world::Held::World(w)) if Some(&w) == mine.as_ref() => None,
            Some(world::Held::World(w)) => Some(
                store
                    .requests()?
                    .into_iter()
                    .find(|(_, r)| r["run"] == w.as_str())
                    .map(|(_, r)| format!("« {} »", r["text"].as_str().unwrap_or("")))
                    .unwrap_or_else(|| "une autre demande".to_string()),
            ),
            Some(world::Held::Changes) => Some(String::new()),
            None => None,
        };
        let mut taken = TAKEN.lock().map_err(|_| io::Error::other("lock"))?;
        let behind = match held {
            Some(by) => Some(by),
            None if taken.iter().any(|p| p == project) => Some("une autre demande".to_string()),
            None => {
                taken.push(project.to_string());
                if store.load(id)?["status"] == "waiting" {
                    store.update(id, |r| {
                        r["status"] = json!("running");
                        if let Some(m) = r.as_object_mut() {
                            m.remove("behind");
                        }
                    })?;
                }
                return Ok(Taken(project.to_string()));
            }
        };
        std::mem::drop(taken);
        store.update(id, |r| {
            r["status"] = json!("waiting");
            r["behind"] = json!(behind);
        })?;
        std::thread::sleep(std::time::Duration::from_secs(3));
    }
}

fn solve_now(store: &Store, id: &str) -> io::Result<()> {
    let request = store.load(id)?;
    let worlds = Worlds::from_env()?;
    let project = Path::new(request["project"].as_str().unwrap_or(""));
    // A correction works on the ready work, which already holds the folder.
    let _turn = match request["amend"].is_string() {
        true => None,
        false => Some(wait_turn(
            store,
            id,
            request["project"].as_str().unwrap_or(""),
        )?),
    };
    let variants = OpenAi::variants_from_env(attempts())?
        .into_iter()
        .map(|m| staffed(m, project, "solve"))
        .collect::<io::Result<Vec<_>>>()?;
    offer_rule(store, id, &request, &variants[0])?;
    let live: Vec<Live> = variants
        .iter()
        .map(|m| Ok(Live::new(m, store.live_path(id)?)))
        .collect::<io::Result<_>>()?;
    let models: Vec<&dyn Model> = live.iter().map(|m| m as &dyn Model).collect();
    let spec_id = request["spec"].as_str().unwrap_or("");
    let amend = request["amend"].as_str();
    let record = match (amend, request["retry"].as_str()) {
        (Some(change), _) => spec::amend(&worlds, spec_id, change, &models, MAX_STEPS)?,
        (None, Some(why)) => spec::retry(&worlds, spec_id, why, &models, MAX_STEPS)?,
        (None, None) => spec::accept(&worlds, spec_id, &models, MAX_STEPS)?,
    };
    let tests = spec::load(&worlds, spec_id)?;
    if amend.is_some() && !record["winner"].is_string() {
        // The change did not pass: the ready work is still there, untouched.
        return store.update(id, |r| {
            if let Some(m) = r.as_object_mut() {
                m.remove("amend");
            }
            note(r, json!({"stage": "solve", "run": record["id"]}));
            r["status"] = json!("ready");
            r["amend_error"] = json!(failure(&record));
        });
    }
    store.update(id, |r| {
        let before = r["files"].clone();
        if let Some(m) = r.as_object_mut() {
            m.remove("retry");
            m.remove("amend");
        }
        note(r, json!({"stage": "solve", "run": record["id"]}));
        apply_run(r, &record);
        // A correction starts from the ready work: its files are still part of it.
        if amend.is_some() {
            let mut files = before.as_array().cloned().unwrap_or_default();
            for f in r["files"].as_array().into_iter().flatten() {
                if !files.contains(f) {
                    files.push(f.clone());
                }
            }
            r["files"] = json!(files);
        }
        // The acceptance tests land in the same commit: they are part of what fennec did.
        if let Some(files) = r["files"].as_array_mut() {
            for t in changed_files(tests["diff"].as_str().unwrap_or(""))
                .into_iter()
                .rev()
            {
                if !files.contains(&json!(t)) {
                    files.insert(0, json!(t));
                }
            }
        }
    })?;
    let Some(run) = record["winner"].as_str() else {
        return Ok(());
    };
    // Tests passing is not "ready": the product is tried first.
    store.update(id, |r| r["status"] = json!("proving"))?;
    let r = store.load(id)?;
    let done_when = [&r["done_when"], &r["brief"], &r["text"]]
        .into_iter()
        .find_map(Value::as_str)
        .map(str::to_string);
    let prover = staffed(OpenAi::variants_from_env(1)?.remove(0), project, "prove")?;
    let watched = Live::new(&prover, store.live_path(id)?);
    let proof = proof::gather(&worlds, run, done_when.as_deref(), &watched)?;
    // Where it will land is decided before "ready", so the card never shows
    // a branch the click would not use.
    let mut probe = store.load(id)?;
    apply_proof(&mut probe, &proof);
    let target = (probe["status"] == "ready")
        .then(|| solve::plan(&worlds, run, &variants[0]).ok())
        .flatten();
    store.update(id, |r| {
        apply_proof(r, &proof);
        if let Some(t) = &target {
            r["target"] = target_json(t);
        }
    })
}

fn target_json(t: &solve::Landing) -> Value {
    json!({"branch": t.branch, "message": t.message, "remote": t.remote})
}

/// "Réessayer" on a failed request: solved again from its validated tests
/// when there are some, told why the last try failed; framed again when it
/// failed before "done when" was validated.
pub fn retry(store: &Store, id: &str) -> io::Result<()> {
    let request = store.load(id)?;
    if request["status"] != "failed" {
        return Err(io::Error::other("cette demande n'a pas échoué"));
    }
    let validated = match request["spec"].as_str() {
        Some(s) => matches!(
            spec::load(&Worlds::from_env()?, s)?["status"].as_str(),
            Some("proposed" | "accepted")
        ),
        None => false,
    };
    let why = request["reason"].as_str().unwrap_or("").to_string();
    store.update(id, |r| {
        if let Some(m) = r.as_object_mut() {
            m.remove("proof");
            m.remove("reason");
        }
        if validated {
            r["status"] = json!("running");
            r["retry"] = json!(why);
        } else {
            r["status"] = json!("framing");
        }
    })?;
    if validated {
        spawn(id.to_string(), solve_now);
    } else {
        frame(id.to_string());
    }
    Ok(())
}

/// The roles that worked on a request, each named once, in the order they
/// came in: framing, solving, proving.
fn add_team(request: &mut Value, names: &Value) {
    let mut team: Vec<Value> = request["team"].as_array().cloned().unwrap_or_default();
    for name in names.as_array().into_iter().flatten() {
        if !team.contains(name) {
            team.push(name.clone());
        }
    }
    request["team"] = json!(team);
}

fn apply_proof(request: &mut Value, proof: &Value) {
    request["proof"] = proof.clone();
    // The exchange is read from the run record, by the journal.
    if let Some(p) = request["proof"].as_object_mut() {
        p.remove("exchange");
    }
    add_team(request, &proof["team"]);
    let failed = proof["tries"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|t| t["exit"] != 0);
    // A role's check is its veto; a tool not installed is only unverified.
    let vetoed = proof["checks"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|c| c["exit"] != 0 && c["missing"] != true);
    if let Some(t) = failed {
        request["status"] = json!("failed");
        request["reason"] = json!(format!(
            "le travail est fait, mais l'essai a échoué : {}",
            t["command"].as_str().unwrap_or("")
        ));
    } else if let Some(c) = vetoed {
        request["status"] = json!("failed");
        request["reason"] = json!(format!(
            "le travail est fait, mais {} bloque : `{}` a échoué",
            c["role"].as_str().unwrap_or(""),
            c["command"].as_str().unwrap_or("")
        ));
    } else if let Some(s) = proof["shots"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|s| s["ok"] != true && s["missing"] != true)
    {
        request["status"] = json!("failed");
        request["reason"] = json!(format!(
            "le travail est fait, mais la page {} ne s'affiche pas : {}",
            s["url"].as_str().unwrap_or(""),
            s["reason"].as_str().unwrap_or("")
        ));
    } else if proof["passed"] == false {
        // Rule 3: tests alone do not make a request ready.
        let blind = proof["shots"]
            .as_array()
            .is_some_and(|s| s.iter().any(|s| s["missing"] == true));
        request["status"] = json!("failed");
        request["reason"] = json!(if blind {
            "le travail est fait, mais fennec n'a pas pu regarder la page : aucun navigateur n'est installé (chromium)"
        } else {
            "le travail est fait, mais fennec n'a pas su essayer le produit : aucune preuve"
        });
    } else {
        request["status"] = json!("ready");
    }
}

/// The paths a diff touches, in order.
fn changed_files(diff: &str) -> Vec<&str> {
    diff.lines()
        .filter_map(|l| l.strip_prefix("diff --git a/"))
        .filter_map(|l| l.split(" b/").next())
        .collect()
}

fn apply_run(request: &mut Value, record: &Value) {
    // The rules each step says it applied: framing's, then the solution's.
    let mut used: Vec<Value> = request["rules"].as_array().cloned().unwrap_or_default();
    let solving = record["attempts"]
        .as_array()
        .and_then(|a| {
            a.iter().find(|t| {
                (record["winner"].is_string() && t["world"] == record["winner"])
                    || t["question"].is_string()
            })
        })
        .and_then(|t| t["rules"].as_array());
    for rule in solving.into_iter().flatten() {
        if !used.contains(rule) {
            used.push(rule.clone());
        }
    }
    request["rules"] = json!(used);
    add_team(request, &record["team"]);
    if record["winner"].is_string() {
        request["status"] = json!("ready");
        request["run"] = record["winner"].clone();
        // What fennec did, kept with the request: the run record is internal.
        if let Some(t) = record["attempts"]
            .as_array()
            .and_then(|a| a.iter().find(|t| t["world"] == record["winner"]))
        {
            request["summary"] = t["summary"].clone();
            request["files"] = json!(changed_files(t["diff"].as_str().unwrap_or("")));
        }
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
        request["reason"] = json!(failure(record));
    }
}

/// Why a run has no winner: its first attempt's reason.
fn failure(record: &Value) -> String {
    record["attempts"]
        .as_array()
        .and_then(|a| a.iter().find_map(|t| t["reason"].as_str()))
        .unwrap_or("aucune tentative n'a abouti")
        .to_string()
}

/// "Corriger" on a ready request: the human tried it and asks for a change.
/// fennec starts from the ready work, not from scratch, then proves it again.
pub fn amend(store: &Store, id: &str, text: &str) -> io::Result<()> {
    let request = store.load(id)?;
    if request["status"] != "ready" || request["commit"].is_string() {
        return Err(io::Error::other("cette demande n'attend pas ta décision"));
    }
    let text = text.trim();
    if text.is_empty() {
        return Err(io::Error::other("dis ce qui ne va pas"));
    }
    store.update(id, |r| {
        say_in(r, "human", text);
        r["status"] = json!("running");
        r["amend"] = json!(text);
        r["heard"] = json!(text);
        if let Some(m) = r.as_object_mut() {
            m.remove("amend_error");
            m.remove("land_error");
        }
    })?;
    spawn(id.to_string(), solve_now);
    Ok(())
}

/// "Intégrer" on a ready request: its work becomes one commit on the branch
/// shown on the card, with the message shown there, then that branch is
/// pushed. Without a branch on the card yet, the click only decides it. A landing that fails leaves the request ready, with the reason;
/// once committed, a new click only retries the push.
/// The human gives up on a request. Only one waiting for them: work in
/// progress would write its result over the decision.
pub fn drop(store: &Store, id: &str) -> io::Result<()> {
    let status = store.load(id)?["status"].as_str().unwrap_or("").to_string();
    if !matches!(status.as_str(), "question" | "blocked" | "ready" | "failed") {
        return Err(io::Error::other(
            "fennec travaille encore sur cette demande",
        ));
    }
    // Its work goes too: in the project folder, the folder comes back.
    let request = store.load(id)?;
    let worlds = Worlds::from_env()?;
    if let Some(run) = request["run"].as_str()
        && worlds.open(run).is_ok()
    {
        review::reject(&worlds, run, None)?;
    }
    if let Some(s) = request["spec"].as_str()
        && worlds.open(s).is_ok()
    {
        spec::reject(&worlds, s, None)?;
    }
    store.update(id, |r| {
        r["status"] = json!("dropped");
        if let Some(m) = r.as_object_mut() {
            m.remove("offer");
        }
    })
}

/// "Passer": the work is set aside, the project untouched. The world stays,
/// so the human can come back to it and commit later.
pub fn pass(store: &Store, id: &str) -> io::Result<()> {
    if store.load(id)?["status"] != "ready" {
        return Err(io::Error::other("cette demande n'attend pas ta décision"));
    }
    store.update(id, |r| r["status"] = json!("passed"))
}

/// Back from the history to the human's decisions.
pub fn resume(store: &Store, id: &str) -> io::Result<()> {
    if store.load(id)?["status"] != "passed" {
        return Err(io::Error::other("cette demande n'a pas été mise de côté"));
    }
    store.update(id, |r| r["status"] = json!("ready"))
}

/// Opens `folder` in the human's editor; false when there is none.
pub fn editor(folder: &Path) -> bool {
    // ponytail: VS Code only (`code`, which WSL hands to Windows); another
    // editor would be a setting.
    match std::process::Command::new("code")
        .arg(&folder)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(mut child) => {
            std::thread::spawn(move || child.wait());
            true
        }
        Err(_) => false,
    }
}

/// "Ouvrir le code": the ready work is a real folder, opened in the human's
/// editor. They may edit it; the commit runs its checks again
/// (`solve::land`).
pub fn open(store: &Store, id: &str) -> io::Result<Value> {
    let request = store.load(id)?;
    let run = request["run"]
        .as_str()
        .filter(|_| request["status"] == "ready")
        .ok_or_else(|| io::Error::other("cette demande n'a pas de travail prêt"))?;
    let folder = Worlds::from_env()?.open(run)?.path;
    let opened = editor(&folder);
    Ok(json!({"folder": folder, "opened": opened}))
}

/// What fennec changed for a request, read from its run: asked for when the
/// human opens it, since a diff is too large for every refresh of the page.
pub fn changes(store: &Store, id: &str) -> io::Result<Value> {
    let request = store.load(id)?;
    let run = request["run"]
        .as_str()
        .ok_or_else(|| io::Error::other("fennec n'a rien changé pour cette demande"))?;
    let record = review::load(&Worlds::from_env()?, run)?;
    let attempt = record["attempts"]
        .as_array()
        .and_then(|a| a.iter().find(|t| t["world"] == record["winner"]))
        .cloned()
        .unwrap_or_default();
    // The acceptance tests come first: they land in the same commit.
    let tests = match request["spec"].as_str() {
        Some(s) => spec::load(&Worlds::from_env()?, s)?["diff"]
            .as_str()
            .unwrap_or("")
            .to_string(),
        None => String::new(),
    };
    let diff = &[tests.as_str(), attempt["diff"].as_str().unwrap_or("")].join("\n");
    // ponytail: a cut at 300 kB; a huge change is read in the project.
    const MAX: usize = 300_000;
    let cut = diff.len() > MAX;
    let shown = &diff[..diff.floor_char_boundary(MAX.min(diff.len()))];
    Ok(
        json!({"summary": attempt["summary"], "files": changed_files(diff), "diff": shown, "cut": cut}),
    )
}

/// Every step of a request, in order, with what was sent to the model and
/// what came back, and what it cost.
pub fn journal(store: &Store, id: &str) -> io::Result<Value> {
    let request = store.load(id)?;
    let worlds = Worlds::from_env()?;
    let mut entries = request["journal"].as_array().cloned().unwrap_or_default();
    // A request from before the journal: what it still points to.
    if entries.is_empty() {
        if let Some(s) = request["spec"].as_str() {
            entries.push(json!({"stage": "frame", "spec": s}));
        }
        if let Some(r) = request["run"].as_str() {
            entries.push(json!({"stage": "solve", "run": r}));
        }
    }
    let mut steps = Vec::new();
    for e in &entries {
        match e["stage"].as_str() {
            Some("talk") => steps.push(roles(
                step("Discussion", &e["exchange"]),
                &e["exchange"]["team"],
            )),
            Some("frame") => {
                if let Ok(s) = spec::load(&worlds, e["spec"].as_str().unwrap_or("")) {
                    steps.push(roles(
                        step("Cadrage : terminé quand et tests", &s),
                        &s["team"],
                    ));
                }
            }
            Some("solve") => {
                let Ok(r) = review::load(&worlds, e["run"].as_str().unwrap_or("")) else {
                    continue;
                };
                for (n, a) in r["attempts"].as_array().into_iter().flatten().enumerate() {
                    let mut s = roles(step(&format!("Travail, essai {}", n + 1), a), &r["team"]);
                    s["passed"] = a["passed"].clone();
                    steps.push(s);
                }
                if !r["proof"]["exchange"].is_null() {
                    let mut s = roles(step("Preuve", &r["proof"]["exchange"]), &r["proof"]["team"]);
                    // A prove role also acts on its own: its commands, run by fennec.
                    s["checks"] = r["proof"]["checks"].clone();
                    steps.push(s);
                }
            }
            _ => {}
        }
    }
    // The step under way, as far as it has gone.
    let under_way = match request["status"].as_str() {
        Some("framing") => Some("Démarrage (en cours)"),
        Some("running") => Some("Travail (en cours)"),
        Some("proving") => Some("Preuve (en cours)"),
        _ => None,
    };
    if let Some(name) = under_way {
        let now = std::fs::read_to_string(store.live_path(id)?)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok());
        let mut s = match &now {
            Some(now) => step(name, now),
            None => json!({"name": name, "messages": [], "cost": cost(&Value::Null)}),
        };
        s["live"] = json!(true);
        steps.push(s);
    }
    Ok(json!(steps))
}

/// The roles that took part in a step, each with what it told the model, as
/// it was sent: read back from the step's system message.
/// ponytail: a brief ends at its first blank line, as the catalogue's do; a
/// role written in several paragraphs shows its first one.
fn roles(mut step: Value, team: &Value) -> Value {
    let system = step["messages"][0]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let roles: Vec<Value> = team
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(|name| {
            let head = format!("As {name} on the team:\n");
            let brief = system
                .split_once(&head)
                .map(|(_, rest)| rest.split("\n\n").next().unwrap_or("").trim().to_string());
            json!({"name": name, "brief": brief})
        })
        .collect();
    step["roles"] = json!(roles);
    step
}

fn step(name: &str, from: &Value) -> Value {
    json!({"name": name, "model": from["model"], "messages": from["messages"],
           "cost": cost(&from["turns"])})
}

/// Tokens and model time summed over the turns of one step.
fn cost(turns: &Value) -> Value {
    let turns: Vec<&Value> = turns.as_array().into_iter().flatten().collect();
    let sum =
        |f: fn(&Value) -> &Value| -> u64 { turns.iter().map(|t| f(t).as_u64().unwrap_or(0)).sum() };
    json!({
        "turns": turns.len(),
        "sent": sum(|t| &t["usage"]["prompt_tokens"]),
        "cached": sum(|t| &t["usage"]["prompt_tokens_details"]["cached_tokens"]),
        "received": sum(|t| &t["usage"]["completion_tokens"]),
        "thinking": sum(|t| &t["usage"]["completion_tokens_details"]["reasoning_tokens"]),
        "ms": sum(|t| &t["model_ms"]),
    })
}

const STATS_VERSION: u64 = 2;
const COUNTS: [&str; 6] = ["turns", "sent", "cached", "received", "thinking", "ms"];

/// A request's cost summed over its finished steps, in all and per model,
/// and how many attempts failed (each one undone).
pub fn stats(store: &Store, id: &str) -> io::Result<Value> {
    Ok(totals(
        journal(store, id)?
            .as_array()
            .map_or(&[][..], Vec::as_slice),
    ))
}

fn totals(steps: &[Value]) -> Value {
    // `v`: stats kept by an older fennec are computed again (see `overview`).
    let mut all = json!({"v": STATS_VERSION, "failed_attempts": 0, "models": {}, "stages": {}});
    let add = |into: &mut Value, cost: &Value| {
        for k in COUNTS {
            into[k] = json!(into[k].as_u64().unwrap_or(0) + cost[k].as_u64().unwrap_or(0));
        }
    };
    for s in steps.iter().filter(|s| s["live"] != true) {
        add(&mut all, &s["cost"]);
        let model = s["model"].as_str().unwrap_or("?");
        if all["models"][model].is_null() {
            all["models"][model] = json!({});
        }
        add(&mut all["models"][model], &s["cost"]);
        // "Travail, essai 2" counts as "Travail".
        let stage = s["name"]
            .as_str()
            .unwrap_or("?")
            .split(',')
            .next()
            .unwrap_or("?");
        if all["stages"][stage].is_null() {
            all["stages"][stage] = json!({});
        }
        add(&mut all["stages"][stage], &s["cost"]);
        for r in s["roles"].as_array().into_iter().flatten() {
            let list = &mut all["stages"][stage]["roles"];
            if !list.as_array().is_some_and(|l| l.contains(&r["name"])) {
                if !list.is_array() {
                    *list = json!([]);
                }
                list.as_array_mut()
                    .into_iter()
                    .for_each(|l| l.push(r["name"].clone()));
            }
        }
        if s["passed"] == false {
            all["failed_attempts"] = json!(all["failed_attempts"].as_u64().unwrap_or(0) + 1);
        }
    }
    all
}

/// "Stats": every request's cost summed per project and per model, with where
/// fennec works in each project. A request from before stats were kept gets
/// them now, once.
pub fn overview(store: &Store) -> io::Result<Value> {
    let mut requests = store.requests()?;
    for (id, r) in &mut requests {
        // Settled requests only: one under way is written by its own step.
        let settled = matches!(
            r["status"].as_str(),
            Some("ready" | "failed" | "landed" | "passed" | "dropped")
        );
        if settled
            && r["stats"]["v"] != STATS_VERSION
            && (r["spec"].is_string() || r["journal"].is_array())
        {
            r["stats"] = stats(store, id)?;
            store.update(id, |saved| saved["stats"] = r["stats"].clone())?;
        }
    }
    let places = store
        .projects()?
        .into_iter()
        .map(|p| {
            let place = fennec_os::world::workspace(Path::new(&p));
            (p, place)
        })
        .collect::<Vec<_>>();
    Ok(summarize(&requests, &places))
}

fn summarize(requests: &[(String, Value)], places: &[(String, String)]) -> Value {
    let mut projects: Vec<Value> = places
        .iter()
        .map(|(path, place)| {
            let name = Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(path);
            json!({"path": path, "name": name, "place": place,
                   "requests": 0, "landed": 0, "failed": 0, "failed_attempts": 0})
        })
        .collect();
    let (mut models, mut stages) = (json!({}), json!({}));
    let mut all = json!({"requests": 0});
    let bump = |v: &mut Value, k: &str, n: u64| v[k] = json!(v[k].as_u64().unwrap_or(0) + n);
    for (_, r) in requests {
        let s = &r["stats"];
        bump(&mut all, "requests", 1);
        for k in COUNTS {
            bump(&mut all, k, s[k].as_u64().unwrap_or(0));
        }
        if let Some(p) = projects.iter_mut().find(|p| p["path"] == r["project"]) {
            bump(p, "requests", 1);
            bump(p, "landed", u64::from(r["status"] == "landed"));
            bump(p, "failed", u64::from(r["status"] == "failed"));
            bump(
                p,
                "failed_attempts",
                s["failed_attempts"].as_u64().unwrap_or(0),
            );
            for k in COUNTS {
                bump(p, k, s[k].as_u64().unwrap_or(0));
            }
        }
        for (key, into) in [("models", &mut models), ("stages", &mut stages)] {
            for (name, cost) in s[key].as_object().into_iter().flatten() {
                if into[name].is_null() {
                    into[name] = json!({"name": name});
                }
                for k in COUNTS {
                    bump(&mut into[name], k, cost[k].as_u64().unwrap_or(0));
                }
            }
        }
    }
    let list = |v: Value| -> Vec<Value> {
        v.as_object()
            .into_iter()
            .flatten()
            .map(|(_, m)| m.clone())
            .collect()
    };
    json!({"all": all, "projects": projects, "models": list(models), "stages": list(stages)})
}

pub fn land(store: &Store, id: &str) -> io::Result<()> {
    let request = store.load(id)?;
    if request["status"] != "ready" || !request["run"].is_string() {
        return Err(io::Error::other(
            "cette demande n'est pas prête pour un commit",
        ));
    }
    store.update(id, |r| {
        r["status"] = json!("landing");
        if let Some(m) = r.as_object_mut() {
            m.remove("land_error");
        }
    })?;
    spawn(id.to_string(), land_now);
    Ok(())
}

fn land_now(store: &Store, id: &str) -> io::Result<()> {
    let request = store.load(id)?;
    let run = request["run"].as_str().unwrap_or("");
    let project = Path::new(request["project"].as_str().unwrap_or(""));
    let worlds = Worlds::from_env()?;
    let target = request["target"].clone();
    if !target["branch"].is_string() {
        // Nothing lands where the human has not seen it: this click only
        // decides the branch and message, shown for the next one.
        return match OpenAi::variants_from_env(1)
            .and_then(|mut m| solve::plan(&worlds, run, &m.remove(0)))
        {
            Ok(t) => store.update(id, |r| {
                r["status"] = json!("ready");
                r["target"] = target_json(&t);
            }),
            Err(e) => fail_landing(store, id, e),
        };
    }
    let branch = target["branch"].as_str().unwrap_or("");
    if !request["commit"].is_string() {
        match solve::land(
            &worlds,
            run,
            Some(branch).filter(|b| !b.is_empty()),
            target["message"].as_str(),
        ) {
            Ok(commit) => store.update(id, |r| r["commit"] = json!(commit))?,
            Err(e) => return fail_landing(store, id, e),
        }
    }
    if let Some(remote) = target["remote"].as_str()
        && let Err(e) = solve::push(project, remote, branch)
    {
        return store.update(id, |r| {
            r["status"] = json!("ready");
            r["land_error"] = json!(format!("le commit est fait, mais pas poussé : {e}"));
        });
    }
    store.update(id, |r| r["status"] = json!("landed"))
}

fn fail_landing(store: &Store, id: &str, e: io::Error) -> io::Result<()> {
    let mut why = e.to_string();
    if why.contains(" after edits onto ") {
        why = format!(
            "tes modifications font échouer les tests : corrige-les dans le dossier du code, puis refais le commit\n{why}"
        );
    }
    store.update(id, |r| {
        r["status"] = json!("ready");
        r["land_error"] = json!(why);
    })
}

/// Work that was running when the server stopped will not resume: say so
/// instead of showing it in progress forever.
pub fn recover(store: &Store) -> io::Result<()> {
    for (id, r) in store.requests()? {
        if matches!(
            r["status"].as_str(),
            Some("thinking" | "framing" | "running" | "proving")
        ) {
            store.update(&id, |r| {
                r["status"] = json!("failed");
                r["reason"] = json!("interrompu : fennec a été arrêté pendant le travail");
            })?;
        } else if r["status"] == "waiting" {
            // Nothing started yet: it waits its turn again.
            spawn(id, solve_now);
        } else if r["status"] == "landing" {
            store.update(&id, |r| {
                r["status"] = json!("ready");
                r["land_error"] = json!("interrompu : fennec a été arrêté pendant le commit");
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
        assert_eq!(
            r["thread"],
            json!([{"who": "fennec", "text": "Which port?"}])
        );
    }

    #[test]
    fn rules_used_while_framing_and_solving_are_kept_once() {
        let mut r = json!({"status": "running", "rules": ["use uv"]});
        apply_run(
            &mut r,
            &json!({"winner": "w2", "attempts": [{"world": "w1", "rules": ["x"]}, {"world": "w2", "rules": ["use uv", "short output"]}]}),
        );
        assert_eq!(r["rules"], json!(["use uv", "short output"]));
    }

    #[test]
    fn the_conversation_becomes_the_brief() {
        let r = json!({"text": "un test", "thread": [
            {"who": "human", "text": "un test"},
            {"who": "fennec", "text": "Un script ou une page ?"},
            {"who": "human", "text": "un script python"}]});
        assert_eq!(
            transcript(&r),
            "un test\n\nAgreed in conversation:\n- fennec: Un script ou une page ?\n- human: un script python\n"
        );
        assert_eq!(
            transcript(&json!({"text": "x", "thread": [{"who": "human", "text": "x"}]})),
            "x"
        );
        assert_eq!(human_words(&r), "un test — un script python");
    }

    #[test]
    fn passing_sets_the_work_aside_until_resumed() {
        let tmp = std::env::temp_dir().join(format!("fennec-lead-pass-{}", std::process::id()));
        let store = Store { root: tmp.clone() };
        std::fs::create_dir_all(tmp.join("requests")).unwrap();
        let id = "1".to_string();
        std::fs::write(tmp.join("requests/1.json"), r#"{"status": "running"}"#).unwrap();
        assert!(pass(&store, &id).is_err(), "only a ready request");
        store.update(&id, |r| r["status"] = json!("ready")).unwrap();
        pass(&store, &id).unwrap();
        assert_eq!(store.load(&id).unwrap()["status"], "passed");
        resume(&store, &id).unwrap();
        assert_eq!(store.load(&id).unwrap()["status"], "ready");
        // A correction needs words, and a ready request not yet committed.
        assert!(amend(&store, &id, "  ").is_err());
        store.update(&id, |r| r["commit"] = json!("c1")).unwrap();
        assert!(amend(&store, &id, "plus haut").is_err());
        assert_eq!(store.load(&id).unwrap()["status"], "ready");
        std::fs::remove_dir_all(tmp).unwrap();
    }

    #[test]
    fn what_the_winner_did_is_kept_for_the_human() {
        let mut r = json!({"status": "running"});
        apply_run(
            &mut r,
            &json!({"winner": "w2", "attempts": [{"world": "w1", "summary": "lost"}, {"world": "w2", "summary": "Ajouté bonjour.py",
                "diff": "diff --git a/bonjour.py b/bonjour.py\nnew file\ndiff --git a/tests/t.py b/tests/t.py\n+x\n"}]}),
        );
        assert_eq!(r["summary"], "Ajouté bonjour.py");
        assert_eq!(r["files"], json!(["bonjour.py", "tests/t.py"]));
    }

    #[test]
    fn stats_sum_finished_steps_per_model_and_per_project() {
        let cost = |n: u64| json!({"turns": 1, "sent": n, "cached": n / 2, "received": 10, "thinking": 0, "ms": 1000});
        let steps = [
            json!({"name": "Discussion", "model": "a", "cost": cost(100)}),
            json!({"name": "Travail, essai 1", "model": "b", "cost": cost(200), "passed": false}),
            json!({"name": "Travail, essai 2", "model": "b", "cost": cost(300), "passed": true}),
            json!({"model": "b", "cost": cost(999), "live": true}),
        ];
        let s = totals(&steps);
        assert_eq!(s["sent"], 600);
        assert_eq!(s["cached"], 300);
        assert_eq!(s["failed_attempts"], 1);
        assert_eq!(s["models"]["b"]["sent"], 500);
        assert_eq!(s["stages"]["Travail"]["turns"], 2);
        let requests = [
            (
                "1".to_string(),
                json!({"project": "/p", "status": "landed", "stats": s}),
            ),
            (
                "2".to_string(),
                json!({"project": "/p", "status": "failed"}),
            ),
        ];
        let v = summarize(&requests, &[("/p".to_string(), "apart".to_string())]);
        assert_eq!(v["all"]["requests"], 2);
        let p = &v["projects"][0];
        assert_eq!(
            (
                p["place"].as_str(),
                p["landed"].as_u64(),
                p["failed"].as_u64()
            ),
            (Some("apart"), Some(1), Some(1))
        );
        assert_eq!(p["ms"], 3000);
        assert_eq!(v["models"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn a_step_names_its_roles_with_what_they_told_the_model() {
        let s = json!({"messages": [{"role": "system", "content":
            "Do it.\n\nAs QA on the team:\nTry it.\nTwice.\n\nAs Dev on the team:\nKeep it small.\n\nHistory: x"}]});
        let s = roles(s, &json!(["QA", "Dev", "Gone"]));
        assert_eq!(
            s["roles"][0],
            json!({"name": "QA", "brief": "Try it.\nTwice."})
        );
        assert_eq!(s["roles"][1]["brief"], "Keep it small.");
        assert_eq!(s["roles"][2]["brief"], Value::Null);
        let t = totals(&[s.clone(), s]);
        assert_eq!(t["stages"]["?"]["roles"], json!(["QA", "Dev", "Gone"]));
    }

    #[test]
    fn a_step_costs_the_sum_of_its_turns() {
        let turns = json!([
            {"model_ms": 400, "usage": {"prompt_tokens": 1000, "completion_tokens": 60,
             "completion_tokens_details": {"reasoning_tokens": 20}}},
            {"model_ms": 100, "usage": {"prompt_tokens": 1100, "completion_tokens": 5,
             "prompt_tokens_details": {"cached_tokens": 1000}}},
            {"model_ms": 50, "usage": null}
        ]);
        assert_eq!(
            cost(&turns),
            json!({"turns": 3, "sent": 2100, "cached": 1000, "received": 65, "thinking": 20, "ms": 550})
        );
        assert_eq!(cost(&Value::Null)["turns"], 0);
    }

    #[test]
    fn each_role_that_worked_is_named_once() {
        let mut r = json!({"status": "running", "team": ["Chef de projet"]});
        apply_run(
            &mut r,
            &json!({"winner": "w1", "team": ["Dev senior"], "attempts": []}),
        );
        apply_proof(
            &mut r,
            &json!({"tries": [{"command": "a", "exit": 0}], "team": ["QA", "Dev senior"]}),
        );
        assert_eq!(r["team"], json!(["Chef de projet", "Dev senior", "QA"]));
    }

    #[test]
    fn a_confirmed_rule_is_written_where_other_tools_read_it() {
        let root = std::env::temp_dir().join(format!("fennec-lead-rule-{}", std::process::id()));
        let project = root.join("p");
        std::fs::create_dir_all(root.join("requests")).unwrap();
        std::fs::create_dir_all(&project).unwrap();
        let store = Store { root: root.clone() };
        let id = "ab";
        std::fs::write(
            root.join("requests/ab.json"),
            json!({"project": project, "text": "t", "status": "ready", "offer": {"rule": "use uv", "heard": "non, uv"}}).to_string(),
        )
        .unwrap();
        assert!(rule(&store, id, "maybe").is_err());
        rule(&store, id, "project").unwrap();
        assert_eq!(
            std::fs::read_to_string(project.join("AGENTS.md")).unwrap(),
            "## Rules\n\n- use uv\n"
        );
        assert!(store.load(id).unwrap().get("offer").is_none());
        assert!(rule(&store, id, "no").is_err(), "an offer is answered once");
        std::fs::remove_dir_all(root).unwrap();
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
        let check = |exit: i32, missing: bool| {
            json!({"tries": [{"command": "a", "exit": 0}], "passed": missing,
                   "checks": [{"role": "Sec", "command": "audit", "exit": exit, "missing": missing}]})
        };
        apply_proof(&mut r, &check(1, false));
        assert_eq!(
            r["reason"],
            "le travail est fait, mais Sec bloque : `audit` a échoué"
        );
        apply_proof(&mut r, &check(127, true));
        assert_eq!(r["status"], "ready", "a missing tool is not a veto");
    }
}
