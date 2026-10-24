//! Analysis tasks: a task's conversation (see [`crate::chat::link`]) that
//! changes nothing and ends on a report instead of a brief. A question on the
//! project, an architecture audit, or the adoption of a project fennec does
//! not know yet. The report is the model's: findings, suggestions, and
//! proposals for the project's knowledge, which only the human accepts and
//! only the kernel writes (see [`crate::knowledge`]).

use std::io;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::provider::Model;
use crate::world::Worlds;

/// What an analysis is asked: the kind is the human's choice, not the model's.
pub const KINDS: [&str; 3] = ["question", "audit", "adopt"];

/// A decision record as the kernel numbers and keeps it.
const ADR_FORMAT: &str = "A decision is a Markdown file `docs/adr/new-<slug>.md` (fennec gives \
it its number) in this form:\n# <title>\n\nStatus: accepted\n\n## Context\n<why a decision was \
needed>\n\n## Decision\n<what was decided>\n\n## Consequences\n<what follows, good and bad>\n\
A decision that replaces another says `Status: supersedes <its number>`.";

const ZONES_FORMAT: &str = "`.fennec-os/zones.json` is \
{\"zones\": [{\"name\": \"auth\", \"kind\": \"security\", \"paths\": [\"src/auth/**\"]}]} where \
kind is architecture, security or sensitive and paths are globs from the project root (`**` \
covers what is below). The vision, the zones file and the decisions are always protected: do \
not list them.";

const READ_ONLY: &str = "You are fennec's analyst on this project. Nothing is changed here: \
read with your tools (read-only), then end with the `report` tool, which is what the developer \
reads. Ask them first, with a reply without a tool call, only if what they want is unclear. \
Write the report in their language; keep file paths exact. You only propose: the developer \
decides.";

const QUESTION: &str = "\n\nThey ask a question about the project. Answer it in `summary`; \
put what supports the answer in `findings` (with files); `suggestions` only if one is worth it.";

const AUDIT: &str = "\n\nAudit the project's architecture. Extract the architecture the code \
actually has (modules, boundaries, direction of dependencies, patterns), and set it against the \
architecture vision and decisions below. `findings`: gaps, drift from the vision, risky zones, \
debt, each with its files and a severity. `suggestions`: argued and ranked, most valuable \
first; kind `task` for a change to the code, with `task` written as the developer would ask \
it; kind `knowledge` when the vision, the zones or the decisions should change, with the new \
full texts in `proposals`.";

const ADOPT: &str = "\n\nThis project is new to fennec: it has no architecture vision yet. \
Read it (tree, manifests, README, AGENTS.md, CLAUDE.md or .cursorrules if any, the main \
modules) and extract the architecture it actually has, not one it should have. Then `report` \
with these `proposals`:\n\
- `ARCHITECTURE.md`: the shape, the modules and their boundaries, the direction of \
dependencies, the conventions, where new things go. What is true today, concise, for a \
developer and for the agents that will work here.\n\
- `.fennec-os/zones.json`: the sensitive zones you see (authentication, secrets, crypto, \
payments, data migrations, dependency manifests and lockfiles...).\n\
- up to five decisions already visible in the code (a framework, a store, a layout), each its \
own file.\n\
`findings`: what the developer should know (risks, debt). `suggestions`: optional.";

/// The tool that ends an analysis.
pub fn report_tool() -> Value {
    json!({"type": "function", "function": {
        "name": "report",
        "description": "Your report, once you have read enough. Ends the analysis.",
        "parameters": {"type": "object", "required": ["summary"], "properties": {
            "summary": {"type": "string", "description": "the answer or the overview, a few paragraphs at most"},
            "findings": {"type": "array", "items": {"type": "object", "required": ["title"], "properties": {
                "title": {"type": "string"},
                "detail": {"type": "string"},
                "files": {"type": "array", "items": {"type": "string"}},
                "severity": {"type": "string", "enum": ["high", "medium", "low"]}}}},
            "suggestions": {"type": "array", "items": {"type": "object", "required": ["title", "kind"], "properties": {
                "title": {"type": "string"},
                "why": {"type": "string"},
                "kind": {"type": "string", "enum": ["task", "knowledge"]},
                "task": {"type": "string", "description": "for kind task: the task as the developer would ask it"}}}},
            "proposals": {"type": "array", "items": {"type": "object", "required": ["file", "content"], "properties": {
                "file": {"type": "string", "description": "ARCHITECTURE.md, .fennec-os/zones.json or docs/adr/<file>.md"},
                "content": {"type": "string", "description": "the file's whole new text"},
                "why": {"type": "string"}}}}
        }}
    }})
}

/// What the analysis chat is told, as its mode: the kind's ask and what the
/// project already knows of itself.
pub fn mode(project: &Path, kind: &str) -> io::Result<String> {
    let ask = match kind {
        "audit" => AUDIT,
        "adopt" => ADOPT,
        _ => QUESTION,
    };
    let formats = if kind == "question" {
        String::new()
    } else {
        format!("\n\n{ADR_FORMAT}\n\n{ZONES_FORMAT}")
    };
    Ok(format!(
        "{READ_ONLY}{ask}{formats}{}",
        crate::spec::vision_context(project)?
    ))
}

/// What a report of `kind` lacks, said back to the model, which goes on: an
/// adoption is its proposals for the vision and the zones (seen: a fine
/// report on the project that proposed nothing). Code decides, not the model.
pub fn lacking(kind: &str, report: &Value) -> Option<String> {
    if kind != "adopt" {
        return None;
    }
    let files: Vec<&str> = report["proposals"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|p| p["content"].as_str().is_some_and(|c| !c.trim().is_empty()))
        .filter_map(|p| p["file"].as_str())
        .collect();
    let missing: Vec<&str> = [crate::zones::VISION_FILE, crate::zones::ZONES_FILE]
        .into_iter()
        .filter(|f| !files.contains(f))
        .collect();
    (!missing.is_empty()).then(|| {
        format!(
            "refused: an adoption report must carry `proposals` with the whole text of {}; \
             call `report` again with them (and the decisions you see), keeping your findings",
            missing.join(" and ")
        )
    })
}

/// An analysis turn: a question back to the person, or the report.
pub enum Analysed {
    Reply(String),
    Report(Value),
}

/// One turn of an analysis task, `chat` its conversation (see
/// [`crate::chat::follow`]). Returns what was said and the exchange, for
/// the task's Journal.
pub fn analyse(
    worlds: &Worlds,
    chat: &str,
    thread: &[Value],
    model: &dyn Model,
) -> io::Result<(Analysed, Value)> {
    let before = crate::chat::follow(worlds, chat, thread)?;
    crate::chat::answer(worlds, chat, model, STEPS)?;
    let c = crate::chat::load(worlds, chat)?;
    let turns: Vec<Value> = c["turns"]
        .as_array()
        .map(|t| t[before.min(t.len())..].to_vec())
        .unwrap_or_default();
    let exchange = json!({"model": model.label(), "messages": c["messages"], "turns": turns});
    if let Some(why) = c["error"].as_str() {
        return Err(io::Error::other(why.to_string()));
    }
    if c["report"].is_object() {
        return Ok((Analysed::Report(tidy(&c["report"])), exchange));
    }
    let content = c["messages"]
        .as_array()
        .into_iter()
        .flatten()
        .rev()
        .find(|m| {
            m["role"] == "assistant" && m["tool_calls"].as_array().is_none_or(|t| t.is_empty())
        })
        .and_then(|m| m["content"].as_str())
        .unwrap_or("")
        .to_string();
    if content.is_empty() {
        return Err(io::Error::other("the model gave no answer"));
    }
    Ok((Analysed::Reply(content), exchange))
}

/// An analysis reads widely before it reports.
const STEPS: usize = 60;

/// The report as the page reads it: lists always lists, unknown kinds and
/// severities dropped to their defaults. Proposals are kept as said; the
/// kernel checks them when the human accepts one.
pub fn tidy(report: &Value) -> Value {
    let list = |k: &str| report[k].as_array().cloned().unwrap_or_default();
    let findings: Vec<Value> = list("findings")
        .into_iter()
        .filter(|f| f["title"].is_string())
        .map(|mut f| {
            if !matches!(f["severity"].as_str(), Some("high" | "medium" | "low")) {
                f["severity"] = json!("medium");
            }
            f
        })
        .collect();
    let suggestions: Vec<Value> = list("suggestions")
        .into_iter()
        .filter(|s| s["title"].is_string())
        .map(|mut s| {
            if s["kind"] != "knowledge" {
                s["kind"] = json!("task");
            }
            s
        })
        .collect();
    let proposals: Vec<Value> = list("proposals")
        .into_iter()
        .filter(|p| p["file"].is_string() && p["content"].is_string())
        .collect();
    json!({"summary": report["summary"].as_str().unwrap_or(""),
           "findings": findings, "suggestions": suggestions, "proposals": proposals})
}

const CAPITALISE: &str = "You keep this project's architecture knowledge up to date: its \
vision (ARCHITECTURE.md), its sensitive zones and its decision records. A change was just made \
(below). Most changes need nothing: then answer {\"proposals\": []}. Propose only when the \
change alters what the vision says (a new module or boundary, a dependency's direction, a \
convention, where things go), or embodies a decision worth recording (a new dependency, a \
pattern chosen over another). Answer with JSON only: {\"proposals\": [{\"file\": \"...\", \
\"content\": \"the file's whole new text\", \"why\": \"...\"}]}. Keep what is still true; \
the developer decides.";

/// The end of a change task's cycle: one ask, without tools, on the work
/// that passed its review. Its proposals for the knowledge, often none,
/// wait for the human next to the review; nothing is written here.
pub fn capitalise(
    worlds: &Worlds,
    run: &str,
    model: &dyn Model,
) -> io::Result<(Vec<Value>, Value)> {
    let record = crate::review::load(worlds, run)?;
    let diff = record["attempts"]
        .as_array()
        .and_then(|a| a.iter().find(|t| t["world"] == run))
        .and_then(|t| t["diff"].as_str())
        .unwrap_or("");
    if diff.trim().is_empty() {
        return Ok((Vec::new(), Value::Null));
    }
    let project = PathBuf::from(record["project"].as_str().unwrap_or(""));
    let system = format!(
        "{CAPITALISE}\n\n{ADR_FORMAT}\n\n{ZONES_FORMAT}{}",
        crate::spec::vision_context(&project)?
    );
    let ask = format!(
        "The task (its goal, design and plan):\n{}\n\nThe change (diff):\n{}",
        record["goal"].as_str().unwrap_or(""),
        crate::agent::truncate(diff)
    );
    let messages = [
        json!({"role": "system", "content": system}),
        json!({"role": "user", "content": ask}),
    ];
    let (answer, exchange) = crate::provider::ask(model, &messages)?;
    Ok((proposals_of(&answer, &project), exchange))
}

/// The proposals in a reply, each next to its file as it stands. A reply
/// that is not the asked JSON proposes nothing: the knowledge stays.
fn proposals_of(answer: &str, project: &Path) -> Vec<Value> {
    let (Some(from), Some(to)) = (answer.find('{'), answer.rfind('}')) else {
        return Vec::new();
    };
    let Ok(v) = serde_json::from_str::<Value>(&answer[from..=to]) else {
        return Vec::new();
    };
    tidy(&v)["proposals"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|mut p| {
            let file = p["file"].as_str().unwrap_or("").to_string();
            p["before"] = json!(current(project, &file));
            p["allowed"] = json!(crate::knowledge::is_knowledge(&file));
            p
        })
        .collect()
}

/// A knowledge file at HEAD, for the page to show a proposal as a diff.
pub fn current(project: &Path, file: &str) -> Option<String> {
    if !crate::knowledge::is_knowledge(file) {
        return None;
    }
    crate::world::git(&PathBuf::from(project), &["show", &format!("HEAD:{file}")]).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_report_is_tidied_for_the_page() {
        let r = tidy(&json!({"summary": "ok",
            "findings": [{"title": "a", "severity": "huge"}, {"detail": "no title"}],
            "suggestions": [{"title": "s", "kind": "weird"}, {"title": "k", "kind": "knowledge"}],
            "proposals": [{"file": "ARCHITECTURE.md", "content": "x"}, {"file": "a.md"}]}));
        assert_eq!(r["findings"].as_array().unwrap().len(), 1);
        assert_eq!(r["findings"][0]["severity"], "medium");
        assert_eq!(r["suggestions"][0]["kind"], "task");
        assert_eq!(r["suggestions"][1]["kind"], "knowledge");
        assert_eq!(r["proposals"].as_array().unwrap().len(), 1);
        assert_eq!(tidy(&json!({}))["findings"], json!([]));
    }

    #[test]
    fn a_capitalisation_reply_is_read_as_json_or_proposes_nothing() {
        let here = Path::new("/nonexistent");
        assert!(proposals_of("nothing to add", here).is_empty());
        assert!(proposals_of("{\"proposals\": []}", here).is_empty());
        let p = proposals_of(
            "```json\n{\"proposals\": [{\"file\": \"docs/adr/new-x.md\", \"content\": \"# X\"},\
             {\"file\": \"src/a.rs\", \"content\": \"y\"}]}\n```",
            here,
        );
        assert_eq!(p.len(), 2);
        assert_eq!(
            (p[0]["allowed"].clone(), p[1]["allowed"].clone()),
            (json!(true), json!(false))
        );
    }

    #[test]
    fn an_adoption_without_its_vision_and_zones_is_sent_back() {
        let bare = json!({"summary": "ok", "proposals": []});
        let why = lacking("adopt", &bare).unwrap();
        assert!(
            why.contains("ARCHITECTURE.md and .fennec-os/zones.json"),
            "{why}"
        );
        assert!(lacking("audit", &bare).is_none());
        let full = json!({"proposals": [{"file": "ARCHITECTURE.md", "content": "# v"},
                                        {"file": ".fennec-os/zones.json", "content": "{}"}]});
        assert!(lacking("adopt", &full).is_none());
    }

    #[test]
    fn each_kind_asks_its_own_thing() {
        let dir = std::env::temp_dir().join(format!("fennec-analysis-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(
            mode(&dir, "adopt")
                .unwrap()
                .contains("no architecture vision yet")
        );
        assert!(
            mode(&dir, "audit")
                .unwrap()
                .contains("Audit the project's architecture")
        );
        let q = mode(&dir, "question").unwrap();
        assert!(q.contains("They ask a question") && !q.contains("docs/adr/new-"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
