//! A task's sheet: the fields a human edits by hand, like a ticket, and a model may help fill.
//! "Ready" renders it as the plan's canonical markdown, the only format downstream readers parse.

use serde::{Deserialize, Serialize};

/// What a task must achieve and how it is judged done; only the objective and the criteria are required.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSheet {
    #[serde(default)]
    pub objective: String,
    #[serde(default)]
    pub context: String,
    #[serde(default)]
    pub constraints: String,
    #[serde(default)]
    pub out_of_scope: String,
    #[serde(default)]
    pub deliverable: String,
    #[serde(default)]
    pub approach: String,
    #[serde(default)]
    pub done_when: Vec<SheetCriterion>,
}

/// One completion criterion on a task sheet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetCriterion {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check: Option<String>,
}

/// The editable fields of a task sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SheetField {
    Objective,
    Context,
    Constraints,
    OutOfScope,
    Deliverable,
    Approach,
    DoneWhen,
}

impl SheetField {
    pub const ALL: [SheetField; 7] = [
        Self::Objective,
        Self::Context,
        Self::Constraints,
        Self::OutOfScope,
        Self::Deliverable,
        Self::Approach,
        Self::DoneWhen,
    ];
}

/// Who last changed a sheet field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Author {
    Human,
    Agent,
}

/// How complete a task sheet is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Completeness {
    pub required: u8,
    pub suggested: u8,
}

impl TaskSheet {
    /// The required fields that are not filled, in sheet order.
    pub fn missing(&self) -> Vec<SheetField> {
        let mut missing = Vec::new();
        if self.objective.trim().is_empty() {
            missing.push(SheetField::Objective);
        }
        if !self
            .done_when
            .iter()
            .any(|criterion| !criterion.text.trim().is_empty())
        {
            missing.push(SheetField::DoneWhen);
        }
        missing
    }

    /// Count the required and suggested fields that are filled.
    pub fn completeness(&self) -> Completeness {
        let required = 2 - self.missing().len() as u8;
        let suggested = [
            self.context.as_str(),
            self.constraints.as_str(),
            self.out_of_scope.as_str(),
            self.deliverable.as_str(),
        ]
        .iter()
        .filter(|value| !value.trim().is_empty())
        .count() as u8;

        Completeness {
            required,
            suggested,
        }
    }

    /// Whether one field is equal on two sheets.
    pub fn field_eq(&self, other: &TaskSheet, field: SheetField) -> bool {
        match field {
            SheetField::Objective => self.objective == other.objective,
            SheetField::Context => self.context == other.context,
            SheetField::Constraints => self.constraints == other.constraints,
            SheetField::OutOfScope => self.out_of_scope == other.out_of_scope,
            SheetField::Deliverable => self.deliverable == other.deliverable,
            SheetField::Approach => self.approach == other.approach,
            SheetField::DoneWhen => self.done_when == other.done_when,
        }
    }

    /// The plan's canonical markdown.
    pub fn to_markdown(&self, title: &str) -> String {
        let mut out = String::new();
        out.push_str("## Plan: ");
        out.push_str(title);
        out.push('\n');
        out.push('\n');
        out.push_str("Goal: ");
        out.push_str(self.objective.trim());
        out.push('\n');

        push_section(&mut out, "## Context", &self.context);
        push_section(&mut out, "## Constraints", &self.constraints);
        push_section(&mut out, "## Out of scope", &self.out_of_scope);
        push_section(&mut out, "## Deliverable", &self.deliverable);
        push_section(&mut out, "## Approach", &self.approach);

        if self
            .done_when
            .iter()
            .any(|criterion| !criterion.text.trim().is_empty())
        {
            out.push('\n');
            out.push_str("## Done when\n");
            for criterion in &self.done_when {
                let text = criterion.text.trim();
                if text.is_empty() {
                    continue;
                }
                out.push_str("- [ ] ");
                out.push_str(text);
                if let Some(check) = criterion.check.as_deref().map(str::trim)
                    && !check.is_empty()
                {
                    out.push_str(" — check: `");
                    out.push_str(check);
                    out.push('`');
                }
                out.push('\n');
            }
        }

        out
    }
}

fn push_section(out: &mut String, header: &str, value: &str) {
    let value = value.trim();
    if value.is_empty() {
        return;
    }
    out.push('\n');
    out.push_str(header);
    out.push('\n');
    out.push_str(value);
    out.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;

    fn criterion(text: &str) -> SheetCriterion {
        SheetCriterion {
            text: text.into(),
            check: None,
        }
    }

    #[test]
    fn missing_names_objective_and_done_when() {
        let both = vec![SheetField::Objective, SheetField::DoneWhen];
        assert_eq!(TaskSheet::default().missing(), both);
        let blank = TaskSheet {
            objective: "  ".into(),
            done_when: vec![criterion("")],
            ..Default::default()
        };
        assert_eq!(blank.missing(), both);
    }

    #[test]
    fn completeness_counts_required_and_suggested() {
        let sheet = TaskSheet {
            objective: "o".into(),
            context: "c".into(),
            done_when: vec![criterion("d")],
            ..Default::default()
        };
        assert_eq!(
            sheet.completeness(),
            Completeness {
                required: 2,
                suggested: 1
            }
        );
        let approach_only = TaskSheet {
            approach: "a".into(),
            ..Default::default()
        };
        assert_eq!(
            approach_only.completeness(),
            Completeness {
                required: 0,
                suggested: 0
            }
        );
    }

    #[test]
    fn markdown_has_the_plan_shape() {
        let sheet = TaskSheet {
            objective: "o".into(),
            context: "c".into(),
            constraints: "k".into(),
            out_of_scope: "x".into(),
            deliverable: "d".into(),
            approach: "a".into(),
            done_when: vec![
                SheetCriterion {
                    text: "c1".into(),
                    check: Some("check1".into()),
                },
                SheetCriterion {
                    text: "c2".into(),
                    check: None,
                },
            ],
        };

        assert_eq!(
            sheet.to_markdown("T"),
            "## Plan: T\n\nGoal: o\n\n## Context\nc\n\n## Constraints\nk\n\n## Out of scope\nx\n\n## Deliverable\nd\n\n## Approach\na\n\n## Done when\n- [ ] c1 — check: `check1`\n- [ ] c2\n"
        );
    }

    #[test]
    fn empty_sections_are_omitted() {
        let sheet = TaskSheet {
            objective: "o".into(),
            done_when: vec![criterion("c")],
            ..Default::default()
        };
        assert_eq!(
            sheet.to_markdown("T"),
            "## Plan: T\n\nGoal: o\n\n## Done when\n- [ ] c\n"
        );
    }

    #[test]
    fn serde_defaults_every_field() {
        let sheet: TaskSheet = serde_json::from_str(r#"{"objective":"o"}"#).unwrap();
        assert_eq!(
            sheet,
            TaskSheet {
                objective: "o".into(),
                ..Default::default()
            }
        );

        assert_eq!(
            serde_json::to_string(&SheetField::OutOfScope).unwrap(),
            "\"outOfScope\""
        );
        assert_eq!(serde_json::to_string(&Author::Agent).unwrap(), "\"agent\"");
    }
}
