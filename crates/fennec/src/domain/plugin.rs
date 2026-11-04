//! Plugins: a human's extension to fennec. A plugin is untrusted code that
//! observes the session event stream and emits typed data blocks; the core
//! renders those blocks with its own primitives and nothing of the plugin is
//! executed in fennec's process or in the 127.0.0.1 origin. No I/O here: the
//! host, config and activation store live in the adapters.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Caps on what an untrusted plugin may emit, so a block cannot grow the
/// server's RAM. A `block.update` beyond these is dropped.
pub const MAX_CONTENT_BYTES: usize = 32 * 1024;
pub const MAX_ELEMENTS: usize = 200;
pub const MAX_BLOCKS: usize = 16;
/// The minimum interval, per block, between two accepted `block.update`s: an
/// untrusted plugin cannot flood the server with fresh data.
pub const MIN_UPDATE_INTERVAL_MS: u64 = 100;

/// One plugin as the human defined it in `~/.agents/plugins.json`. The code
/// it runs lives under `~/.agents/plugins/<name>/`; whether it is enabled,
/// and its network access, are not here — they are the human's store
/// decision (`PluginActivation`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PluginDef {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub blocks: Vec<PluginBlockDef>,
}

impl PluginDef {
    /// The problems with this definition, empty when it is usable.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.name.trim().is_empty() {
            out.push("the name is empty".into());
        } else if !self
            .name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            // The name becomes a path segment (code dir, writable scratch):
            // `..` or `/home/x` would point the sandbox's writable bind anywhere.
            out.push("the name may only hold letters, digits, - and _".into());
        }
        if self.command.trim().is_empty() {
            out.push("a plugin needs a command".into());
        }
        for block in &self.blocks {
            if block.id.trim().is_empty() {
                out.push("a block needs an id".into());
            }
        }
        out
    }
}

/// One block a plugin contributes, and where it shows.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PluginBlockDef {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub scope: BlockScope,
}

/// Where a block shows. A session block is per session (the session's left
/// bar); a global block is for the whole server (the Settings' Plugins tab).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BlockScope {
    Session,
    #[default]
    Global,
}

/// The human's decision to run one plugin, stored in SQLite: the definition's
/// path, a hash of its code, and the network access and enabled flag. A
/// changed definition or code makes the hash no longer match, deactivating it.
/// Activation is global: one row per plugin, for every project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginActivation {
    pub name: String,
    /// The plugin's code directory, e.g. `~/.agents/plugins/counter`.
    pub path: String,
    pub code_hash: String,
    pub network: bool,
    pub enabled: bool,
}

/// A block's data as a plugin emits it: an ordered list of typed elements.
/// The core renders these; nothing here is HTML.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockContent {
    pub elements: Vec<BlockElement>,
}

impl BlockContent {
    /// True when the content is within the caps an untrusted plugin may use:
    /// not too many elements, and not too large serialised.
    pub fn within_caps(&self) -> bool {
        self.elements.len() <= MAX_ELEMENTS
            && serde_json::to_string(self).is_ok_and(|json| json.len() <= MAX_CONTENT_BYTES)
    }
}

/// One typed element of a block, as the core renders it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum BlockElement {
    Text {
        text: String,
    },
    Figure {
        label: String,
        value: String,
        #[serde(default)]
        tone: Option<Tone>,
    },
    Badge {
        label: String,
        #[serde(default)]
        tone: Option<Tone>,
    },
}

/// A tint for a figure or badge, mapped to the core's tone primitives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    #[default]
    None,
    Info,
    Success,
    Warning,
    Danger,
}

/// Where a block's data is stored in the host: its id, and the session it is
/// for (a global block has none).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BlockKey {
    pub block_id: String,
    pub session_id: Option<String>,
}

/// A block as served to the UI: the plugin's name (the provenance badge), the
/// block's title and scope, and its data. The core renders it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServedPluginBlock {
    pub plugin: String,
    pub title: String,
    pub scope: BlockScope,
    pub content: BlockContent,
}

/// A `block.update` a plugin emits over the wire: data for one of its blocks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginBlockUpdate {
    pub block: String,
    #[serde(default)]
    pub session: Option<String>,
    pub content: BlockContent,
}

impl PluginBlockUpdate {
    /// The key this update writes, and whether it is within the caps.
    pub fn key(&self) -> BlockKey {
        BlockKey {
            block_id: self.block.clone(),
            session_id: self.session.clone(),
        }
    }

    pub fn within_caps(&self) -> bool {
        self.content.within_caps()
    }
}

/// A deterministic hash of the definition and the plugin's code files (by
/// relative path, then content). A changed definition or file yields a
/// different hash, so an activation no longer matches. Every field is length-prefixed so a change cannot
/// slide across a field boundary and keep the same hash.
pub fn code_hash(def: &PluginDef, files: &[(String, String)]) -> String {
    let mut hasher = Hasher::new();
    hasher.update_len(def.name.as_bytes());
    hasher.update_len(def.command.as_bytes());
    for arg in &def.args {
        hasher.update_len(arg.as_bytes());
    }
    for (key, value) in &def.env {
        hasher.update_len(key.as_bytes());
        hasher.update_len(value.as_bytes());
    }
    for (path, content) in files {
        hasher.update_len(path.as_bytes());
        hasher.update_len(content.as_bytes());
    }
    hasher.finish()
}

/// SHA-256 over length-prefixed fields: a model that can write the code
/// must not be able to forge a file with the activated hash.
struct Hasher(ring::digest::Context);

impl Hasher {
    fn new() -> Self {
        Self(ring::digest::Context::new(&ring::digest::SHA256))
    }
    /// A length-prefixed field: the boundary cannot move with the content.
    fn update_len(&mut self, bytes: &[u8]) {
        self.0.update(&(bytes.len() as u64).to_le_bytes());
        self.0.update(bytes);
    }
    fn finish(self) -> String {
        self.0
            .finish()
            .as_ref()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_definition_round_trips() {
        let def = PluginDef {
            name: "counter".into(),
            command: "python3".into(),
            args: vec!["counter.py".into()],
            env: BTreeMap::new(),
            blocks: vec![PluginBlockDef {
                id: "calls".into(),
                title: "Tool calls".into(),
                scope: BlockScope::Global,
            }],
        };
        let json = serde_json::to_string(&def).unwrap();
        let back: PluginDef = serde_json::from_str(&json).unwrap();
        assert_eq!(def, back);
    }

    #[test]
    fn a_name_that_leaves_its_directory_is_a_problem() {
        for name in ["..", "../x", "/home/x", "a/b", "a b"] {
            let def = PluginDef {
                name: name.into(),
                command: "python3".into(),
                ..Default::default()
            };
            assert!(!def.problems().is_empty(), "{name}");
        }
        let def = PluginDef {
            name: "tool-counter_2".into(),
            command: "python3".into(),
            ..Default::default()
        };
        assert!(def.problems().is_empty());
    }

    #[test]
    fn block_elements_use_the_documented_wire_names() {
        let content = BlockContent {
            elements: vec![
                BlockElement::Text { text: "hi".into() },
                BlockElement::Figure {
                    label: "calls".into(),
                    value: "3".into(),
                    tone: Some(Tone::Info),
                },
                BlockElement::Badge {
                    label: "ok".into(),
                    tone: None,
                },
            ],
        };
        let json = serde_json::to_string(&content).unwrap();
        assert!(json.contains("\"kind\":\"text\""));
        assert!(json.contains("\"kind\":\"figure\""));
        assert!(json.contains("\"tone\":\"info\""));
        let back: BlockContent = serde_json::from_str(&json).unwrap();
        assert_eq!(content, back);
    }

    #[test]
    fn a_content_beyond_the_caps_is_not_within_them() {
        let within = BlockContent {
            elements: vec![BlockElement::Text {
                text: "small".into(),
            }],
        };
        assert!(within.within_caps());

        let too_many = BlockContent {
            elements: vec![BlockElement::Text { text: "x".into() }; MAX_ELEMENTS + 1],
        };
        assert!(!too_many.within_caps());

        let too_large = BlockContent {
            elements: vec![BlockElement::Text {
                text: "y".repeat(MAX_CONTENT_BYTES),
            }],
        };
        assert!(!too_large.within_caps());
    }

    #[test]
    fn a_changed_definition_or_file_changes_the_hash() {
        let def = PluginDef {
            name: "counter".into(),
            command: "python3".into(),
            args: vec!["counter.py".into()],
            env: BTreeMap::new(),
            blocks: Vec::new(),
        };
        let files = vec![("counter.py".to_string(), "print(1)".to_string())];
        let base = code_hash(&def, &files);

        // A changed file content changes the hash.
        let files2 = vec![("counter.py".to_string(), "print(2)".to_string())];
        assert_ne!(code_hash(&def, &files2), base);

        // A changed command changes the hash.
        let mut changed_cmd = def.clone();
        changed_cmd.command = "python3.12".into();
        assert_ne!(code_hash(&changed_cmd, &files), base);

        // A change that slides across a field boundary (arg "a" + file "bc"
        // vs arg "ab" + file "c") still changes the hash: fields are
        // length-prefixed.
        let one = PluginDef {
            args: vec!["a".into()],
            ..def.clone()
        };
        let two = PluginDef {
            args: vec!["ab".into()],
            ..def.clone()
        };
        assert_ne!(
            code_hash(&one, &[("bc".into(), "x".into())]),
            code_hash(&two, &[("c".into(), "x".into())])
        );

        // The same definition and files give the same hash.
        assert_eq!(code_hash(&def, &files), base);
    }
}
