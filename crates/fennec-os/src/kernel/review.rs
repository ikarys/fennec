//! The review queue: runs that wait on a human, and the corrections that
//! become rules. Say the same correction twice and the harness asks to write
//! it down once (`rules::promote`).

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};

use crate::world::Worlds;

fn run_path(worlds: &Worlds, id: &str) -> io::Result<PathBuf> {
    if id.is_empty() || id.contains('/') {
        return Err(io::Error::other(format!("no run {id}")));
    }
    Ok(worlds.root.join("runs").join(format!("{id}.json")))
}

pub fn load(worlds: &Worlds, id: &str) -> io::Result<Value> {
    let text = std::fs::read_to_string(run_path(worlds, id)?)
        .map_err(|_| io::Error::other(format!("no run {id}")))?;
    serde_json::from_str(&text).map_err(io::Error::other)
}

pub(crate) fn save(worlds: &Worlds, id: &str, record: &Value) -> io::Result<()> {
    let text = serde_json::to_string_pretty(record).map_err(io::Error::other)?;
    write_atomic(&run_path(worlds, id)?, &text)
}

/// Readers (the review page) load records while jobs save them: written
/// aside then renamed, a record is seen whole, old or new, never half.
pub(crate) fn write_atomic(path: &Path, text: &str) -> io::Result<()> {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let tmp = path.with_extension(format!("tmp{}-{n}", std::process::id()));
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

/// Marks a run; a run without a record (a hand-made world) is left alone.
pub fn set_status(worlds: &Worlds, id: &str, status: &str) -> io::Result<()> {
    let Ok(mut record) = load(worlds, id) else {
        return Ok(());
    };
    record["status"] = json!(status);
    save(worlds, id, &record)
}

/// Every run record, oldest first.
pub fn runs(worlds: &Worlds) -> io::Result<Vec<(String, Value)>> {
    let Ok(entries) = std::fs::read_dir(worlds.root.join("runs")) else {
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
    // Ids are hex timestamps of the same width: lexical order is time order.
    ids.sort();
    ids.into_iter()
        .map(|id| load(worlds, &id).map(|r| (id, r)))
        .collect()
}

/// Runs waiting on a human: `ready` (a world to land) or `failed`.
pub fn queue(worlds: &Worlds) -> io::Result<Vec<(String, Value)>> {
    Ok(runs(worlds)?
        .into_iter()
        .filter(|(_, r)| matches!(r["status"].as_str(), Some("ready" | "failed")))
        .collect())
}

/// Rejects a run, dropping its world if one waits to land. A correction is
/// recorded against the project; returns how many times the same correction
/// has now been given there.
pub fn reject(worlds: &Worlds, id: &str, correction: Option<&str>) -> io::Result<usize> {
    let mut record = load(worlds, id)?;
    if let Ok(world) = worlds.open(id) {
        worlds.drop(world)?;
    }
    let _ = std::fs::remove_file(worlds.root.join("passed").join(id));
    record["status"] = json!("rejected");
    let Some(text) = correction.filter(|c| !c.trim().is_empty()) else {
        save(worlds, id, &record)?;
        return Ok(0);
    };
    record["correction"] = json!(text);
    save(worlds, id, &record)?;

    let project = record["project"].as_str().unwrap_or("");
    let log = worlds.root.join("corrections.jsonl");
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log)?;
    writeln!(
        file,
        "{}",
        json!({"run": id, "project": project, "correction": text})
    )?;
    // ponytail: "the same" is exact after normalisation; paraphrases count
    // apart until a model proposes groupings for a human to confirm.
    let key = normalise(text);
    let count = std::fs::read_to_string(&log)?
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|c| {
            c["project"] == project && c["correction"].as_str().map(normalise) == Some(key.clone())
        })
        .count();
    Ok(count)
}

pub(crate) fn normalise(text: &str) -> String {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// What a new run on this project should know without being told: what
/// landed, what was corrected, what failed. Built from run records, oldest
/// first, the latest `limit` entries.
pub fn agenda(worlds: &Worlds, project: &str, limit: usize) -> io::Result<Vec<String>> {
    let mut items = Vec::new();
    for (_, r) in runs(worlds)? {
        if r["project"] != project {
            continue;
        }
        let goal = r["goal"]
            .as_str()
            .unwrap_or("")
            .lines()
            .next()
            .unwrap_or("");
        let winner = r["attempts"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|a| a["world"] == r["winner"]);
        let item = match r["status"].as_str() {
            Some("landed") => format!(
                "landed \"{goal}\": {}",
                winner
                    .and_then(|w| w["summary"].as_str())
                    .unwrap_or("no summary")
            ),
            Some("rejected") => match r["correction"].as_str() {
                Some(c) => format!("rejected \"{goal}\", correction: {c}"),
                None => continue,
            },
            Some("failed") => {
                let reasons: Vec<&str> = r["attempts"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|a| a["reason"].as_str())
                    .collect();
                format!("failed \"{goal}\": {}", reasons.join("; "))
            }
            _ => continue,
        };
        // ponytail: one line per run, cut at 300 chars; a sidekick model can
        // summarise better once there is one.
        items.push(item.chars().take(300).collect());
    }
    let skip = items.len().saturating_sub(limit);
    Ok(items.split_off(skip))
}

/// Where an attempt's time went, from its timed turns and checks. Anything
/// left of the wall time is the harness itself (fork, commit, diff, rules).
/// `out_tok_per_s` divides output tokens by model time, which includes
/// prefill and queueing: a floor on decode speed, not the decode speed.
pub fn profile(attempt: &Value) -> Value {
    let turns: Vec<&Value> = attempt["turns"].as_array().into_iter().flatten().collect();
    let num = |v: &Value| v.as_u64().unwrap_or(0);
    let model_ms: u64 = turns.iter().map(|t| num(&t["model_ms"])).sum();
    let tool_ms: u64 = turns
        .iter()
        .flat_map(|t| t["tools"].as_array().into_iter().flatten())
        .map(|t| num(&t["ms"]))
        .sum();
    let verify_ms: u64 = attempt["checks"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|c| num(&c["ms"]))
        .sum();
    let usage = |k: &str| -> u64 { turns.iter().map(|t| num(&t["usage"][k])).sum() };
    let cached: u64 = turns
        .iter()
        .map(|t| num(&t["usage"]["prompt_tokens_details"]["cached_tokens"]))
        .sum();
    let reasoning: u64 = turns
        .iter()
        .map(|t| num(&t["usage"]["completion_tokens_details"]["reasoning_tokens"]))
        .sum();
    let (input, output) = (usage("prompt_tokens"), usage("completion_tokens"));
    let wall_ms = attempt["ms"].as_u64();
    json!({
        "wall_ms": wall_ms,
        "model_ms": model_ms,
        "tool_ms": tool_ms,
        "verify_ms": verify_ms,
        "harness_ms": wall_ms.map(|w| w.saturating_sub(model_ms + tool_ms + verify_ms)),
        "turns": turns.len(),
        "prompt_tokens": input,
        "cached_tokens": cached,
        "completion_tokens": output,
        "reasoning_tokens": reasoning,
        "out_tok_per_s": (model_ms > 0).then(|| output * 1000 / model_ms),
    })
}

/// One line for humans: `wall 84.0s = model 70.1s + tools 6.2s + ...`.
pub fn profile_line(p: &Value) -> String {
    let s = |k: &str| format!("{:.2}s", p[k].as_u64().unwrap_or(0) as f64 / 1000.0);
    let wall = match p["wall_ms"].as_u64() {
        Some(_) => format!("wall {} = ", s("wall_ms")),
        None => String::new(),
    };
    let harness = match p["harness_ms"].as_u64() {
        Some(_) => format!(" + harness {}", s("harness_ms")),
        None => String::new(),
    };
    let speed = match p["out_tok_per_s"].as_u64() {
        Some(t) => format!("  ≥{t} tok/s"),
        None => String::new(),
    };
    format!(
        "{wall}model {} + tools {} + verify {}{harness}  {} turns  in {} (cached {}) out {} (reasoning {}){speed}",
        s("model_ms"),
        s("tool_ms"),
        s("verify_ms"),
        p["turns"],
        p["prompt_tokens"],
        p["cached_tokens"],
        p["completion_tokens"],
        p["reasoning_tokens"],
    )
}
