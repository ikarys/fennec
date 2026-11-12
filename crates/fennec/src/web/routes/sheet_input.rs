//! A task sheet request's validation and the event it yields, kept pure so it is unit-tested.

use serde_json::{Map, Value};

use crate::domain::plan::Plan;
use crate::domain::plan_sheet;
use backlog::{Author, BacklogEvent, Idea, IdeaState, SheetCriterion, SheetField, TaskSheet};

pub const MAX_CRITERIA: usize = 20;
pub const MAX_CHECK_CHARS: usize = 500;

#[derive(Debug)]
pub struct Refusal {
    pub status: u16,
    pub message: String,
    pub missing: Vec<SheetField>,
}

pub fn parse_sheet(value: &Value, max_chars: usize) -> Result<TaskSheet, String> {
    let Value::Object(map) = value else {
        return Err("sheet must be an object".to_string());
    };

    Ok(TaskSheet {
        objective: text_field(map, "objective", max_chars)?,
        context: text_field(map, "context", max_chars)?,
        constraints: text_field(map, "constraints", max_chars)?,
        out_of_scope: text_field(map, "outOfScope", max_chars)?,
        deliverable: text_field(map, "deliverable", max_chars)?,
        approach: text_field(map, "approach", max_chars)?,
        done_when: criteria(map, max_chars)?,
    })
}

pub fn sheet_event(
    idea: &Idea,
    action: &str,
    body: &Value,
    max_chars: usize,
    pending: Option<&Plan>,
) -> Option<Result<BacklogEvent, Refusal>> {
    let ready_with_sheet = action == "ready" && idea.sheet.is_some();
    if !matches!(action, "sheet" | "sheet-apply" | "turn-into-task") && !ready_with_sheet {
        return None;
    }

    if matches!(
        idea.state,
        IdeaState::Started | IdeaState::Dropped | IdeaState::Completed
    ) {
        return Some(Err(refusal(
            409,
            "the idea is no longer a draft",
            Vec::new(),
        )));
    }

    match action {
        "sheet" => Some(
            parse_sheet(&body["sheet"], max_chars)
                .map(|sheet| {
                    let by = if body.get("by").and_then(Value::as_str) == Some("agent") {
                        Author::Agent
                    } else {
                        Author::Human
                    };
                    BacklogEvent::SheetEdited {
                        idea_id: idea.id.clone(),
                        sheet,
                        by,
                    }
                })
                .map_err(|message| refusal(400, &message, Vec::new())),
        ),
        "sheet-apply" => Some(match (idea.sheet.as_ref(), pending) {
            (None, _) => Err(refusal(409, "the idea is not a task", Vec::new())),
            (Some(_), None) => Err(refusal(409, "no plan is waiting to be applied", Vec::new())),
            (Some(current), Some(plan)) => Ok(BacklogEvent::SheetEdited {
                idea_id: idea.id.clone(),
                sheet: plan_sheet::sheet_from_plan(plan, current),
                by: Author::Agent,
            }),
        }),
        "turn-into-task" => Some(if idea.sheet.is_some() {
            Err(refusal(409, "the idea is already a task", Vec::new()))
        } else {
            let sheet = TaskSheet {
                objective: idea.text.clone(),
                ..Default::default()
            };
            Ok(BacklogEvent::SheetEdited {
                idea_id: idea.id.clone(),
                sheet,
                by: Author::Human,
            })
        }),
        "ready" => {
            let sheet = idea.sheet.as_ref()?;
            let missing = sheet.missing();
            Some(if missing.is_empty() {
                let plan = sheet.to_markdown(&backlog::title(&idea.text, None));
                Ok(BacklogEvent::Ready {
                    idea_id: idea.id.clone(),
                    plan,
                })
            } else {
                Err(refusal(400, "the task sheet is incomplete", missing))
            })
        }
        _ => None,
    }
}

fn refusal(status: u16, message: &str, missing: Vec<SheetField>) -> Refusal {
    Refusal {
        status,
        message: message.to_string(),
        missing,
    }
}

fn text_field(map: &Map<String, Value>, key: &str, max_chars: usize) -> Result<String, String> {
    let text = map.get(key).and_then(Value::as_str).unwrap_or("").trim();
    if text.chars().count() > max_chars {
        return Err(format!("{key} too long"));
    }
    // The sheet is rendered as plan markdown whose sections are level-2 headings; a "## " line
    // would inject a section (markdown section injection).
    if text
        .lines()
        .any(|line| line.trim_start().starts_with("## "))
    {
        return Err(format!(
            "{key}: a line starting with \"## \" would split the plan"
        ));
    }
    Ok(text.to_string())
}

fn criteria(map: &Map<String, Value>, max_chars: usize) -> Result<Vec<SheetCriterion>, String> {
    let Some(done_when) = map.get("doneWhen") else {
        return Ok(Vec::new());
    };
    let Value::Array(items) = done_when else {
        return Err("doneWhen must be an array".to_string());
    };

    let mut kept = Vec::new();
    for item in items {
        let text = item
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        if text.is_empty() {
            continue;
        }
        if text.contains('\n') || text.contains('\r') {
            return Err("a criterion must be one line".to_string());
        }
        if text.chars().count() > max_chars {
            return Err("a criterion is too long".to_string());
        }

        let check = item
            .get("check")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        let check = if check.is_empty() {
            None
        } else {
            if check.contains('\n')
                || check.contains('\r')
                || check.chars().count() > MAX_CHECK_CHARS
            {
                return Err("a check must be one line of at most 500 characters".to_string());
            }
            Some(check.to_string())
        };

        kept.push(SheetCriterion {
            text: text.to_string(),
            check,
        });
        if kept.len() > MAX_CRITERIA {
            return Err("at most 20 criteria".to_string());
        }
    }

    Ok(kept)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::criteria::parse_done_when;
    use serde_json::json;

    fn idea(state: IdeaState, sheet: Option<TaskSheet>) -> Idea {
        Idea {
            id: "i1".to_string(),
            text: "Export the costs as CSV. More detail.".to_string(),
            state,
            session_id: None,
            plan: None,
            created_at: 0,
            updated_at: 0,
            halted: None,
            started_at: None,
            autonomous: false,
            attachments: vec![],
            sheet,
            model_fields: Default::default(),
        }
    }

    #[test]
    fn parse_sheet_rejects_oversize_multiline_and_too_many() {
        let value = json!({ "objective": "a".repeat(4001) });
        assert!(parse_sheet(&value, 4000).is_err());

        let criteria: Vec<Value> = (0..21)
            .map(|i| json!({ "text": format!("c{i}") }))
            .collect();
        let value = json!({ "doneWhen": criteria });
        assert!(parse_sheet(&value, 4000).is_err());

        let value = json!({ "doneWhen": [{ "text": "c", "check": "a\nb" }] });
        assert!(parse_sheet(&value, 4000).is_err());

        let value = json!({ "doneWhen": [{ "text": "a\nb" }] });
        assert!(parse_sheet(&value, 4000).is_err());
    }

    #[test]
    fn parse_sheet_rejects_a_plan_heading_in_a_field() {
        let value = json!({ "context": "intro\n## Done when\n- x" });
        assert!(parse_sheet(&value, 4000).is_err());

        let value = json!({ "context": "### Detail" });
        assert!(parse_sheet(&value, 4000).is_ok());
    }

    #[test]
    fn parse_sheet_trims_and_skips_blank_criteria() {
        let value = json!({
            "objective": "  goal  ",
            "doneWhen": [
                { "text": "   " },
                { "text": "  do it  ", "check": "  " }
            ]
        });
        let sheet = parse_sheet(&value, 4000).unwrap();
        assert_eq!(sheet.objective, "goal");
        assert_eq!(sheet.done_when.len(), 1);
        assert_eq!(sheet.done_when[0].text, "do it");
        assert_eq!(sheet.done_when[0].check, None);
    }

    #[test]
    fn ready_without_objective_is_400_with_missing() {
        let sheet = TaskSheet {
            objective: String::new(),
            done_when: vec![SheetCriterion {
                text: "done".to_string(),
                check: None,
            }],
            ..Default::default()
        };
        let idea = idea(IdeaState::Framing, Some(sheet));
        let result = sheet_event(&idea, "ready", &json!({}), 4000, None).unwrap();
        let Err(refusal) = result else {
            panic!("expected refusal");
        };
        assert_eq!(refusal.status, 400);
        assert_eq!(refusal.missing, vec![SheetField::Objective]);
    }

    #[test]
    fn ready_with_a_full_sheet_emits_a_plan_read_back_by_the_readers() {
        let sheet = TaskSheet {
            objective: "Export costs".to_string(),
            context: "CSV".to_string(),
            done_when: vec![
                SheetCriterion {
                    text: "file exists".to_string(),
                    check: Some("cargo test".to_string()),
                },
                SheetCriterion {
                    text: "columns correct".to_string(),
                    check: None,
                },
            ],
            ..Default::default()
        };
        let idea = idea(IdeaState::Framing, Some(sheet.clone()));
        let result = sheet_event(&idea, "ready", &json!({}), 4000, None).unwrap();
        let Ok(BacklogEvent::Ready { plan, .. }) = result else {
            panic!("expected ready event");
        };

        let criteria = parse_done_when(&plan);
        assert_eq!(criteria.len(), 2);
        assert_eq!(criteria[0].description, "file exists");
        assert_eq!(criteria[0].check, Some("cargo test".to_string()));
        assert_eq!(criteria[1].description, "columns correct");
        assert_eq!(criteria[1].check, None);

        assert_eq!(
            backlog::title(&idea.text, Some(&plan)),
            backlog::title(&idea.text, None)
        );
    }

    #[test]
    fn sheet_on_a_started_idea_is_409() {
        let started = idea(IdeaState::Started, None);
        let result = sheet_event(&started, "sheet", &json!({ "sheet": {} }), 4000, None).unwrap();
        assert!(matches!(result, Err(Refusal { status: 409, .. })));

        let dropped = idea(IdeaState::Dropped, None);
        let result = sheet_event(&dropped, "turn-into-task", &json!({}), 4000, None).unwrap();
        assert!(matches!(result, Err(Refusal { status: 409, .. })));
    }

    #[test]
    fn turn_into_task_twice_is_409() {
        let idea = idea(IdeaState::Framing, Some(TaskSheet::default()));
        let result = sheet_event(&idea, "turn-into-task", &json!({}), 4000, None).unwrap();
        assert!(matches!(result, Err(Refusal { status: 409, .. })));
    }

    #[test]
    fn turn_into_task_copies_the_text_as_objective() {
        let idea = idea(IdeaState::Framing, None);
        let result = sheet_event(&idea, "turn-into-task", &json!({}), 4000, None).unwrap();
        let Ok(BacklogEvent::SheetEdited { sheet, by, .. }) = result else {
            panic!("expected sheet edited event");
        };
        assert_eq!(sheet.objective, idea.text);
        assert_eq!(by, Author::Human);
    }

    #[test]
    fn ready_without_a_sheet_is_left_to_the_caller() {
        let idea = idea(IdeaState::Framing, None);
        assert!(sheet_event(&idea, "ready", &json!({}), 4000, None).is_none());
    }

    #[test]
    fn apply_without_a_pending_plan_is_409() {
        let idea = idea(IdeaState::Framing, Some(TaskSheet::default()));
        let result = sheet_event(&idea, "sheet-apply", &json!({}), 4000, None).unwrap();
        assert!(matches!(result, Err(Refusal { status: 409, .. })));
    }

    #[test]
    fn apply_saves_the_plan_as_an_agent_edit() {
        let plan = Plan {
            title: "t".to_string(),
            goal: "Export costs".to_string(),
            context: String::new(),
            constraints: String::new(),
            out_of_scope: String::new(),
            deliverable: String::new(),
            steps: vec![],
            done_when: vec![],
            risks: vec![],
        };
        let idea = idea(IdeaState::Framing, Some(TaskSheet::default()));
        let result = sheet_event(&idea, "sheet-apply", &json!({}), 4000, Some(&plan)).unwrap();
        let Ok(BacklogEvent::SheetEdited { sheet, by, .. }) = result else {
            panic!("expected sheet edited event");
        };
        assert_eq!(sheet.objective, plan.goal);
        assert_eq!(by, Author::Agent);
    }

    #[test]
    fn sheet_by_agent_is_recorded() {
        let idea = idea(IdeaState::Framing, None);
        let body = json!({ "sheet": { "objective": "goal" }, "by": "agent" });
        let result = sheet_event(&idea, "sheet", &body, 4000, None).unwrap();
        let Ok(BacklogEvent::SheetEdited { by, .. }) = result else {
            panic!("expected sheet edited event");
        };
        assert_eq!(by, Author::Agent);
    }
}
