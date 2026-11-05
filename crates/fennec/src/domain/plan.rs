//! The plan a plan agent proposes with `propose_plan`: fennec validates it,
//! renders it as the plan's canonical markdown, and downstream readers only
//! ever parse that markdown.

use super::criteria::{self, Criterion};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub title: String,
    pub goal: String,
    pub steps: Vec<PlanStep>,
    pub done_when: Vec<PlannedCriterion>,
    pub risks: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanStep {
    pub title: String,
    /// Empty when the step changes no file: the `Files:` line is omitted.
    pub files: Vec<String>,
    pub change: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedCriterion {
    pub text: String,
    /// A read-only command that exits 0 once the criterion holds; None when
    /// only a reader can judge it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check: Option<String>,
}

impl Plan {
    /// The plan's canonical markdown, exactly the format of
    /// `agents/plan.agent.md`: the only format fennec's readers parse.
    pub fn to_markdown(&self) -> String {
        let mut lines = vec![
            format!("## Plan: {}", self.title),
            String::new(),
            format!("Goal: {}", self.goal),
        ];
        for (index, step) in self.steps.iter().enumerate() {
            lines.push(String::new());
            lines.push(format!("### Step {}: {}", index + 1, step.title));
            if !step.files.is_empty() {
                let files = step
                    .files
                    .iter()
                    .map(|file| format!("`{file}`"))
                    .collect::<Vec<_>>()
                    .join(", ");
                lines.push(format!("- Files: {files}"));
            }
            lines.push(format!("- Change: {}", step.change));
        }
        lines.push(String::new());
        lines.push("## Done when".to_string());
        for criterion in &self.done_when {
            match &criterion.check {
                Some(check) => lines.push(format!("- [ ] {} — check: `{check}`", criterion.text)),
                None => lines.push(format!("- [ ] {}", criterion.text)),
            }
        }
        lines.push(String::new());
        lines.push("## Risks and open questions".to_string());
        if self.risks.is_empty() {
            lines.push("- None".to_string());
        } else {
            for risk in &self.risks {
                lines.push(format!("- {risk}"));
            }
        }
        lines.push(String::new());
        lines.join("\n")
    }

    /// The plan's criteria as fennec's readers see them: parsed from the
    /// rendered markdown, so the ids match what the criteria node reads from
    /// the node's output.
    pub fn criteria(&self) -> Vec<Criterion> {
        criteria::parse_done_when(&self.to_markdown())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::plan_scope;

    fn plan() -> Plan {
        Plan {
            title: "Add the plan tool".into(),
            goal: "The plan agent ends its turn with a propose_plan call".into(),
            steps: vec![
                PlanStep {
                    title: "Write the domain".into(),
                    files: vec!["src/domain/plan.rs".into()],
                    change: "The plan's vocabulary and its canonical markdown".into(),
                },
                PlanStep {
                    title: "Wire the tool".into(),
                    files: Vec::new(),
                    change: "The loop runs it and records the plan".into(),
                },
            ],
            done_when: vec![
                PlannedCriterion {
                    text: "The build passes".into(),
                    check: Some("cargo build".into()),
                },
                PlannedCriterion {
                    text: "The code stays clean".into(),
                    check: None,
                },
            ],
            risks: vec!["The model may still write the plan as text".into()],
        }
    }

    #[test]
    fn markdown_round_trips_the_criteria() {
        let markdown = plan().to_markdown();
        let criteria = criteria::parse_done_when(&markdown);
        assert_eq!(
            criteria,
            vec![
                Criterion {
                    id: "C1".into(),
                    description: "The build passes".into(),
                    check: Some("cargo build".into()),
                },
                Criterion {
                    id: "C2".into(),
                    description: "The code stays clean".into(),
                    check: None,
                },
            ]
        );
        assert_eq!(plan().criteria(), criteria);
    }

    #[test]
    fn markdown_round_trips_the_files() {
        let markdown = plan().to_markdown();
        assert_eq!(
            plan_scope::plan_files(&markdown),
            vec!["src/domain/plan.rs".to_string()]
        );
    }

    #[test]
    fn a_plan_without_risks_renders_none() {
        let mut p = plan();
        p.risks.clear();
        let markdown = p.to_markdown();
        assert!(markdown.contains("## Risks and open questions\n- None"));
    }

    #[test]
    fn a_step_without_files_omits_the_files_line() {
        let markdown = plan().to_markdown();
        let step = markdown
            .split("### Step 2: Wire the tool")
            .nth(1)
            .unwrap()
            .split("## Done when")
            .next()
            .unwrap();
        assert!(!step.contains("Files:"));
        assert!(step.contains("- Change: The loop runs it and records the plan"));
    }

    #[test]
    fn serde_uses_camel_case_and_skips_the_absent_check() {
        let value = serde_json::to_value(plan()).unwrap();
        assert_eq!(
            value,
            serde_json::json!({
                "title": "Add the plan tool",
                "goal": "The plan agent ends its turn with a propose_plan call",
                "steps": [
                    {
                        "title": "Write the domain",
                        "files": ["src/domain/plan.rs"],
                        "change": "The plan's vocabulary and its canonical markdown"
                    },
                    {
                        "title": "Wire the tool",
                        "files": [],
                        "change": "The loop runs it and records the plan"
                    }
                ],
                "doneWhen": [
                    {"text": "The build passes", "check": "cargo build"},
                    {"text": "The code stays clean"}
                ],
                "risks": ["The model may still write the plan as text"]
            })
        );
        let back: Plan = serde_json::from_value(value).unwrap();
        assert_eq!(back, plan());
    }
}
