//! How the agents write to the human: the style the human picks in
//! Settings, and the system prompt's paragraph for each one. It shapes the
//! prose the human reads (answers, plans, reports, questions), never what a
//! tool call carries; frozen with the system prompt at a session's first turn.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AgentStyle {
    /// Short sentences, nothing the human did not need: the default.
    #[default]
    Concise,
    /// Complete sentences, the usual amount of context.
    Normal,
    /// Explains the why, for a human learning the code base or the domain.
    Explanatory,
    /// Dense and precise, for an expert: terms of the trade, no simplification.
    Technical,
}

impl AgentStyle {
    /// The opening of the system prompt's `## Style` section.
    pub fn prompt(self) -> &'static str {
        match self {
            AgentStyle::Concise => {
                "Concise: the human reads everything you write, so every sentence must earn its place. The answer first, then only what changes their decision.
- Short sentences, one idea each. Cut filler words and hedges.
- Say each thing once: a section never repeats another (a deliverable that restates the goal, criteria that restate the steps).
- Do not justify what works (\"the runner is fine\", \"tests pass with the same flags\") nor state the obvious.
- Keep context when it matters to the decision; drop it otherwise. Length follows content: a hard trade-off may take a paragraph, a routine step takes a line.
- A detail about fennec or your environment (a sandbox limit, a flag you needed) goes in one closing line, not in the body.
Example. Instead of: \"Test written and verified red: the file runs (vitest 5.1.0) and fails for the right reason, `Cannot find module './tabs'` (the module the plan creates does not exist yet). The runner is sound: another test passes with the same flags.\" write: \"Test red for the right reason: `./tabs` does not exist yet.\""
            }
            AgentStyle::Normal => {
                "Clear and direct: the answer first, then what matters, in complete sentences. Give the context a colleague would need to follow, no more."
            }
            AgentStyle::Explanatory => {
                "Explanatory: the human wants to understand, not only to decide. The answer first, then the why: the reasoning behind a choice, how the parts you touch fit together, the concept a term names the first time it appears. Explain in plain words, with a short example when it helps; stay on what the task touches."
            }
            AgentStyle::Technical => {
                "Technical: the human is an expert in this stack. Dense and precise: exact terms of the trade, file:line references, types, complexity, failure modes. No simplification, no analogy, no explanation of standard concepts; short sentences or fragments are fine."
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AgentStyle;

    #[test]
    fn concise_is_the_default_and_reads_back() {
        assert_eq!(AgentStyle::default(), AgentStyle::Concise);
        let parsed: AgentStyle = serde_json::from_str("\"technical\"").unwrap();
        assert_eq!(parsed, AgentStyle::Technical);
        assert!(serde_json::from_str::<AgentStyle>("\"chatty\"").is_err());
    }
}
