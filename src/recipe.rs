//! Recipes: the tool calls of a landed run, replayed without the model when
//! the same goal comes back. A replay is an attempt like any other, judged by
//! the same rules; if it fails, the models run.

use std::io;
use std::path::Path;

use serde_json::{Value, json};

use crate::provider::Model;
use crate::review::{self, normalise};
use crate::world::Worlds;

/// Plays back recorded assistant turns, one per call. Stateless: the turn to
/// play is the number of assistant turns already in the conversation.
pub struct Recipe {
    pub run: String,
    turns: Vec<Value>,
}

impl Model for Recipe {
    fn chat(&self, messages: &[Value], _tools: &Value) -> io::Result<Value> {
        let played = messages.iter().filter(|m| m["role"] == "assistant").count();
        Ok(self.turns.get(played).cloned().unwrap_or_else(|| {
            json!({"role": "assistant", "content": "", "tool_calls": [{
                "id": "recipe-end", "type": "function",
                "function": {"name": "done", "arguments": "{\"summary\": \"replayed\"}"},
            }]})
        }))
    }

    fn label(&self) -> String {
        format!("recipe:{}", self.run)
    }
}

/// The recipe for this goal, if the latest run on it landed and its trajectory
/// was recorded whole.
// ponytail: same goal after normalisation only; a goal that differs by one
// word runs the models. Matching by intent needs the intent compiler.
pub fn find(worlds: &Worlds, project: &Path, goal: &str) -> io::Result<Option<Recipe>> {
    let project = project.display().to_string();
    let key = normalise(goal);
    let Some((id, run)) = review::runs(worlds)?.into_iter().rev().find(|(_, r)| {
        r["project"] == project.as_str() && r["goal"].as_str().map(normalise) == Some(key.clone())
    }) else {
        return Ok(None);
    };
    // A rejected or failed latest run means the old way no longer holds.
    if run["status"] != "landed" {
        return Ok(None);
    }
    let Some(winner) = run["attempts"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|a| a["world"] == run["winner"])
    else {
        return Ok(None);
    };
    let turns: Vec<Value> = winner["messages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|m| m["role"] == "assistant" && m["tool_calls"].is_array())
        .map(|m| json!({"role": "assistant", "content": "", "tool_calls": m["tool_calls"]}))
        .collect();
    if turns.is_empty() || turns.iter().any(elided) {
        return Ok(None);
    }
    Ok(Some(Recipe { run: id, turns }))
}

/// Context bounding replaced a written file with a marker: not replayable.
fn elided(turn: &Value) -> bool {
    turn["tool_calls"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| c["function"]["arguments"].as_str())
        .filter_map(|a| serde_json::from_str::<Value>(a).ok())
        .filter_map(|a| a["content"].as_str().map(str::to_string))
        .any(|c| c.starts_with('[') && c.ends_with(" bytes elided]"))
}
