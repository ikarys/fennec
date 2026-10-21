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

/// Agent steps per compile, attempt or chat turn. A scaffold took more than
/// 40 (seen 2026-10-20); loops and idle runs are stopped well before this by
/// the engine's watch, so the cap only bounds a run that keeps progressing.
const MAX_STEPS: usize = 150;

/// `FENNEC_LEAD_ATTEMPTS` (default 1): parallel attempts once validated.
fn attempts() -> usize {
    std::env::var("FENNEC_LEAD_ATTEMPTS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1usize)
        .max(1)
}

/// A request the human asked to stop: the worker sees it at its checkpoints
/// and the request ends failed. Cleared when the worker's thread ends, so a
/// later "Réessayer" of the same id starts clean.
static STOPS: std::sync::Mutex<std::vec::Vec<String>> = std::sync::Mutex::new(Vec::new());

const STOPPED: &str = "stopped: you asked to stop";

struct Stopped(String);

impl Drop for Stopped {
    fn drop(&mut self) {
        if let Ok(mut s) = STOPS.lock() {
            s.retain(|x| x != &self.0);
        }
    }
}

/// The worker's thread lives here while it runs on a request: "Réessayer"
/// refuses to start a second one on the same request.
static WORKING: std::sync::Mutex<std::vec::Vec<String>> = std::sync::Mutex::new(Vec::new());

struct Working(String);

impl Working {
    fn new(id: String) -> Self {
        if let Ok(mut w) = WORKING.lock() {
            w.push(id.clone());
        }
        Self(id)
    }
}

impl Drop for Working {
    fn drop(&mut self) {
        if let Ok(mut w) = WORKING.lock() {
            w.retain(|x| x != &self.0);
        }
    }
}

pub fn working(id: &str) -> bool {
    WORKING.lock().is_ok_and(|w| w.iter().any(|x| x == id))
}

/// Whether any worker runs, so the model server is in use.
/// ponytail: a discussion's turn is not counted; it is short.
pub fn busy() -> bool {
    WORKING.lock().is_ok_and(|w| !w.is_empty())
}

/// A discussion's turn lives here while it runs: a second turn on the same
/// chat is refused, so two threads never write the same chat file.
static CHATS: std::sync::Mutex<std::vec::Vec<String>> = std::sync::Mutex::new(Vec::new());

struct ChatTurn(String);

impl ChatTurn {
    fn new(id: String) -> Option<Self> {
        let mut c = CHATS.lock().ok()?;
        if c.iter().any(|x| x == &id) {
            return None;
        }
        c.push(id.clone());
        Some(Self(id))
    }
}

impl Drop for ChatTurn {
    fn drop(&mut self) {
        if let Ok(mut c) = CHATS.lock() {
            c.retain(|x| x != &self.0);
        }
    }
}

/// Runs `step` in the background; an error fails the request with its reason.
fn spawn(id: String, step: fn(&Store, &str) -> io::Result<()>) {
    std::thread::spawn(move || {
        let _stopped = Stopped(id.clone());
        let _working = Working::new(id.clone());
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
        // A stopped request ends failed, whatever the step wrote last.
        if stopped(&id) {
            let _ = store.update(&id, |r| {
                r["status"] = json!("failed");
                r["reason"] = json!(STOPPED);
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

/// A discussion's turn: the settings' model, no team; the chat file keeps the
/// exchange and each call's cost, the page follows it.
pub fn chat(id: String) {
    let Some(turn) = ChatTurn::new(id.clone()) else {
        // The previous turn has not released the guard: its message was
        // already committed as thinking, so settle it here with a reason —
        // otherwise the page stays on "répond…" with no way out.
        if let Ok(worlds) = Worlds::from_env() {
            let _ = fennec_os::chat::fail(&worlds, &id, "the previous turn is still running");
        }
        eprintln!("fennec: discussion {id} refused, a turn is still running");
        return;
    };
    std::thread::spawn(move || {
        let _turn = turn;
        let Ok(worlds) = Worlds::from_env() else {
            return;
        };
        match OpenAi::variants_from_env(1) {
            // No model to answer with: the turn never started, so say so and
            // let the page move on, as a failed turn would.
            Err(e) => {
                let _ = fennec_os::chat::fail(&worlds, &id, &e.to_string());
                eprintln!("fennec: discussion {id} cannot start: {e}");
            }
            Ok(model) => {
                if let Err(e) = fennec_os::chat::answer(&worlds, &id, &model[0], MAX_STEPS) {
                    eprintln!("fennec: discussion {id} stops: {e}");
                }
            }
        }
    });
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
    // Framed while it waited: the conversation already took place.
    if let Some(brief) = request["ahead"].as_str() {
        let brief = brief.to_string();
        store.update(id, |r| {
            r["brief"] = json!(brief);
            r["status"] = json!("framing");
            if let Some(m) = r.as_object_mut() {
                m.remove("ahead");
            }
        })?;
        return frame_now(store, id);
    }
    // Asked while it waited: the human's answer comes first.
    if request["asked"] == true {
        return store.update(id, |r| {
            r["status"] = json!("talk");
            if let Some(m) = r.as_object_mut() {
                m.remove("asked");
            }
        });
    }
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
        spec::Discussed::Brief(brief, skills, roles) => {
            let brief = with_images(&brief, &thread);
            store.update(id, |r| {
                note(r, step);
                r["brief"] = json!(brief);
                r["picked"] = json!(skills);
                if let Some(roles) = roles {
                    r["roles"] = json!(roles);
                }
                r["status"] = json!("framing");
            })?;
            frame_now(store, id)
        }
    }
}

/// The model restates the request in words: the images the human attached go
/// with them, for the work to see.
fn with_images(brief: &str, thread: &[Value]) -> String {
    let images = thread
        .iter()
        .filter(|t| t["who"] == "human")
        .flat_map(|t| crate::state::image_lines(t["text"].as_str().unwrap_or("")));
    std::iter::once(brief)
        .chain(images)
        .collect::<Vec<_>>()
        .join("\n")
}

/// A Fleet task framed while it waits: the conversation's turn runs now, so
/// a question reaches the human before the Fleet gets to the task, and the
/// task starts on its brief. It stays for later either way.
pub fn prepare(id: String) {
    std::thread::spawn(move || {
        let _working = Working::new(id.clone());
        let Ok(store) = Store::from_env() else { return };
        if let Err(e) = prepare_now(&store, &id) {
            // Not tried again: the task is framed as usual once launched.
            let _ = store.update(&id, |r| {
                r["ahead_failed"] = json!(true);
                note(r, json!({"stage": "talk", "error": e.to_string()}));
            });
        }
    });
}

fn prepare_now(store: &Store, id: &str) -> io::Result<()> {
    let request = store.load(id)?;
    if request["status"] != "later" {
        return Ok(());
    }
    let project = Path::new(request["project"].as_str().unwrap_or(""));
    let model = staffed(OpenAi::variants_from_env(1)?.remove(0), project, "frame")?;
    let thread = request["thread"].as_array().cloned().unwrap_or_default();
    let (said, exchange) = spec::discuss(project, &thread, &model)?;
    store.update(id, |r| {
        // Reworded, launched or answered meanwhile: this turn is out of date.
        if r["status"] != "later" || r["thread"] != request["thread"] {
            return;
        }
        note(r, json!({"stage": "talk", "exchange": exchange}));
        match &said {
            spec::Discussed::Reply(text) => {
                say_in(r, "fennec", text);
                r["asked"] = json!(true);
            }
            spec::Discussed::Brief(brief, skills, roles) => {
                r["ahead"] = json!(with_images(brief, &thread));
                r["picked"] = json!(skills);
                if let Some(roles) = roles {
                    r["roles"] = json!(roles);
                }
            }
        }
    })
}

/// A Backlog task too big for one go: the model cuts it into smaller ones
/// that take its place in the Backlog, each after the ones it builds on, so
/// the Fleet runs them in order. Each is framed and validated as usual.
pub fn split(store: &Store, id: &str) -> io::Result<()> {
    let request = store.load(id)?;
    if request["status"] != "later" || request["splitting"] == true {
        return Err(io::Error::other("only a task in the Backlog can be split"));
    }
    store.update(id, |r| {
        r["splitting"] = json!(true);
        if let Some(m) = r.as_object_mut() {
            m.remove("split_error");
        }
    })?;
    let id = id.to_string();
    let root = store.root.clone();
    std::thread::spawn(move || {
        let _working = Working::new(id.clone());
        let Ok(store) = Store::from_env() else {
            // The turn never started: the task is splittable again.
            let _ = Store { root }.update(&id, |r| r["splitting"] = json!(false));
            return;
        };
        if let Err(e) = split_now(&store, &id) {
            let _ = store.update(&id, |r| {
                r["split_error"] = json!(e.to_string());
                if let Some(m) = r.as_object_mut() {
                    m.remove("splitting");
                }
            });
        }
    });
    Ok(())
}

fn split_now(store: &Store, id: &str) -> io::Result<()> {
    let request = store.load(id)?;
    let project = request["project"].as_str().unwrap_or("");
    let model = staffed(
        OpenAi::variants_from_env(1)?.remove(0),
        Path::new(project),
        "frame",
    )?;
    // What fennec understood, when the human read it; else their words.
    let task = match request["ahead"].as_str() {
        Some(brief) => brief.to_string(),
        None => transcript(&request),
    };
    let parts = spec::split(Path::new(project), &task, &model)?;
    replace_by(store, id, &parts)
}

/// `id` gives way to `parts`: each inherits its `after` and its roles left
/// out, and a task that came after `id` comes after all of them.
fn replace_by(store: &Store, id: &str, parts: &[(String, Vec<usize>)]) -> io::Result<()> {
    let request = store.load(id)?;
    if request["status"] != "later" {
        return Err(io::Error::other("the task left the Backlog meanwhile"));
    }
    let project = request["project"].as_str().unwrap_or("");
    let off: Vec<String> = request["off"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|k| k.as_str().map(str::to_string))
        .collect();
    let inherited = request["after"].as_array().cloned().unwrap_or_default();
    let mut ids: Vec<String> = Vec::new();
    for (text, after) in parts {
        let new = store.add_request(project, text, &off, true)?;
        let mut deps = inherited.clone();
        deps.extend(after.iter().map(|a| json!(ids[*a])));
        store.update(&new, |r| r["after"] = json!(deps))?;
        ids.push(new);
    }
    for (other, r) in store.requests()? {
        if r["after"]
            .as_array()
            .is_some_and(|a| a.contains(&json!(id)))
        {
            store.update(&other, |r| {
                if let Some(a) = r["after"].as_array_mut() {
                    a.retain(|x| x != id);
                    a.extend(ids.iter().map(|i| json!(i)));
                }
            })?;
        }
    }
    store.forget(id)
}

/// The human speaks in the conversation: an answer, a precision, or a
/// correction of the proposed "done when", which is then dropped.
pub fn say(store: &Store, id: &str, text: &str) -> io::Result<()> {
    let request = store.load(id)?;
    let open = match request["status"].as_str() {
        Some("talk" | "question") => true,
        // In the Backlog: an answer, or a correction of what fennec understood.
        Some("later") => request["asked"] == true || request["ahead"].is_string(),
        Some("blocked") => request["stage"] == "framing",
        _ => false,
    };
    if !open {
        return Err(io::Error::other("this task does not wait for an answer"));
    }
    let text = text.trim();
    if text.is_empty() {
        return Err(io::Error::other("empty message"));
    }
    // A task framed ahead stays for later: the next turn runs when the
    // model is free again. Corrected, it leaves Ready until validated anew.
    if request["status"] == "later" {
        return store.update(id, |r| {
            say_in(r, "human", text);
            note(r, json!({"stage": "said", "text": text}));
            if let Some(m) = r.as_object_mut() {
                for k in ["asked", "ahead", "done_when", "picked", "roles"] {
                    m.remove(k);
                }
            }
        });
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
        // The journal reads as a story: what the human said marks where the
        // work turned.
        note(r, json!({"stage": "said", "text": text}));
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
    let brief = transcript(&request);
    if request["status"] == "later" && request["asked"] == true {
        return store.update(id, |r| {
            r["ahead"] = json!(brief);
            r["done_when"] = json!(human_words(&request));
            if let Some(m) = r.as_object_mut() {
                m.remove("asked");
            }
        });
    }
    if request["status"] != "talk" {
        return Err(io::Error::other("this task is not in discussion"));
    }
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

/// Once the request is clear, the framer writes its tests: a valid spec is
/// proposed to the human, who validates once; a question goes to the human,
/// a reason fails the request.
fn frame_now(store: &Store, id: &str) -> io::Result<()> {
    let request = store.load(id)?;
    let project = Path::new(request["project"].as_str().unwrap_or(""));
    crate::state::start(project)?;
    let brief = request["brief"]
        .as_str()
        .or(request["text"].as_str())
        .unwrap_or("")
        .to_string();
    // The skills framing picked; none recorded, all the human keeps.
    let skills: Option<Vec<String>> = request["picked"].as_array().map(|a| {
        a.iter()
            .filter_map(|s| s.as_str().map(str::to_string))
            .collect()
    });
    let roles: Option<Vec<String>> = request["roles"].as_array().map(|a| {
        a.iter()
            .filter_map(|s| s.as_str().map(str::to_string))
            .collect()
    });
    let worlds = Worlds::from_env()?;
    let model = staffed(OpenAi::variants_from_env(1)?.remove(0), project, "frame")?;
    let mut s = spec::compile(&worlds, project, &brief, &model, MAX_STEPS)?;
    let spec_id = s["world"].as_str().unwrap_or("").to_string();
    match s["status"].as_str() {
        Some("proposed") => {
            // The framing's picks ride on the spec the human validates.
            if let Some(skills) = &skills {
                s["skills"] = json!(skills);
            }
            if let Some(roles) = &roles {
                s["roles"] = json!(roles);
            }
            spec::save(&worlds, &spec_id, &s)?;
            store.update(id, |r| {
                r["spec"] = json!(spec_id);
                r["done_when"] = s["done_when"].clone();
                r["status"] = json!("question");
                note(r, json!({"stage": "frame", "spec": spec_id}));
            })
        }
        _ if s["question"].is_string() => {
            let q = s["question"].as_str().unwrap_or("").to_string();
            let why = s["reason"]
                .as_str()
                .unwrap_or("it asks before it can write the tests");
            store.update(id, |r| {
                r["spec"] = json!(spec_id);
                note(r, json!({"stage": "frame", "spec": spec_id}));
                block(r, "framing", &q, why);
            })
        }
        _ => {
            let why = s["reason"]
                .as_str()
                .unwrap_or("the framer wrote no tests")
                .to_string();
            store.update(id, |r| {
                r["spec"] = json!(spec_id);
                r["status"] = json!("failed");
                r["reason"] = json!(why);
                note(r, json!({"stage": "frame", "spec": spec_id}));
            })
        }
    }
}

/// The human's one answer: validate starts the work, a correction re-frames.
pub fn answer(store: &Store, id: &str, accept: bool, correction: &str) -> io::Result<()> {
    let request = store.load(id)?;
    if request["status"] != "question" {
        return Err(io::Error::other("this task does not wait for an answer"));
    }
    if accept {
        store.update(id, |r| r["status"] = json!("running"))?;
        spawn(id.to_string(), solve_now);
    } else {
        let correction = correction.trim();
        if correction.is_empty() {
            return Err(io::Error::other("say what is wrong"));
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
        return Err(io::Error::other("no rule offered"));
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
        return Err(io::Error::other("this task does not wait for an answer"));
    }
    let text = text.trim();
    if text.is_empty() {
        return Err(io::Error::other("empty answer"));
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
        // A question mid-solve may come after the spec was accepted (a role
        // asked while its chain already had a winner): solve_now then goes
        // through spec::retry, which handles an accepted spec, not
        // spec::accept, which would fail on it.
        store.update(id, |r| {
            r["status"] = json!("running");
            r["heard"] = json!(text);
            r["retry"] = json!(text);
        })?;
        spawn(id.to_string(), solve_now);
    }
    Ok(())
}

/// Projects whose folder a request just took: two waiting requests must not
/// both see it free. Held while the work runs; by then the folder says it.
static TAKEN: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

#[derive(Debug)]
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
        if stopped(id) {
            return Err(io::Error::other(STOPPED));
        }
        let held = match world::held(Path::new(project))? {
            // Its own last try still holds the folder: retrying drops it.
            Some(world::Held::World(w)) if Some(&w) == mine.as_ref() => None,
            Some(world::Held::World(w)) => {
                let owner = store
                    .requests()?
                    .into_iter()
                    .find(|(_, r)| r["run"] == w.as_str());
                match owner {
                    Some((_, r)) => Some(format!("\"{}\"", crate::fleet::short(&r))),
                    // No request owns it and no worker here holds the folder:
                    // a start cut short (fennec restarted before the run was
                    // recorded) left it. Dropping it stashes what it held.
                    // ponytail: assumes one fennec per project folder; a second
                    // instance mid-start would lose its world.
                    None if !TAKEN.lock().is_ok_and(|t| t.iter().any(|p| p == project)) => {
                        let worlds = Worlds::from_env()?;
                        match worlds.open(&w).and_then(|o| worlds.drop(o)) {
                            Ok(()) => continue,
                            Err(e) => Some(format!("an old interrupted task ({e})")),
                        }
                    }
                    None => Some("another task".to_string()),
                }
            }
            Some(world::Held::Changes) => Some(String::new()),
            None => None,
        };
        let mut taken = TAKEN.lock().map_err(|_| io::Error::other("lock"))?;
        let behind = match held {
            Some(by) => Some(by),
            None if taken.iter().any(|p| p == project) => Some("another task".to_string()),
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

/// The branch the last of `request`'s `after` tasks was committed on.
/// ponytail: the last one only; tasks after work on two branches need a merge.
fn after_branch(store: &Store, request: &Value) -> Option<String> {
    request["after"]
        .as_array()?
        .iter()
        .filter_map(|a| store.load(a.as_str()?).ok())
        .filter(|d| d["status"] == "landed")
        .filter_map(|d| d["target"]["branch"].as_str().map(str::to_string))
        .next_back()
}

fn solve_now(store: &Store, id: &str) -> io::Result<()> {
    let request = store.load(id)?;
    let mut worlds = Worlds::from_env()?;
    // The Fleet's work goes apart: ready and waiting for the human, it must
    // not hold the folder the next task of the queue needs.
    let fleet = request["fleet"] == true;
    worlds.apart = fleet;
    // Committed on the Fleet's branch, it builds on what landed there; else
    // on the branch the tasks it comes after were committed on, or it would
    // redo or undo their work.
    worlds.base = request["fleet_branch"]
        .as_str()
        .map(str::to_string)
        .or(after_branch(store, &request));
    let project = Path::new(request["project"].as_str().unwrap_or(""));
    // A correction works on the ready work, which already holds the folder.
    let _turn = match request["amend"].is_string() || fleet {
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
        (Some(change), _) => spec::amend(&worlds, spec_id, change, &models, MAX_STEPS, None)?,
        (None, Some(why)) => spec::retry(&worlds, spec_id, why, &models, MAX_STEPS, None)?,
        (None, None) => spec::accept(&worlds, spec_id, &models, MAX_STEPS, None)?,
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
            r["files"] = merged(&before, &r["files"]);
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
    let Some(mut run) = record["winner"].as_str().map(str::to_string) else {
        return Ok(());
    };
    let r = store.load(id)?;
    let done_when = [&r["done_when"], &r["brief"], &r["text"]]
        .into_iter()
        .find_map(Value::as_str)
        .map(str::to_string);
    let prover = staffed(OpenAi::variants_from_env(1)?.remove(0), project, "prove")?;
    let watched = Live::new(&prover, store.live_path(id)?);
    let mut fixes = 0;
    loop {
        // Tests passing is not "ready": the product is tried first.
        store.update(id, |r| r["status"] = json!("proving"))?;
        let proof = proof::gather(&worlds, &run, done_when.as_deref(), &watched)?;
        let mut probe = store.load(id)?;
        apply_proof(&mut probe, &proof);
        // What the proof found goes back to the builder, which starts from
        // its own work: a missing script or a crash is fixed, not failed.
        if probe["status"] == "failed" && fixes < MAX_FIXES && !stopped(id) {
            fixes += 1;
            let found = findings(&proof);
            store.update(id, |r| {
                note(r, json!({"stage": "qa", "findings": found}));
                r["status"] = json!("running");
            })?;
            let fixed = spec::fix(&worlds, spec_id, &found, &models, MAX_STEPS, None)?;
            if let Some(w) = fixed["winner"].as_str() {
                run = w.to_string();
                // The proof asked for a change: a fix that changed nothing
                // holds no work to prove, it fails where it stands.
                if nothing_changed(&probe["files"], &fixed) {
                    return store.update(id, |r| {
                        note(r, json!({"stage": "solve", "run": fixed["id"]}));
                        r["status"] = json!("failed");
                        r["reason"] =
                            json!("fennec changed nothing, and the proof asked for the change");
                    });
                }
                store.update(id, |r| {
                    let before = r["files"].clone();
                    note(r, json!({"stage": "solve", "run": fixed["id"]}));
                    apply_run(r, &fixed);
                    r["files"] = merged(&before, &r["files"]);
                })?;
                continue;
            }
            // No fix passed: a question goes to the human, otherwise the
            // proof's own reason stands.
            let asked = fixed["attempts"]
                .as_array()
                .is_some_and(|a| a.iter().any(|t| t["question"].is_string()));
            return store.update(id, |r| {
                note(r, json!({"stage": "solve", "run": fixed["id"]}));
                if asked {
                    apply_run(r, &fixed);
                } else {
                    apply_proof(r, &proof);
                }
            });
        }
        // Where it will land is decided before "ready", so the card never shows
        // a branch the click would not use.
        let target = (probe["status"] == "ready")
            .then(|| solve::plan(&worlds, &run, &variants[0]).ok())
            .flatten();
        return store.update(id, |r| {
            apply_proof(r, &proof);
            if let Some(t) = &target {
                r["target"] = target_json(t);
            }
        });
    }
}

/// How many times the proof sends a ready run back to its builder.
const MAX_FIXES: usize = 3;

fn stopped(id: &str) -> bool {
    STOPS.lock().is_ok_and(|s| s.iter().any(|x| x == id))
}

/// Files of the work so far, then the new ones a later run touched.
fn merged(before: &Value, after: &Value) -> Value {
    let mut files = before.as_array().cloned().unwrap_or_default();
    for f in after.as_array().into_iter().flatten() {
        if !files.contains(f) {
            files.push(f.clone());
        }
    }
    json!(files)
}

/// The proof asked for a change, so a fix that changed nothing — on top of
/// work that changed nothing — holds no work to prove.
fn nothing_changed(held: &Value, fixed: &Value) -> bool {
    let held_empty = held.as_array().is_some_and(|f| f.is_empty());
    let diff = fixed["attempts"]
        .as_array()
        .and_then(|a| {
            a.iter()
                .find(|t| t["world"] == fixed["winner"].as_str().unwrap_or(""))
        })
        .map(|t| t["diff"].as_str().unwrap_or(""))
        .unwrap_or("");
    held_empty && changed_files(diff).is_empty()
}

/// What a failed proof tells the builder: each failing try with the end of
/// its output, a page that did not show, a role's check, or no proof at all.
fn findings(proof: &Value) -> String {
    let tail = |o: &Value| {
        let o = o.as_str().unwrap_or("").trim();
        o[o.floor_char_boundary(o.len().saturating_sub(1500))..].to_string()
    };
    let mut out = Vec::new();
    for t in proof["tries"].as_array().into_iter().flatten() {
        if t["exit"] != 0 {
            out.push(format!(
                "`{}` exited {}:\n{}",
                t["command"].as_str().unwrap_or(""),
                t["exit"],
                tail(&t["output"])
            ));
        }
    }
    for c in proof["checks"].as_array().into_iter().flatten() {
        if c["exit"] != 0 && c["missing"] != true {
            out.push(format!(
                "the {} check `{}` exited {}:\n{}",
                c["role"].as_str().unwrap_or(""),
                c["command"].as_str().unwrap_or(""),
                c["exit"],
                tail(&c["output"])
            ));
        }
    }
    for s in proof["shots"].as_array().into_iter().flatten() {
        if s["ok"] != true && s["missing"] != true {
            out.push(format!(
                "the page {} did not show: {}",
                s["url"].as_str().unwrap_or(""),
                s["reason"].as_str().unwrap_or("")
            ));
        }
    }
    if out.is_empty() {
        let why: Vec<&str> = proof["unverified"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        out.push(format!(
            "nothing could be tried: no command or page proved it works ({}). \
             Make it runnable here and say how in the README.",
            why.join("; ")
        ));
    }
    out.join("\n\n")
}

fn target_json(t: &solve::Landing) -> Value {
    json!({"branch": t.branch, "message": t.message, "remote": t.remote})
}

/// "Réessayer" on a failed request: solved again from its validated tests
/// when there are some, told why the last try failed; framed again when it
/// failed before "done when" was validated.
pub fn retry(store: &Store, id: &str) -> io::Result<()> {
    let request = store.load(id)?;
    // A worker that is still alive owns the request; a dead one left it in
    // place: the step starts over and sets the status again as it goes.
    if matches!(request["status"].as_str(), Some("waiting" | "landing")) {
        if working(id) {
            return Err(io::Error::other("this task is already running"));
        }
        spawn(
            id.to_string(),
            match request["status"].as_str() {
                Some("landing") => land_now,
                _ => solve_now,
            },
        );
        return Ok(());
    }
    // A ready request whose commit did not push landed nothing: the work is
    // redone.
    let recommit = request["status"] == "ready" && request["land_error"].is_string();
    if request["status"] != "failed" && !recommit {
        return Err(io::Error::other("this task did not fail"));
    }
    let validated = match request["spec"].as_str() {
        Some(s) => matches!(
            spec::load(&Worlds::from_env()?, s)?["status"].as_str(),
            Some("proposed" | "accepted")
        ),
        None => false,
    };
    let why = if recommit {
        request["land_error"].as_str().unwrap_or("").to_string()
    } else {
        request["reason"].as_str().unwrap_or("").to_string()
    };
    store.update(id, |r| {
        if let Some(m) = r.as_object_mut() {
            m.remove("proof");
            m.remove("reason");
            if recommit {
                // Nothing was pushed: the commit is redone with the new work.
                m.remove("commit");
                m.remove("land_error");
            }
            // Its next failure stops the Fleet as the first one did.
            m.remove("fleet_seen");
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
            "the work is done, but the try failed: {}",
            t["command"].as_str().unwrap_or("")
        ));
    } else if let Some(c) = vetoed {
        request["status"] = json!("failed");
        request["reason"] = json!(format!(
            "the work is done, but {} blocks: `{}` failed",
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
            "the work is done, but the page {} does not show: {}",
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
            "the work is done, but fennec could not look at the page: no browser is installed (chromium)"
        } else {
            "the work is done, but fennec could not try the product: no proof"
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
            request["skills"] = t["skills"].clone();
            // The reviewer's points the dev rejected, for the human's call:
            // fresh with each run, the stale ones die with it.
            if let Some(d) = t["disagreements"].as_array() {
                request["disagreements"] = json!(d);
            } else {
                request.as_object_mut().map(|m| m.remove("disagreements"));
            }
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
        return Err(io::Error::other(
            "this task does not wait for your decision",
        ));
    }
    let text = text.trim();
    if text.is_empty() {
        return Err(io::Error::other("say what is wrong"));
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

/// "Forcer le point" / "Garder la position du dev" on a ready card: the
/// human's call on each of the reviewer's points the dev rejected.
pub fn disagree(store: &Store, id: &str, point: &str, action: &str) -> io::Result<()> {
    let request = store.load(id)?;
    if request["status"] != "ready" || request["commit"].is_string() {
        return Err(io::Error::other(
            "this task does not wait for your decision",
        ));
    }
    let point = point.trim();
    if point.is_empty() {
        return Err(io::Error::other("say which point"));
    }
    if action == "keep" {
        // The dev's position stands; the point leaves the card.
        store.update(id, |r| {
            if let Some(d) = r["disagreements"].as_array()
                && d.iter().any(|d| d["point"].as_str() == Some(point))
            {
                r["disagreements"] = json!(
                    d.iter()
                        .filter(|d| d["point"].as_str() != Some(point))
                        .cloned()
                        .collect::<Vec<_>>()
                );
            }
        })
    } else {
        // The reviewer's point is forced: the work is redone on it.
        let text = format!("force the reviewer's point: {point}");
        store.update(id, |r| {
            say_in(r, "human", &text);
            r["status"] = json!("running");
            r["amend"] = json!(format!("the reviewer's point is forced, apply it: {point}"));
            r["heard"] = json!(text);
            r.as_object_mut().map(|m| m.remove("disagreements"));
            if let Some(m) = r.as_object_mut() {
                m.remove("amend_error");
                m.remove("land_error");
            }
        })?;
        spawn(id.to_string(), solve_now);
        Ok(())
    }
}

/// "Intégrer" on a ready request: its work becomes one commit on the branch
/// shown on the card, with the message shown there, then that branch is
/// pushed. Without a branch on the card yet, the click only decides it. A landing that fails leaves the request ready, with the reason;
/// once committed, a new click only retries the push.
/// "Arrêter" on a request under way: it fails with the reason, the worker
/// ends at its next checkpoint, and the folder it held comes back. A
/// stopped request can be retried, unlike a dropped one.
pub fn stop(store: &Store, id: &str) -> io::Result<()> {
    let request = store.load(id)?;
    if !matches!(
        request["status"].as_str(),
        Some("thinking" | "framing" | "waiting" | "running" | "proving" | "landing")
    ) {
        return Err(io::Error::other("fennec no longer works on this task"));
    }
    if let Ok(mut s) = STOPS.lock() {
        s.push(id.to_string());
    }
    // Its work goes too: in the project folder, the folder comes back.
    let worlds = Worlds::from_env()?;
    if let Some(run) = request["run"].as_str()
        && worlds.open(run).is_ok()
    {
        review::reject(&worlds, run, None)?;
    }
    store.update(id, |r| {
        r["status"] = json!("failed");
        r["reason"] = json!(STOPPED);
        if let Some(m) = r.as_object_mut() {
            m.remove("behind");
            m.remove("land_error");
        }
    })
}

/// Work left behind that no request needs any more: a start or a drop cut
/// short (fennec restarted, a step failed half way). A request that is not
/// dropped or landed keeps what it names; a project with work in flight is
/// left out, since its newest world may not be on its request yet.
pub fn leftovers(store: &Store) -> io::Result<Value> {
    let worlds = Worlds::from_env()?;
    let requests = store.requests()?;
    let kept: Vec<String> = requests
        .iter()
        .filter(|(_, r)| !matches!(r["status"].as_str(), Some("dropped" | "landed")))
        .map(|(_, r)| r.to_string())
        .collect();
    let mut busy: Vec<String> = requests
        .iter()
        .filter(|(_, r)| {
            matches!(
                r["status"].as_str(),
                Some("thinking" | "framing" | "waiting" | "running" | "proving" | "landing")
            )
        })
        .filter_map(|(_, r)| r["project"].as_str().map(str::to_string))
        .collect();
    busy.extend(TAKEN.lock().map(|t| t.clone()).unwrap_or_default());
    let mut left = Vec::new();
    for id in worlds.list()? {
        if kept.iter().any(|r| r.contains(&id)) {
            continue;
        }
        let Ok(facts) = worlds.facts(&id) else {
            continue;
        };
        let project = Path::new(facts["project"].as_str().unwrap_or(""));
        if !busy.iter().any(|p| Path::new(p) == project) {
            left.push(facts);
        }
    }
    Ok(json!(left))
}

/// Throws away the leftovers the human picked, checked again here: what
/// became used since the page was drawn stays.
pub fn clear(store: &Store, ids: &[String]) -> io::Result<Value> {
    let left = leftovers(store)?;
    let worlds = Worlds::from_env()?;
    let mut errors = Vec::new();
    for id in ids {
        if !left
            .as_array()
            .into_iter()
            .flatten()
            .any(|f| f["id"] == id.as_str())
        {
            errors.push(format!("{id}: a task uses it again"));
        } else if let Err(e) = worlds.forget(id) {
            errors.push(format!("{id} : {e}"));
        }
    }
    Ok(json!({"errors": errors}))
}

/// The human gives up on a request. Only one waiting for them: work in
/// progress would write its result over the decision.
pub fn drop(store: &Store, id: &str) -> io::Result<()> {
    let status = store.load(id)?["status"].as_str().unwrap_or("").to_string();
    if !matches!(
        status.as_str(),
        "talk" | "question" | "blocked" | "ready" | "failed" | "later"
    ) {
        return Err(io::Error::other("fennec still works on this task"));
    }
    undo(&store.load(id)?)?;
    store.update(id, |r| {
        r["status"] = json!("dropped");
        if let Some(m) = r.as_object_mut() {
            m.remove("offer");
        }
    })
}

/// A request's work goes: in the project folder, the folder comes back.
fn undo(request: &Value) -> io::Result<()> {
    if !request["run"].is_string() && !request["spec"].is_string() {
        return Ok(());
    }
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
    Ok(())
}

/// A failed request back to "Plus tard", as first written: its work goes,
/// the human can reword it and launch it again, or give it to the Fleet.
/// The Journal stays, so what the failed try cost is still counted.
pub fn later(store: &Store, id: &str) -> io::Result<()> {
    let request = store.load(id)?;
    if request["status"] != "failed" {
        return Err(io::Error::other("this task did not fail"));
    }
    undo(&request)?;
    store.update(id, |r| {
        let text = r["text"].clone();
        *r = json!({"project": r["project"], "text": text, "off": r["off"], "after": r["after"],
                    "journal": r["journal"], "status": "later",
                    "thread": [{"who": "human", "text": text}]});
    })
}

/// "Passer": the work is set aside, the project untouched. The world stays,
/// so the human can come back to it and commit later.
pub fn pass(store: &Store, id: &str) -> io::Result<()> {
    let request = store.load(id)?;
    if request["status"] != "ready" {
        return Err(io::Error::other(
            "this task does not wait for your decision",
        ));
    }
    park(&request, true)?;
    store.update(id, |r| r["status"] = json!("passed"))
}

/// Back from the history to the human's decisions: its work back in the
/// folder, if the folder is free.
pub fn resume(store: &Store, id: &str) -> io::Result<()> {
    let request = store.load(id)?;
    if request["status"] != "passed" {
        return Err(io::Error::other("this task was not set aside"));
    }
    park(&request, false)?;
    store.update(id, |r| r["status"] = json!("ready"))
}

/// Work set aside does not hold the project folder: it waits on its branch,
/// the folder goes back to the human (fennec-os `park`), and comes back to
/// it when resumed.
pub fn park(request: &Value, on: bool) -> io::Result<()> {
    let Some(run) = request["run"].as_str() else {
        return Ok(());
    };
    let worlds = Worlds::from_env()?;
    let Ok(world) = worlds.open(run) else {
        return Ok(());
    };
    if on {
        worlds.park(&world)
    } else {
        worlds.unpark(&world)
    }
}

/// Opens `folder` in the human's editor; false when there is none.
pub fn editor(folder: &Path) -> bool {
    // ponytail: VS Code only (`code`, which WSL hands to Windows); another
    // editor would be a setting.
    match std::process::Command::new("code")
        .arg(folder)
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
        .ok_or_else(|| io::Error::other("this task has no work ready"))?;
    let folder = Worlds::from_env()?.open(run)?.path;
    let opened = editor(&folder);
    Ok(json!({"folder": folder, "opened": opened}))
}

/// "Ouvrir la page": the ready work's product, started the way its proof
/// started it, for the human's own browser.
pub fn page(store: &Store, id: &str) -> io::Result<Value> {
    let request = store.load(id)?;
    let run = request["run"]
        .as_str()
        .filter(|_| request["status"] == "ready")
        .ok_or_else(|| io::Error::other("this task has no work ready"))?;
    Ok(json!({"url": proof::page(&Worlds::from_env()?, run)?}))
}

/// What fennec changed for a request, read from its run: asked for when the
/// human opens it, since a diff is too large for every refresh of the page.
pub fn changes(store: &Store, id: &str) -> io::Result<Value> {
    let request = store.load(id)?;
    let run = request["run"]
        .as_str()
        .ok_or_else(|| io::Error::other("fennec changed nothing for this task"))?;
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
                        step("Framing: done when and tests", &s),
                        &s["team"],
                    ));
                }
            }
            Some("solve") => {
                let Ok(r) = review::load(&worlds, e["run"].as_str().unwrap_or("")) else {
                    continue;
                };
                for (n, a) in r["attempts"].as_array().into_iter().flatten().enumerate() {
                    // The team's attempt: one step per role, its own context.
                    match a["roles"].as_array().filter(|rs| {
                        !rs.is_empty() && rs[0]["name"].is_string()
                    }) {
                        Some(rs) => {
                            for role in rs {
                                let name = role["name"].as_str().unwrap_or("agent");
                                let mut s = roles(
                                    step(&format!("Work, try {} · {name}", n + 1), role),
                                    &json!([name]),
                                );
                                s["tools"] = role["tools"].clone();
                                s["passed"] = a["passed"].clone();
                                steps.push(s);
                            }
                        }
                        None => {
                            let mut s =
                                roles(step(&format!("Work, try {}", n + 1), a), &r["team"]);
                            s["passed"] = a["passed"].clone();
                            steps.push(s);
                        }
                    }
                }
                if !r["proof"]["exchange"].is_null() {
                    let mut s = roles(step("Proof", &r["proof"]["exchange"]), &r["proof"]["team"]);
                    // A prove role also acts on its own: its commands, run by fennec.
                    s["checks"] = r["proof"]["checks"].clone();
                    steps.push(s);
                }
            }
            Some("said") => steps.push(json!({"name": "Your answer", "said": e["text"]})),
            Some("qa") => steps.push(json!({"name": "Proof failed: sent back to work", "said": e["findings"]})),
            Some("outside") => steps.push(json!({
                "name": format!("Via {}", e["via"].as_str().unwrap_or("")),
                "said": format!("{} by {}", e["did"].as_str().unwrap_or(""), e["from"].as_str().unwrap_or(""))})),
            Some("plugin") => steps.push(json!({
                "name": format!("Plugin {}", e["name"].as_str().unwrap_or("")),
                "said": format!("could not notify: {}", e["error"].as_str().unwrap_or(""))})),
            _ => {}
        }
    }
    // The step under way, as far as it has gone.
    let under_way = match request["status"].as_str() {
        Some("framing") => Some("Starting (running)"),
        Some("running") => Some("Work (running)"),
        Some("proving") => Some("Proof (running)"),
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

/// Tokens and model time summed over a list of model calls.
pub fn cost(turns: &Value) -> Value {
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

const STATS_VERSION: u64 = 4;
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
        // "Work, try 2" counts as "Work".
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
    // Out of the lists, out of the figures: an archived request is not shown.
    requests.retain(|(_, r)| r["archived"] != true);
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
    let mut v = summarize(&requests, &places);
    // Disk the worlds take apart from the folders, all and per project; a
    // world in the folder itself takes none.
    // ponytail: walks every world at each look; cache it if worlds grow big.
    let worlds = Worlds::from_env()?;
    let mut disk = 0;
    for id in worlds.list()? {
        let Ok(f) = worlds.facts(&id) else { continue };
        let bytes = f["bytes"].as_u64().unwrap_or(0);
        disk += bytes;
        if let Some(p) = v["projects"]
            .as_array_mut()
            .and_then(|l| l.iter_mut().find(|p| p["path"] == f["project"]))
        {
            p["disk"] = json!(p["disk"].as_u64().unwrap_or(0) + bytes);
        }
    }
    v["all"]["disk"] = json!(disk);
    v["memory"] = memory();
    Ok(v)
}

/// The machine's memory, total and available, in bytes; empty off Linux.
fn memory() -> Value {
    let info = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let kb = |k: &str| {
        info.lines()
            .find_map(|l| l.strip_prefix(k))
            .and_then(|l| l.trim().trim_end_matches("kB").trim().parse::<u64>().ok())
            .map(|n| n * 1024)
    };
    json!({"total": kb("MemTotal:"), "available": kb("MemAvailable:")})
}

fn summarize(requests: &[(String, Value)], places: &[(String, String)]) -> Value {
    let mut projects: Vec<Value> = places
        .iter()
        .map(|(path, place)| {
            let name = Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(path);
            json!({"path": path, "name": name, "place": place, "disk": 0,
                   "requests": 0, "landed": 0, "failed": 0, "failed_attempts": 0,
                   "first_try": 0, "retried": 0})
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
            // Quality: proven work, at the first try or after failed ones.
            if matches!(r["status"].as_str(), Some("ready" | "landed")) {
                let key = match s["failed_attempts"].as_u64().unwrap_or(0) {
                    0 => "first_try",
                    _ => "retried",
                };
                bump(p, key, 1);
            }
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
        return Err(io::Error::other("this task is not ready for a commit"));
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
    // The work changed nothing: nothing commits, nothing pushes, nothing to
    // lose: the task closes where it stands.
    if request["files"].as_array().is_some_and(Vec::is_empty) {
        return store.update(id, |r| r["status"] = json!("landed"));
    }
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
        // The person's hooks look first; one refusing stops the commit.
        let hook = json!({"event": "before_land", "task": id, "project": project,
                          "branch": branch, "run": run});
        // Tests never reach the person's real plugins.
        let said = if cfg!(test) {
            Vec::new()
        } else {
            fennec_os::plugin::emit(&hook)?
        };
        for (name, said) in said {
            if let Err(e) = said {
                return store.update(id, |r| {
                    r["status"] = json!("ready");
                    r["land_error"] = json!(format!("not verified: plugin {name} refuses ({e})"));
                });
            }
        }
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
            r["land_error"] = json!(format!("the commit is done, but not pushed: {e}"));
        });
    }
    store.update(id, |r| r["status"] = json!("landed"))
}

fn fail_landing(store: &Store, id: &str, e: io::Error) -> io::Result<()> {
    let mut why = e.to_string();
    if why.contains(" after edits onto ") {
        why = format!(
            "your changes make the tests fail: fix them in the code folder, then commit again\n{why}"
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
                r["reason"] = json!("interrupted: fennec was stopped during the work");
            })?;
        } else if r["status"] == "waiting" {
            // Nothing started yet: it waits its turn again.
            spawn(id, solve_now);
        } else if r["status"] == "landing" {
            store.update(&id, |r| {
                r["status"] = json!("ready");
                r["land_error"] = json!("interrupted: fennec was stopped during the commit");
            })?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // FENNEC_OS_HOME is process-wide, so tests that point it at a temp dir
    // must not overlap: each holds this until its `Worlds::from_env` is done.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn a_failed_proof_tells_the_builder_what_failed() {
        let long = format!("{}uvicorn: command not found", "x".repeat(3000));
        let f = findings(&json!({
            "tries": [{"command": "make run", "exit": 127, "output": long}, {"command": "ls", "exit": 0, "output": "ok"}],
            "checks": [{"role": "QA", "command": "ruff check", "exit": 1, "output": "E501"},
                       {"role": "QA", "command": "semgrep", "exit": 127, "missing": true}],
            "shots": [{"url": "http://localhost:5173", "ok": false, "reason": "nothing listens"}]
        }));
        assert!(f.contains("`make run` exited 127") && f.ends_with("nothing listens"));
        assert!(
            f.contains("uvicorn: command not found") && f.len() < 2000,
            "only the end of the output"
        );
        assert!(
            f.contains("QA check `ruff check`") && !f.contains("semgrep"),
            "a missing tool is not sent back"
        );
        assert!(!f.contains("`ls`"));
        assert!(
            findings(&json!({"tries": [], "unverified": ["no try proposed"]}))
                .contains("no try proposed")
        );
    }

    #[test]
    fn a_fix_that_changes_nothing_holds_nothing_to_prove() {
        let fixed =
            |diff: &str| json!({"winner": "w2", "attempts": [{"world": "w2", "diff": diff}]});
        assert!(nothing_changed(&json!([]), &fixed("")));
        let orphan =
            json!({"winner": "w9", "attempts": [{"world": "w2", "diff": "diff --git a/b b/b\n"}]});
        assert!(nothing_changed(&json!([]), &orphan));
        assert!(!nothing_changed(&json!(["ui/app.js"]), &fixed("")));
        assert!(!nothing_changed(
            &json!([]),
            &fixed("diff --git a/ui/app.js b/ui/app.js\n")
        ));
    }

    #[test]
    fn a_task_that_changed_nothing_lands_without_committing() {
        let tmp =
            std::env::temp_dir().join(format!("fennec-lead-land-none-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let home = tmp.join("home");
        std::fs::create_dir_all(home.join("requests")).unwrap();
        let _env = ENV_LOCK.lock().unwrap();
        // SAFETY: this test holds ENV_LOCK across its env use.
        unsafe { std::env::set_var("FENNEC_OS_HOME", tmp.join("os")) };
        let id = "cafe";
        std::fs::write(
            home.join("requests/cafe.json"),
            json!({"project": "/p", "status": "ready", "run": "w1", "files": [],
                   "target": {"branch": "feat/x", "message": "m", "remote": "origin"}})
            .to_string(),
        )
        .unwrap();
        let store = Store { root: home.clone() };
        land_now(&store, id).unwrap();
        let r = store.load(id).unwrap();
        assert_eq!(r["status"], "landed");
        assert_eq!(r["commit"], Value::Null);
        assert_eq!(r["land_error"], Value::Null);
        std::fs::remove_dir_all(&tmp).unwrap();
    }

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
    fn an_answer_during_an_accepted_solve_restarts_through_retry() {
        let tmp = std::env::temp_dir().join(format!("fennec-lead-reply-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let os = tmp.join("os");
        let home = tmp.join("home");
        let project = tmp.join("p");
        std::fs::create_dir_all(home.join("requests")).unwrap();
        std::fs::create_dir_all(&project).unwrap();
        let _env = ENV_LOCK.lock().unwrap();
        // SAFETY: this test holds ENV_LOCK across its env use.
        unsafe { std::env::set_var("FENNEC_OS_HOME", &os) };
        let worlds = Worlds::from_env().unwrap();
        let spec_id = "s1";
        // The first solve accepted the spec (the chain had a winner) while a
        // role of the chain was still asking: the run keeps it accepted.
        let spec = json!({"project": project.to_string_lossy(), "request": "a task",
                          "world": "w0", "goal": "a task", "verify": [], "protect": [],
                          "status": "accepted", "run": "w1"});
        std::fs::create_dir_all(os.join("specs")).unwrap();
        std::fs::write(
            os.join("specs").join(format!("{spec_id}.json")),
            spec.to_string(),
        )
        .unwrap();
        let id = "cafe";
        std::fs::write(
            home.join("requests/cafe.json"),
            json!({"project": project.to_string_lossy(), "text": "a task",
                   "status": "blocked", "stage": "solving", "question": "Which port?",
                   "spec": spec_id, "thread": [{"who": "human", "text": "a task"}]})
            .to_string(),
        )
        .unwrap();
        let store = Store { root: home.clone() };
        reply(&store, id, "port 9").unwrap();
        let r = store.load(id).unwrap();
        assert_eq!(r["status"], "running");
        // Routed through spec::retry, which handles an accepted spec:
        // spec::accept would fail it with `spec is "accepted"`.
        assert_eq!(r["retry"], "port 9");
        let spec = fennec_os::spec::load(&worlds, spec_id).unwrap();
        assert_eq!(spec["answers"][0]["answer"], "port 9");
        std::fs::remove_dir_all(&tmp).unwrap();
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
    fn a_task_framed_ahead_takes_its_answer_and_stays_for_later() {
        let tmp = std::env::temp_dir().join(format!("fennec-lead-asked-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let store = Store { root: tmp.clone() };
        std::fs::create_dir_all(tmp.join("requests")).unwrap();
        std::fs::write(
            tmp.join("requests/1.json"),
            r#"{"project": "/p", "text": "un test", "status": "later", "asked": true,
                "thread": [{"who": "human", "text": "un test"}, {"who": "fennec", "text": "Un script ?"}]}"#,
        )
        .unwrap();
        assert_eq!(store.view().unwrap()["todo"][0]["kind"], "talk");
        say(&store, "1", "oui, en python").unwrap();
        let r = store.load("1").unwrap();
        assert_eq!(r["status"], "later");
        assert!(r["asked"].is_null(), "framed again when the model is free");
        assert_eq!(r["thread"][2]["text"], "oui, en python");
        assert!(say(&store, "1", "encore").is_err(), "nothing asked now");
        // Launched while it asks: the conversation goes on with the human.
        store.update("1", |r| r["asked"] = json!(true)).unwrap();
        frame_anyway(&store, "1").unwrap();
        let r = store.load("1").unwrap();
        assert_eq!(r["status"], "later");
        assert!(r["ahead"].as_str().unwrap().contains("oui, en python"));
        // What fennec understood, corrected: framed again, to validate again.
        say(&store, "1", "plutôt en rust").unwrap();
        let r = store.load("1").unwrap();
        assert!(r["ahead"].is_null() && r["done_when"].is_null());
        assert_eq!(r["status"], "later");
        store
            .update("1", |r| {
                r["ahead"] = Value::Null;
                r["asked"] = json!(true);
                r["status"] = json!("thinking");
            })
            .unwrap();
        talk_now(&store, "1").unwrap();
        assert_eq!(store.load("1").unwrap()["status"], "talk");
        std::fs::remove_dir_all(tmp).unwrap();
    }

    #[test]
    fn a_failed_request_goes_back_for_later_as_first_written() {
        let tmp = std::env::temp_dir().join(format!("fennec-lead-relater-{}", std::process::id()));
        let store = Store { root: tmp.clone() };
        std::fs::create_dir_all(tmp.join("requests")).unwrap();
        let id = "1".to_string();
        std::fs::write(
            tmp.join("requests/1.json"),
            r#"{"project": "/p", "text": "add filters", "status": "running", "fleet": true, "after": ["0"],
                "reason": "timeout", "journal": [{"stage": "said", "text": "x"}],
                "thread": [{"who": "human", "text": "add filters"}, {"who": "fennec", "text": "ok"}]}"#,
        )
        .unwrap();
        assert!(later(&store, &id).is_err(), "only a failed request");
        store
            .update(&id, |r| r["status"] = json!("failed"))
            .unwrap();
        later(&store, &id).unwrap();
        let r = store.load(&id).unwrap();
        assert_eq!(r["status"], "later");
        assert_eq!(r["text"], "add filters");
        assert_eq!(r["after"], json!(["0"]), "it still comes after");
        assert_eq!(r["thread"].as_array().unwrap().len(), 1);
        assert_eq!(
            r["journal"].as_array().unwrap().len(),
            1,
            "the Journal stays"
        );
        assert!(r["reason"].is_null() && r["fleet"].is_null());
        // Back for later, it can be reworded.
        store.rewrite(&id, "add two filters").unwrap();
        std::fs::remove_dir_all(tmp).unwrap();
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
            json!({"name": "Work, try 1", "model": "b", "cost": cost(200), "passed": false}),
            json!({"name": "Work, try 2", "model": "b", "cost": cost(300), "passed": true}),
            json!({"model": "b", "cost": cost(999), "live": true}),
        ];
        let s = totals(&steps);
        assert_eq!(s["sent"], 600);
        assert_eq!(s["cached"], 300);
        assert_eq!(s["failed_attempts"], 1);
        assert_eq!(s["models"]["b"]["sent"], 500);
        assert_eq!(s["stages"]["Work"]["turns"], 2);
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
        // Landed after one failed try: proven, but not at the first.
        assert_eq!(
            (p["first_try"].as_u64(), p["retried"].as_u64()),
            (Some(0), Some(1))
        );
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
        let t = totals(&[s.clone(), s.clone()]);
        assert_eq!(t["stages"]["?"]["roles"], json!(["QA", "Dev", "Gone"]));
        // A per-role work step names its role in the step: it counts for Work.
        let w = roles(
            json!({"name": "Work, try 1 · Dev", "messages": s["messages"].clone()}),
            &json!(["Dev"]),
        );
        let t = totals(&[w]);
        assert_eq!(t["stages"]["Work"]["roles"], json!(["Dev"]));
    }

    #[test]
    fn a_per_role_work_step_keeps_its_role_for_the_stats() {
        let tmp = std::env::temp_dir().join(format!("fennec-lead-per-role-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let os = tmp.join("os");
        let home = tmp.join("home");
        let project = tmp.join("p");
        std::fs::create_dir_all(home.join("requests")).unwrap();
        std::fs::create_dir_all(os.join("runs")).unwrap();
        std::fs::create_dir_all(&project).unwrap();
        let _env = ENV_LOCK.lock().unwrap();
        // SAFETY: this test holds ENV_LOCK across its env use.
        unsafe { std::env::set_var("FENNEC_OS_HOME", &os) };
        std::fs::write(
            os.join("runs/abcd.json"),
            json!({"team": [], "proof": {}, "attempts": [{"world": "w1", "passed": true,
                "roles": [{"name": "Dev senior", "model": "m", "turns": [],
                    "messages": [{"role": "system",
                        "content": "As Dev senior on the team:\nKeep it small."}]}]}]})
            .to_string(),
        )
        .unwrap();
        let id = "cafe";
        std::fs::write(
            home.join("requests/cafe.json"),
            json!({"project": project.to_string_lossy(), "text": "a task",
                   "status": "failed", "journal": [{"stage": "solve", "run": "abcd"}]})
            .to_string(),
        )
        .unwrap();
        let store = Store { root: home.clone() };
        let steps: Vec<Value> = journal(&store, id)
            .unwrap()
            .as_array()
            .unwrap()
            .clone()
            .into_iter()
            .collect();
        assert_eq!(steps[0]["name"], "Work, try 1 · Dev senior");
        assert_eq!(
            steps[0]["roles"][0],
            json!({"name": "Dev senior", "brief": "Keep it small."})
        );
        let t = totals(&steps);
        assert_eq!(t["stages"]["Work"]["roles"], json!(["Dev senior"]));
        std::fs::remove_dir_all(&tmp).unwrap();
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
    fn a_stopped_request_fails_and_the_stop_is_said_once() {
        let tmp = std::env::temp_dir().join(format!("fennec-lead-stop-{}", std::process::id()));
        let store = Store { root: tmp.clone() };
        std::fs::create_dir_all(tmp.join("requests")).unwrap();
        let id = "a1";
        std::fs::write(
            tmp.join("requests/a1.json"),
            r#"{"status": "waiting", "behind": "« x »"}"#,
        )
        .unwrap();
        stop(&store, id).unwrap();
        let r = store.load(id).unwrap();
        assert_eq!(r["status"], "failed");
        assert_eq!(r["reason"], "stopped: you asked to stop");
        assert!(r.get("behind").is_none());
        // Stopped: not worked on anymore, so no second stop, and recover leaves it.
        assert!(stop(&store, id).is_err());
        recover(&store).unwrap();
        assert_eq!(store.load(id).unwrap()["status"], "failed");
        std::fs::remove_dir_all(tmp).unwrap();
    }

    #[test]
    fn a_request_not_worked_on_cannot_be_stopped() {
        let tmp = std::env::temp_dir().join(format!("fennec-lead-stop2-{}", std::process::id()));
        let store = Store { root: tmp.clone() };
        std::fs::create_dir_all(tmp.join("requests")).unwrap();
        for (id, status) in [("b1", "later"), ("b2", "ready"), ("b3", "landed")] {
            std::fs::write(
                tmp.join(format!("requests/{id}.json")),
                format!(r#"{{"status": "{status}"}}"#),
            )
            .unwrap();
            let err = stop(&store, id).unwrap_err().to_string();
            assert!(err.contains("no longer works"), "{id}: {err}");
        }
        std::fs::remove_dir_all(tmp).unwrap();
    }

    #[test]
    fn a_task_starts_from_the_branch_its_dependency_was_committed_on() {
        let tmp = std::env::temp_dir().join(format!("fennec-lead-after-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let store = Store {
            root: tmp.join("home"),
        };
        std::fs::create_dir_all(store.root.join("requests")).unwrap();
        let p = tmp.join("p");
        std::fs::create_dir_all(&p).unwrap();
        store.add_project(p.to_str().unwrap(), "apart").unwrap();
        let p = std::fs::canonicalize(p).unwrap().display().to_string();
        let first = store.add_request(&p, "first", &[], true).unwrap();
        let next = store.add_request(&p, "next", &[], true).unwrap();
        store
            .update(&next, |r| r["after"] = json!([first]))
            .unwrap();
        let after = |s: &Store| after_branch(s, &s.load(&next).unwrap());
        assert_eq!(after(&store), None);
        store
            .update(&first, |r| {
                r["status"] = json!("landed");
                r["target"] = json!({"branch": "feat/a"});
            })
            .unwrap();
        assert_eq!(after(&store).as_deref(), Some("feat/a"));
        std::fs::remove_dir_all(tmp).unwrap();
    }

    #[test]
    fn a_split_task_gives_way_to_its_parts_in_order() {
        let tmp = std::env::temp_dir().join(format!("fennec-lead-split-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let store = Store {
            root: tmp.join("home"),
        };
        std::fs::create_dir_all(store.root.join("requests")).unwrap();
        let p = tmp.join("p");
        std::fs::create_dir_all(&p).unwrap();
        store.add_project(p.to_str().unwrap(), "apart").unwrap();
        let p = std::fs::canonicalize(p).unwrap().display().to_string();
        let first = store.add_request(&p, "first", &[], true).unwrap();
        let big = store
            .add_request(&p, "big", &["review".into()], true)
            .unwrap();
        let next = store.add_request(&p, "next", &[], true).unwrap();
        store.update(&big, |r| r["after"] = json!([first])).unwrap();
        store.update(&next, |r| r["after"] = json!([big])).unwrap();
        let parts = [("api".to_string(), vec![]), ("ui".to_string(), vec![0])];
        replace_by(&store, &big, &parts).unwrap();
        assert!(store.load(&big).is_err(), "the big one is gone");
        let all = store.requests().unwrap();
        let id = |t: &str| all.iter().find(|(_, r)| r["text"] == t).unwrap().0.clone();
        let (api, ui) = (id("api"), id("ui"));
        let r = store.load(&ui).unwrap();
        assert_eq!(
            (r["status"].as_str(), &r["off"]),
            (Some("later"), &json!(["review"]))
        );
        assert_eq!(r["after"], json!([first, api]));
        assert_eq!(store.load(&next).unwrap()["after"], json!([api, ui]));
        std::fs::remove_dir_all(tmp).unwrap();
    }

    #[test]
    fn a_stopped_worker_leaves_the_queue() {
        let tmp = std::env::temp_dir().join(format!("fennec-lead-stop3-{}", std::process::id()));
        let store = Store {
            root: tmp.join("home"),
        };
        std::fs::create_dir_all(store.root.join("requests")).unwrap();
        let project = tmp.join("p");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(
            store.root.join("requests/c3.json"),
            r#"{"status": "waiting"}"#,
        )
        .unwrap();
        stop(&store, "c3").unwrap();
        // The worker's next checkpoint leaves the queue with the stop's reason.
        let err = wait_turn(&store, "c3", project.to_str().unwrap()).unwrap_err();
        assert_eq!(err.to_string(), "stopped: you asked to stop");
        std::fs::remove_dir_all(tmp).unwrap();
    }

    #[test]
    fn a_world_no_request_owns_does_not_block_the_folder() {
        let tmp = std::env::temp_dir().join(format!("fennec-lead-orphan-{}", std::process::id()));
        let project = tmp.join("p");
        std::fs::create_dir_all(&project).unwrap();
        let git = |args: &[&str]| {
            let out = std::process::Command::new("git")
                .args(["-c", "user.name=t", "-c", "user.email=t@t"])
                .args(args)
                .current_dir(&project)
                .output()
                .unwrap();
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        git(&["init", "-q", "-b", "main"]);
        std::fs::write(project.join("a"), "a").unwrap();
        git(&["add", "."]);
        git(&["commit", "-q", "-m", "a"]);
        let _env = ENV_LOCK.lock().unwrap();
        // SAFETY: this test holds ENV_LOCK across its env use.
        unsafe { std::env::set_var("FENNEC_OS_HOME", tmp.join("os")) };
        // A start cut short: the world holds the folder, no request knows it.
        Worlds::from_env()
            .unwrap()
            .fork(project.to_str().unwrap())
            .unwrap();
        std::fs::write(project.join("half"), "done").unwrap();
        let store = Store {
            root: tmp.join("home"),
        };
        std::fs::create_dir_all(store.root.join("requests")).unwrap();
        std::fs::write(
            store.root.join("requests/c4.json"),
            r#"{"status": "waiting"}"#,
        )
        .unwrap();
        wait_turn(&store, "c4", project.to_str().unwrap()).unwrap();
        assert_eq!(git(&["branch", "--show-current"]), "main");
        assert_eq!(git(&["branch", "--list", "world/*"]), "");
        // What it held is set aside, not lost.
        assert!(git(&["stash", "list"]).contains("fennec"));
        std::fs::remove_dir_all(tmp).unwrap();
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
                .ends_with("does not show: no text")
        );
        let check = |exit: i32, missing: bool| {
            json!({"tries": [{"command": "a", "exit": 0}], "passed": missing,
                   "checks": [{"role": "Sec", "command": "audit", "exit": exit, "missing": missing}]})
        };
        apply_proof(&mut r, &check(1, false));
        assert_eq!(
            r["reason"],
            "the work is done, but Sec blocks: `audit` failed"
        );
        apply_proof(&mut r, &check(127, true));
        assert_eq!(r["status"], "ready", "a missing tool is not a veto");
    }

    #[test]
    fn a_second_turn_on_the_same_chat_is_refused() {
        let first = ChatTurn::new("c1".to_string());
        assert!(first.is_some(), "the first turn starts");
        assert!(
            ChatTurn::new("c1".to_string()).is_none(),
            "a second one is refused"
        );
        assert!(
            ChatTurn::new("c2".to_string()).is_some(),
            "another chat is not"
        );
        std::mem::drop(first);
        assert!(
            ChatTurn::new("c1".to_string()).is_some(),
            "the guard clears when the turn ends"
        );
    }

    #[test]
    fn a_refused_chat_turn_is_settled_with_a_reason() {
        let tmp =
            std::env::temp_dir().join(format!("fennec-lead-chatrefuse-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let home = tmp.join("home");
        let chats = home.join("chats");
        std::fs::create_dir_all(&chats).unwrap();
        let id = "c9";
        // As chat::say leaves it: the message is committed as thinking before
        // work::chat runs.
        std::fs::write(
            chats.join(format!("{id}.json")),
            json!({"id": id, "status": "thinking", "messages": []}).to_string(),
        )
        .unwrap();
        let _env = ENV_LOCK.lock().unwrap();
        // SAFETY: this test holds ENV_LOCK across its env use.
        unsafe { std::env::set_var("FENNEC_OS_HOME", &home) };
        let worlds = Worlds::from_env().unwrap();
        // A previous turn still holds the guard: this one is refused.
        let held = ChatTurn::new(id.to_string()).unwrap();
        chat(id.to_string());
        let c = fennec_os::chat::load(&worlds, id).unwrap();
        assert_eq!(c["status"], "idle", "a refused turn settles the chat");
        assert!(c["error"].is_string(), "and says why, so the page recovers");
        std::mem::drop(held);
        std::fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn a_worker_in_flight_is_not_retried_twice() {
        let live = Working::new("d1".to_string());
        assert!(working("d1"), "the worker says it is here");
        assert!(!working("d2"), "and only for its own request");
        std::mem::drop(live);
        assert!(!working("d1"), "the guard clears when the worker ends");
    }

    #[test]
    fn waiting_and_landing_are_retried_the_rest_not() {
        let tmp = std::env::temp_dir().join(format!("fennec-lead-retry-{}", std::process::id()));
        let store = Store { root: tmp.clone() };
        std::fs::create_dir_all(tmp.join("requests")).unwrap();
        for (id, status) in [
            ("c1", "waiting"),
            ("c2", "landing"),
            ("c3", "ready"),
            ("c4", "running"),
        ] {
            std::fs::write(
                tmp.join(format!("requests/{id}.json")),
                format!(r#"{{"status": "{status}"}}"#),
            )
            .unwrap();
        }
        // Nothing was pushed on c5: its work may be redone.
        std::fs::write(
            tmp.join("requests/c5.json"),
            r#"{"status": "ready", "land_error": "the push failed", "commit": "c9"}"#,
        )
        .unwrap();
        // The stuck ones start over; the rest is left where it is.
        retry(&store, "c1").unwrap();
        retry(&store, "c2").unwrap();
        retry(&store, "c5").unwrap();
        let c5 = store.load("c5").unwrap();
        assert_eq!(c5["status"], "framing");
        assert_eq!(c5["commit"], Value::Null);
        assert_eq!(c5["land_error"], Value::Null);
        assert!(retry(&store, "c3").is_err());
        assert!(retry(&store, "c4").is_err());
        assert!(retry(&store, "nope").is_err());
        std::fs::remove_dir_all(tmp).unwrap();
    }
}
