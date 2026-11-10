//! The framing plan mapped onto a task sheet: the human's "Apply to the sheet".

use super::plan::Plan;
use backlog::{SheetCriterion, TaskSheet};

/// The sheet the plan describes; a field the plan leaves empty keeps `current`'s value.
pub fn sheet_from_plan(plan: &Plan, current: &TaskSheet) -> TaskSheet {
    let approach = plan
        .steps
        .iter()
        .enumerate()
        .map(|(index, step)| format!("{}. {}: {}", index + 1, step.title, step.change))
        .collect::<Vec<_>>()
        .join("\n");
    let mut context = plan.context.clone();
    if !plan.risks.is_empty() {
        if !context.is_empty() {
            context.push_str("\n\n");
        }
        let risks = plan.risks.iter().map(|risk| format!("- {risk}"));
        context.push_str(&format!("Risks:\n{}", risks.collect::<Vec<_>>().join("\n")));
    }
    let done_when = plan
        .done_when
        .iter()
        .map(|criterion| SheetCriterion {
            text: criterion.text.clone(),
            check: criterion.check.clone(),
        })
        .collect::<Vec<_>>();
    let keep = |new: String, old: &String| if new.is_empty() { old.clone() } else { new };
    TaskSheet {
        objective: keep(plan.goal.clone(), &current.objective),
        context: keep(context, &current.context),
        constraints: keep(plan.constraints.clone(), &current.constraints),
        out_of_scope: keep(plan.out_of_scope.clone(), &current.out_of_scope),
        deliverable: keep(plan.deliverable.clone(), &current.deliverable),
        approach: keep(approach, &current.approach),
        done_when: if done_when.is_empty() {
            current.done_when.clone()
        } else {
            done_when
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::plan::{PlanStep, PlannedCriterion};

    fn plan() -> Plan {
        Plan {
            title: "t".into(),
            goal: "Export costs".into(),
            context: "CSV".into(),
            constraints: "No new dependency".into(),
            out_of_scope: "The UI".into(),
            deliverable: "A command".into(),
            steps: vec![
                PlanStep {
                    title: "Write".into(),
                    files: vec![],
                    change: "the writer".into(),
                },
                PlanStep {
                    title: "Wire".into(),
                    files: vec![],
                    change: "the route".into(),
                },
            ],
            done_when: vec![PlannedCriterion {
                text: "It exports".into(),
                check: Some("cargo test".into()),
            }],
            risks: vec!["Big files".into(), "Locale".into()],
        }
    }

    #[test]
    fn maps_every_plan_field() {
        let sheet = sheet_from_plan(&plan(), &TaskSheet::default());
        assert_eq!(
            sheet,
            TaskSheet {
                objective: "Export costs".into(),
                context: "CSV\n\nRisks:\n- Big files\n- Locale".into(),
                constraints: "No new dependency".into(),
                out_of_scope: "The UI".into(),
                deliverable: "A command".into(),
                approach: "1. Write: the writer\n2. Wire: the route".into(),
                done_when: vec![SheetCriterion {
                    text: "It exports".into(),
                    check: Some("cargo test".into()),
                }],
            }
        );
    }

    #[test]
    fn an_empty_plan_field_keeps_the_sheet() {
        let current = TaskSheet {
            objective: "old".into(),
            context: "kept context".into(),
            approach: "kept approach".into(),
            done_when: vec![SheetCriterion {
                text: "kept".into(),
                check: None,
            }],
            ..Default::default()
        };
        let plan = Plan {
            context: String::new(),
            steps: vec![],
            done_when: vec![],
            risks: vec![],
            ..plan()
        };
        let sheet = sheet_from_plan(&plan, &current);
        assert_eq!(sheet.objective, "Export costs");
        assert_eq!(sheet.context, "kept context");
        assert_eq!(sheet.approach, "kept approach");
        assert_eq!(sheet.done_when, current.done_when);
    }
}
