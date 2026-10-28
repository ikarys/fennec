//! What the model's context is made of, part by part, for the context gauge.
//! Measured in bytes, since fennec has no tokenizer: `spread` shares the exact
//! token count among the parts pro rata, so a part's figure is indicative.

use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

/// Which side of the context a part belongs to, so a view can group them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PartKind {
    /// The frozen system prompt and tool definitions.
    Prompt,
    /// What the human and the model said.
    Conversation,
    /// Tool calls and what they returned.
    Tools,
}

#[derive(Debug, PartialEq)]
pub struct ContextPart {
    pub kind: PartKind,
    pub label: String,
    pub bytes: u64,
}

#[derive(Debug, PartialEq, Default)]
pub struct ContextParts {
    /// Largest first; parts of zero bytes are left out.
    pub parts: Vec<ContextPart>,
    /// Images are tokens no byte count tells; they are only counted.
    pub images: u64,
}

pub fn context_parts(system_prompt: &str, tools: &Value, messages: &[Value]) -> ContextParts {
    let mut bytes_by_label: Parts = BTreeMap::new();
    let mut images = 0u64;

    for (label, bytes) in prompt_parts(system_prompt) {
        add(&mut bytes_by_label, PartKind::Prompt, &label, bytes);
    }
    if !tools.is_null() {
        add(
            &mut bytes_by_label,
            PartKind::Prompt,
            "Définitions d'outils",
            tools.to_string().len() as u64,
        );
    }

    let mut tool_names: HashMap<String, String> = HashMap::new();
    for message in messages {
        let role = message["role"].as_str().unwrap_or("");
        let (bytes, image_count) = content_bytes(&message["content"]);
        images += image_count;

        match role {
            "user" => add(
                &mut bytes_by_label,
                PartKind::Conversation,
                "Messages humains",
                bytes,
            ),
            "assistant" => {
                add(
                    &mut bytes_by_label,
                    PartKind::Conversation,
                    "Réponses du modèle",
                    bytes,
                );
                if let Some(reasoning) = message["reasoning_content"].as_str() {
                    add(
                        &mut bytes_by_label,
                        PartKind::Conversation,
                        "Raisonnement",
                        reasoning.len() as u64,
                    );
                }
                if let Some(calls) = message["tool_calls"].as_array() {
                    let mut call_bytes = 0u64;
                    for call in calls {
                        let name = call["function"]["name"].as_str().unwrap_or("");
                        let arguments = call["function"]["arguments"].as_str().unwrap_or("");
                        call_bytes += name.len() as u64 + arguments.len() as u64;
                        if let Some(id) = call["id"].as_str() {
                            tool_names.insert(id.to_string(), name.to_string());
                        }
                    }
                    add(
                        &mut bytes_by_label,
                        PartKind::Tools,
                        "Appels d'outils",
                        call_bytes,
                    );
                }
            }
            "tool" => {
                let id = message["tool_call_id"].as_str().unwrap_or("");
                let label = match tool_names.get(id) {
                    Some(name) if name == "load_skill" => "Skills chargés".to_string(),
                    Some(name) => format!("Résultat : {name}"),
                    None => "Résultat : ?".to_string(),
                };
                add(&mut bytes_by_label, PartKind::Tools, &label, bytes);
            }
            _ => {}
        }
    }

    let mut parts: Vec<ContextPart> = bytes_by_label
        .into_iter()
        .filter(|(_, (_, bytes))| *bytes > 0)
        .map(|(label, (kind, bytes))| ContextPart { kind, label, bytes })
        .collect();
    parts.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.label.cmp(&b.label)));

    ContextParts { parts, images }
}

/// Each part's share of `total` tokens, rounded; same order as the parts.
pub fn spread(parts: &[ContextPart], total: u64) -> Vec<u64> {
    let total_bytes: u64 = parts.iter().map(|part| part.bytes).sum();
    if total_bytes == 0 {
        return vec![0; parts.len()];
    }
    parts
        .iter()
        .map(|part| (total as f64 * part.bytes as f64 / total_bytes as f64).round() as u64)
        .collect()
}

fn prompt_parts(prompt: &str) -> BTreeMap<String, u64> {
    let mut parts: BTreeMap<String, u64> = BTreeMap::new();
    let mut attributed = 0u64;

    let skills_start = prompt.find("\n## AVAILABLE SKILLS");
    let (prefix, skills_bytes) = match skills_start {
        Some(start) => (&prompt[..start], (prompt.len() - start) as u64),
        None => (prompt, 0),
    };
    if skills_bytes > 0 {
        parts.insert("Liste des skills".to_string(), skills_bytes);
        attributed += skills_bytes;
    }

    let custom_header = "\n## CUSTOM INSTRUCTIONS\n\n";
    let custom_start = prefix.find(custom_header);
    let custom = match custom_start {
        Some(start) => &prefix[start + custom_header.len()..],
        None => "",
    };

    if let Some(global_start) = custom.find("## GLOBAL INSTRUCTIONS") {
        let file_header = "## FILE INSTRUCTIONS";
        let global_end = custom[global_start..]
            .find(file_header)
            .map(|offset| global_start + offset)
            .unwrap_or(custom.len());
        let global_bytes = (global_end - global_start) as u64;
        parts.insert("Instructions globales".to_string(), global_bytes);
        attributed += global_bytes;
    }

    if let Some(file_start) = custom.find("## FILE INSTRUCTIONS") {
        let file_section = &custom[file_start + "## FILE INSTRUCTIONS\n\n".len()..];
        let mut start = 0usize;
        while let Some(offset) = file_section[start..].find("Instructions from: ") {
            let block_start = start + offset;
            // The next file's line, searched past this one's own.
            let block_end = file_section[block_start + 1..]
                .find("Instructions from: ")
                .map(|offset| block_start + 1 + offset)
                .unwrap_or(file_section.len());
            let block = &file_section[block_start..block_end];
            let path = block
                .strip_prefix("Instructions from: ")
                .and_then(|rest| rest.split('\n').next())
                .unwrap_or("");
            let label = file_label(path);
            *parts.entry(label).or_default() += block.len() as u64;
            attributed += block.len() as u64;
            start = block_end;
        }
    }

    let base_bytes = prompt.len() as u64 - attributed;
    if base_bytes > 0 {
        parts.insert("Prompt de base".to_string(), base_bytes);
    }

    parts
}

/// The file name alone: the directories above are often a worktree's
/// generated name. Files of the same name sum up.
fn file_label(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string())
}

fn content_bytes(content: &Value) -> (u64, u64) {
    match content {
        Value::String(text) => (text.len() as u64, 0),
        Value::Array(parts) => {
            let mut bytes = 0u64;
            let mut images = 0u64;
            for part in parts {
                match part["type"].as_str() {
                    Some("image_url") => images += 1,
                    Some("text") => {
                        bytes += part["text"].as_str().unwrap_or("").len() as u64;
                    }
                    _ => {}
                }
            }
            (bytes, images)
        }
        _ => (0, 0),
    }
}

/// Bytes by label; a label always has the same kind.
type Parts = BTreeMap<String, (PartKind, u64)>;

fn add(parts: &mut Parts, kind: PartKind, label: &str, bytes: u64) {
    if bytes > 0 {
        parts.entry(label.to_string()).or_insert((kind, 0)).1 += bytes;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_prompt_with_global_files_and_skills_is_split_by_label() {
        let prompt = [
            "base",
            "\n## CUSTOM INSTRUCTIONS\n\n",
            "## GLOBAL INSTRUCTIONS\n\n",
            "global",
            "\n## FILE INSTRUCTIONS\n\n",
            "Instructions from: /home/u/proj/AGENTS.md\n",
            "file one\n",
            "Instructions from: /home/u/proj/docs/RULES.md\n",
            "file two\n",
            "\n## AVAILABLE SKILLS\n\n",
            "skills",
        ]
        .join("");
        let parts = context_parts(&prompt, &Value::Null, &[]);

        let labels: Vec<&str> = parts.parts.iter().map(|part| part.label.as_str()).collect();
        assert!(labels.contains(&"Prompt de base"));
        assert!(labels.contains(&"Instructions globales"));
        assert!(labels.contains(&"AGENTS.md"));
        assert!(labels.contains(&"RULES.md"));
        assert!(labels.contains(&"Liste des skills"));

        let global_start = prompt.find("## GLOBAL INSTRUCTIONS").unwrap();
        let file_start = prompt.find("## FILE INSTRUCTIONS").unwrap();
        let global = parts
            .parts
            .iter()
            .find(|part| part.label == "Instructions globales")
            .unwrap();
        assert_eq!(global.bytes, (file_start - global_start) as u64);

        let first_file = prompt.find("Instructions from: ").unwrap();
        let second_file = prompt[first_file + 1..]
            .find("Instructions from: ")
            .unwrap()
            + first_file
            + 1;
        let file_one = parts
            .parts
            .iter()
            .find(|part| part.label == "AGENTS.md")
            .unwrap();
        assert_eq!(file_one.bytes, (second_file - first_file) as u64);

        assert_eq!(
            parts.parts.iter().map(|part| part.bytes).sum::<u64>(),
            prompt.len() as u64
        );
    }

    #[test]
    fn messages_are_split_by_role_and_tool_result() {
        let messages = vec![
            json!({"role": "user", "content": "hello"}),
            json!({
                "role": "assistant",
                "content": "answer",
                "reasoning_content": "think",
                "tool_calls": [
                    {
                        "id": "c1",
                        "type": "function",
                        "function": {"name": "read_file", "arguments": "{\"path\":\"x\"}"}
                    },
                    {
                        "id": "c2",
                        "type": "function",
                        "function": {"name": "load_skill", "arguments": "{\"skill\":\"s\"}"}
                    }
                ]
            }),
            json!({"role": "tool", "tool_call_id": "c1", "content": "result one"}),
            json!({"role": "tool", "tool_call_id": "c2", "content": "result two"}),
            json!({
                "role": "user",
                "content": [
                    {"type": "text", "text": "look"},
                    {"type": "image_url", "image_url": {"url": "data:image/png;base64,AAAA"}}
                ]
            }),
        ];

        let parts = context_parts("", &Value::Null, &messages);
        assert_eq!(parts.images, 1);

        let labels: Vec<&str> = parts.parts.iter().map(|part| part.label.as_str()).collect();
        assert!(labels.contains(&"Messages humains"));
        assert!(labels.contains(&"Réponses du modèle"));
        assert!(labels.contains(&"Raisonnement"));
        assert!(labels.contains(&"Appels d'outils"));
        assert!(labels.contains(&"Résultat : read_file"));
        assert!(labels.contains(&"Skills chargés"));
    }

    #[test]
    fn spread_shares_the_tokens_pro_rata() {
        let parts = [
            ContextPart {
                kind: PartKind::Prompt,
                label: "a".into(),
                bytes: 30,
            },
            ContextPart {
                kind: PartKind::Prompt,
                label: "b".into(),
                bytes: 10,
            },
        ];
        assert_eq!(spread(&parts, 8), vec![6, 2]);
    }

    #[test]
    fn spread_of_zero_bytes_is_zero() {
        let parts = [
            ContextPart {
                kind: PartKind::Prompt,
                label: "a".into(),
                bytes: 0,
            },
            ContextPart {
                kind: PartKind::Prompt,
                label: "b".into(),
                bytes: 0,
            },
        ];
        assert_eq!(spread(&parts, 5), vec![0, 0]);
    }
}
