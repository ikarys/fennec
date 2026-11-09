//! The plan a plan agent proposes with `propose_plan`: fennec validates it,
//! renders it as the plan's canonical markdown, and downstream readers only
//! ever parse that markdown.

use super::criteria::{self, Criterion};
use super::event::{Event, Role};
use super::preflight::PreflightResult;
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

/// The plan still waiting for the human, served whole: the UI loads only the log's tail, so
/// it cannot look an older plan up by its id.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pending {
    pub call_id: String,
    pub plan: Plan,
    pub markdown: String,
    /// The plan's checks run at the end of the planner's turn, empty when none ran.
    pub preflight: Vec<PreflightResult>,
}

/// The last `plan.proposed`, unless a workflow started since or the human wrote after leaving
/// the plan agent. A later human question in plan mode keeps it pending: the decision stays
/// offered while they discuss.
pub fn pending(events: &[Event]) -> Option<Pending> {
    let mut pending: Option<Pending> = None;
    let mut left_plan = false;

    for event in events {
        match event {
            Event::PlanProposed {
                call_id,
                plan,
                markdown,
            } => {
                pending = Some(Pending {
                    call_id: call_id.clone(),
                    plan: plan.clone(),
                    markdown: markdown.clone(),
                    preflight: Vec::new(),
                });
                left_plan = false;
            }
            Event::CriteriaPreflight { results, .. } => {
                if let Some(pending) = pending.as_mut() {
                    pending.preflight = results.clone();
                }
            }
            Event::WorkflowStarted { .. } => pending = None,
            Event::ModeChanged { mode, .. } => {
                left_plan = mode.as_str() != "plan";
            }
            Event::MessageStart {
                role: Role::User,
                is_system_generated: false,
                ..
            } if left_plan => pending = None,
            _ => {}
        }
    }

    pending
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

    fn pending_id(events: &[Event]) -> Option<String> {
        pending(events).map(|pending| pending.call_id)
    }

    fn proposed(id: &str) -> Event {
        Event::PlanProposed {
            call_id: id.into(),
            plan: Plan {
                title: String::new(),
                goal: String::new(),
                steps: vec![],
                done_when: vec![],
                risks: vec![],
            },
            markdown: String::new(),
        }
    }

    fn human(text: &str) -> Event {
        Event::MessageStart {
            message_id: "m".into(),
            role: Role::User,
            content: Some(text.into()),
            is_system_generated: false,
            metadata: None,
            attachments: vec![],
        }
    }

    fn mode(m: &str) -> Event {
        Event::ModeChanged {
            mode: m.into(),
            auto: false,
            reason: None,
        }
    }

    fn started() -> Event {
        Event::WorkflowStarted {
            run_id: "r".into(),
            workflow: serde_json::from_value(serde_json::json!({
                "id": "w",
                "name": "w",
                "nodes": []
            }))
            .unwrap(),
            task: String::new(),
            step_by_step: false,
            started_at: None,
        }
    }

    #[test]
    fn no_plan_none_pending() {
        assert_eq!(pending_id(&[]), None);
    }

    #[test]
    fn a_proposed_plan_is_pending() {
        assert_eq!(pending_id(&[proposed("p1")]), Some("p1".into()));
    }

    #[test]
    fn a_question_in_plan_mode_keeps_it_pending() {
        assert_eq!(
            pending_id(&[proposed("p1"), human("why?")]),
            Some("p1".into())
        );
    }

    #[test]
    fn a_newer_plan_replaces_the_older() {
        assert_eq!(
            pending_id(&[proposed("p1"), proposed("p2")]),
            Some("p2".into())
        );
    }

    #[test]
    fn a_started_workflow_clears_it() {
        assert_eq!(pending_id(&[proposed("p1"), started()]), None);
    }

    #[test]
    fn writing_after_leaving_plan_clears_it() {
        assert_eq!(
            pending_id(&[proposed("p1"), mode("builder"), human("go")]),
            None
        );
    }

    #[test]
    fn switching_back_to_plan_keeps_it() {
        assert_eq!(
            pending_id(&[proposed("p1"), mode("builder"), mode("plan"), human("hm")]),
            Some("p1".into())
        );
    }

    #[test]
    fn switching_away_without_writing_keeps_it() {
        assert_eq!(
            pending_id(&[proposed("p1"), mode("builder")]),
            Some("p1".into())
        );
    }

    #[test]
    fn carries_the_preflight_of_its_turn() {
        let result = PreflightResult {
            criterion_id: "c1".into(),
            command: "cargo test".into(),
            verdict: super::super::preflight::PreflightVerdict::Fails,
            exit_code: Some(1),
            output: String::new(),
            existence_only: false,
        };
        let preflight = Event::CriteriaPreflight {
            message_id: "m".into(),
            results: vec![result.clone()],
        };
        let served = pending(&[proposed("p1"), preflight]).unwrap();
        assert_eq!(served.preflight, vec![result]);
        assert!(pending(&[proposed("p1")]).unwrap().preflight.is_empty());
    }
}
