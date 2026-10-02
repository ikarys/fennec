//! Sleep: landed runs are distilled, offline, into candidate know-how. The
//! model proposes; a human keeps or dismisses. Kept know-how lives in the
//! project (`.fennec-os/knowhow.md`, versioned, shareable) and is read from
//! HEAD, like rules.

use std::io;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::provider::Model;
use crate::review::{self, normalise};
use crate::schedule;
use crate::world::{Worlds, git};

pub const KNOWHOW_FILE: &str = ".fennec-os/knowhow.md";
const PER_RUN: usize = 3;

const DISTILL: &str = "You read the trajectory of a coding agent that succeeded on a task. \
Extract at most 3 short facts specific to this project that would save a future agent time: \
commands that work, conventions, pitfalls it hit. Skip anything generic or task-specific. \
One fact per line, each starting with \"- \". If nothing is reusable, answer nothing.";

/// Kept know-how at the project's HEAD, one item per `- ` line.
pub fn knowhow(project: &Path) -> io::Result<Vec<String>> {
    let Ok(text) = git(project, &["show", &format!("HEAD:{KNOWHOW_FILE}")]) else {
        return Ok(Vec::new());
    };
    Ok(text
        .lines()
        .filter_map(|l| l.strip_prefix("- "))
        .map(str::to_string)
        .collect())
}

fn candidates_path(worlds: &Worlds) -> PathBuf {
    worlds.root.join("candidates.json")
}

/// Every candidate ever proposed, with its status: proposed, kept, dismissed.
pub fn candidates(worlds: &Worlds) -> io::Result<Vec<Value>> {
    match std::fs::read_to_string(candidates_path(worlds)) {
        Ok(text) => serde_json::from_str(&text).map_err(io::Error::other),
        Err(_) => Ok(Vec::new()),
    }
}

fn save_candidates(worlds: &Worlds, all: &[Value]) -> io::Result<()> {
    let text = serde_json::to_string_pretty(all).map_err(io::Error::other)?;
    std::fs::write(candidates_path(worlds), text)
}

/// Distils every landed run not yet distilled. Returns the new candidates.
pub fn sleep(worlds: &Worlds, model: &dyn Model) -> io::Result<Vec<Value>> {
    let mut all = candidates(worlds)?;
    let mut new = Vec::new();
    for (id, mut run) in review::runs(worlds)? {
        if run["status"] != "landed" || run["distilled"] == true {
            continue;
        }
        let project = run["project"].as_str().unwrap_or("").to_string();
        // Anything already proposed, dismissed or kept is not proposed again.
        let mut known: Vec<String> = all
            .iter()
            .filter(|c| c["project"] == project.as_str())
            .filter_map(|c| c["text"].as_str().map(normalise))
            .collect();
        known.extend(
            knowhow(Path::new(&project))
                .unwrap_or_default()
                .iter()
                .map(|k| normalise(k)),
        );
        schedule::yield_to_interactive(worlds)?;
        for text in distill(model, &run)? {
            let key = normalise(&text);
            if known.contains(&key) {
                continue;
            }
            known.push(key);
            let candidate = json!({
                "id": format!("k{}", all.len() + 1),
                "project": project,
                "run": id,
                "text": text,
                "status": "proposed",
            });
            all.push(candidate.clone());
            new.push(candidate);
        }
        run["distilled"] = json!(true);
        review::save(worlds, &id, &run)?;
        // Saved per run: a model failure later keeps what was learnt so far.
        save_candidates(worlds, &all)?;
    }
    Ok(new)
}

fn distill(model: &dyn Model, run: &Value) -> io::Result<Vec<String>> {
    let Some(winner) = run["attempts"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|a| a["world"] == run["winner"])
    else {
        return Ok(Vec::new());
    };
    let mut trajectory = format!("Task: {}\n", run["goal"].as_str().unwrap_or(""));
    for m in winner["messages"].as_array().into_iter().flatten() {
        for call in m["tool_calls"].as_array().into_iter().flatten() {
            let args: Value = call["function"]["arguments"]
                .as_str()
                .and_then(|a| serde_json::from_str(a).ok())
                .unwrap_or(Value::Null);
            match call["function"]["name"].as_str() {
                Some("shell") => {
                    trajectory.push_str(&format!("$ {}\n", args["command"].as_str().unwrap_or("")))
                }
                Some("write_file") => {
                    trajectory.push_str(&format!("write {}\n", args["path"].as_str().unwrap_or("")))
                }
                _ => {}
            }
        }
        if m["role"] == "tool" {
            let first = m["content"].as_str().unwrap_or("").lines().next();
            trajectory.push_str(&format!("  -> {}\n", first.unwrap_or("")));
        }
    }
    trajectory.push_str(&format!(
        "Agent's summary: {}\n",
        winner["summary"].as_str().unwrap_or("none")
    ));
    let messages = [
        json!({"role": "system", "content": DISTILL}),
        json!({"role": "user", "content": trajectory}),
    ];
    let reply = model.chat(&messages, &json!([]))?;
    let content = reply["content"].as_str().unwrap_or("");
    // Some servers leave the reasoning inline.
    let content = content.rsplit("</think>").next().unwrap_or(content);
    Ok(content
        .lines()
        .filter_map(|l| l.trim().strip_prefix("- "))
        .map(|l| l.trim().chars().take(200).collect::<String>())
        .filter(|l| !l.is_empty())
        .take(PER_RUN)
        .collect())
}

fn decide(worlds: &Worlds, id: &str, status: &str) -> io::Result<Value> {
    let mut all = candidates(worlds)?;
    let candidate = all
        .iter_mut()
        .find(|c| c["id"] == id && c["status"] == "proposed")
        .ok_or_else(|| io::Error::other(format!("no proposed candidate {id}")))?;
    candidate["status"] = json!(status);
    let decided = candidate.clone();
    save_candidates(worlds, &all)?;
    Ok(decided)
}

/// Appends the candidate to the project's know-how file. It takes effect
/// once the human commits it. Returns the file.
pub fn keep(worlds: &Worlds, id: &str) -> io::Result<PathBuf> {
    let candidate = decide(worlds, id, "kept")?;
    let path = Path::new(candidate["project"].as_str().unwrap_or("")).join(KNOWHOW_FILE);
    let mut text = std::fs::read_to_string(&path).unwrap_or_else(|_| "# Know-how\n\n".to_string());
    text.push_str(&format!("- {}\n", candidate["text"].as_str().unwrap_or("")));
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&path, text)?;
    Ok(path)
}

pub fn dismiss(worlds: &Worlds, id: &str) -> io::Result<()> {
    decide(worlds, id, "dismissed").map(|_| ())
}
