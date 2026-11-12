//! The report a builder agent submits with `submit_report`: fennec validates
//! it, renders it in the builder's "Final report" format, and the turn ends.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub title: String,
    pub summary: String,
    /// What the work found or measured that the human asked to know; empty
    /// when the task asked for no result.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub findings: String,
    pub steps: Vec<ReportStep>,
    pub done_when: Vec<ReportCriterion>,
    pub changed: Vec<ChangedFile>,
    /// What the human should try by hand; empty when there is nothing.
    pub test: String,
    /// What remains or was found on the way; empty when there is nothing.
    pub todo: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportStep {
    pub title: String,
    /// True only for a step the agent verified.
    pub done: bool,
    /// What was done, in one line; why it is not done, when `done` is false.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportCriterion {
    pub text: String,
    /// True only for a criterion the agent verified.
    pub met: bool,
    /// How the agent verified it; None when there is none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub how: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangedFile {
    pub path: String,
    pub what: String,
}

impl Report {
    /// The report's canonical markdown, exactly the format of
    /// `agents/builder.agent.md`'s "Final report": the format the UI's
    /// legacy text path reads back for old sessions.
    pub fn to_markdown(&self) -> String {
        let mut lines = vec![
            format!("## Report: {}", self.title),
            String::new(),
            format!("Summary: {}", self.summary),
            String::new(),
        ];
        if !self.findings.is_empty() {
            lines.push("### Findings".to_string());
            lines.push(self.findings.clone());
            lines.push(String::new());
        }
        lines.push("### Steps".to_string());
        for (index, step) in self.steps.iter().enumerate() {
            let box_ = if step.done { "x" } else { " " };
            match &step.note {
                Some(note) => lines.push(format!(
                    "- [{box_}] Step {}: {} — {note}",
                    index + 1,
                    step.title
                )),
                None => lines.push(format!("- [{box_}] Step {}: {}", index + 1, step.title)),
            }
        }
        lines.push(String::new());
        lines.push("### Done when".to_string());
        for criterion in &self.done_when {
            let box_ = if criterion.met { "x" } else { " " };
            match &criterion.how {
                Some(how) => lines.push(format!("- [{box_}] {} — {how}", criterion.text)),
                None => lines.push(format!("- [{box_}] {}", criterion.text)),
            }
        }
        lines.push(String::new());
        lines.push("### Changed".to_string());
        if self.changed.is_empty() {
            lines.push("- None".to_string());
        } else {
            for file in &self.changed {
                lines.push(format!("- `{}`: {}", file.path, file.what));
            }
        }
        lines.push(String::new());
        lines.push("### Next".to_string());
        lines.push(format!("- Test: {}", next_value(&self.test)));
        lines.push(format!("- To do: {}", next_value(&self.todo)));
        lines.join("\n")
    }
}

/// The `Next` section renders `Nothing` for an empty field.
fn next_value(value: &str) -> &str {
    if value.is_empty() { "Nothing" } else { value }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report() -> Report {
        Report {
            title: "Add the report tool".into(),
            summary: "The builder ends its turn with a submit_report call".into(),
            findings: String::new(),
            steps: vec![
                ReportStep {
                    title: "Write the domain".into(),
                    done: true,
                    note: Some("The report's vocabulary and its markdown".into()),
                },
                ReportStep {
                    title: "Wire the tool".into(),
                    done: false,
                    note: None,
                },
            ],
            done_when: vec![
                ReportCriterion {
                    text: "The build passes".into(),
                    met: true,
                    how: Some("just check".into()),
                },
                ReportCriterion {
                    text: "The code stays clean".into(),
                    met: false,
                    how: None,
                },
            ],
            changed: vec![
                ChangedFile {
                    path: "crates/fennec/src/domain/report.rs".into(),
                    what: "The report's vocabulary".into(),
                },
                ChangedFile {
                    path: "crates/fennec/src/agent/submit_report.rs".into(),
                    what: "The loop's tool".into(),
                },
            ],
            test: String::new(),
            todo: String::new(),
        }
    }

    #[test]
    fn markdown_matches_the_builder_format_exactly() {
        assert_eq!(
            report().to_markdown(),
            "## Report: Add the report tool\n\
             \n\
             Summary: The builder ends its turn with a submit_report call\n\
             \n\
             ### Steps\n\
             - [x] Step 1: Write the domain — The report's vocabulary and its markdown\n\
             - [ ] Step 2: Wire the tool\n\
             \n\
             ### Done when\n\
             - [x] The build passes — just check\n\
             - [ ] The code stays clean\n\
             \n\
             ### Changed\n\
             - `crates/fennec/src/domain/report.rs`: The report's vocabulary\n\
             - `crates/fennec/src/agent/submit_report.rs`: The loop's tool\n\
             \n\
             ### Next\n\
             - Test: Nothing\n\
             - To do: Nothing"
        );
    }

    #[test]
    fn findings_render_before_steps_when_present() {
        let mut r = report();
        r.findings = "p50 12 ms\np99 40 ms".into();
        let markdown = r.to_markdown();
        assert!(markdown.contains("### Findings\np50 12 ms\np99 40 ms\n\n### Steps"));
    }

    #[test]
    fn an_empty_findings_field_renders_no_findings_section() {
        let markdown = report().to_markdown();
        assert!(!markdown.contains("### Findings"));
    }

    #[test]
    fn an_empty_changed_list_renders_none() {
        let mut r = report();
        r.changed.clear();
        let markdown = r.to_markdown();
        assert!(markdown.contains("### Changed\n- None\n"));
    }

    #[test]
    fn empty_test_and_todo_render_nothing() {
        let mut r = report();
        r.test = "Open the UI and read the report".into();
        let markdown = r.to_markdown();
        assert!(markdown.contains("- Test: Open the UI and read the report"));
        assert!(markdown.contains("- To do: Nothing"));
    }

    #[test]
    fn serde_uses_camel_case_and_skips_the_absent_note_and_how() {
        let value = serde_json::to_value(report()).unwrap();
        assert_eq!(
            value,
            serde_json::json!({
                "title": "Add the report tool",
                "summary": "The builder ends its turn with a submit_report call",
                "steps": [
                    {"title": "Write the domain", "done": true, "note": "The report's vocabulary and its markdown"},
                    {"title": "Wire the tool", "done": false}
                ],
                "doneWhen": [
                    {"text": "The build passes", "met": true, "how": "just check"},
                    {"text": "The code stays clean", "met": false}
                ],
                "changed": [
                    {"path": "crates/fennec/src/domain/report.rs", "what": "The report's vocabulary"},
                    {"path": "crates/fennec/src/agent/submit_report.rs", "what": "The loop's tool"}
                ],
                "test": "",
                "todo": ""
            })
        );
        let back: Report = serde_json::from_value(value).unwrap();
        assert_eq!(back, report());
    }
}
