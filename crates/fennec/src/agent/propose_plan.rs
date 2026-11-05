//! The `propose_plan` tool, owned by the loop and fennec's own: the plan
//! agent ends its turn with it, fennec validates and records the plan, and
//! renders it as the plan's canonical markdown.

use crate::agent::tool_args;
use crate::domain::event::Event;
use crate::domain::plan::{Plan, PlanStep, PlannedCriterion};
use serde_json::{Value, json};

pub const NAME: &str = "propose_plan";

/// What the model reads back once the plan is in the log.
pub const RECORDED: &str = "Plan recorded. The human reviews it; end your turn here.";

/// The OpenAI tool definition.
pub fn definition() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": NAME,
            "description": "Propose your plan: this is how a plan ends. Once the goal is clear, call it with the complete plan instead of writing the plan as text, and end your turn right after. The human reviews it before anything is built.",
            "parameters": {
                "type": "object",
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "Short title of the plan"
                    },
                    "goal": {
                        "type": "string",
                        "description": "One sentence: what is true once the plan is done"
                    },
                    "steps": {
                        "type": "array",
                        "description": "The steps, one short self-contained piece each; a small task needs one or two, not more",
                        "items": {
                            "type": "object",
                            "properties": {
                                "title": {
                                    "type": "string",
                                    "description": "Imperative title of the step"
                                },
                                "files": {
                                    "type": "array",
                                    "items": { "type": "string" },
                                    "description": "The real files the step touches, read from the codebase (new ones marked \"(new)\"); empty when the step changes no file"
                                },
                                "change": {
                                    "type": "string",
                                    "description": "What to change and why, in one or two lines"
                                }
                            },
                            "required": ["title", "change"]
                        }
                    },
                    "doneWhen": {
                        "type": "array",
                        "description": "The \"Done when\" criteria: the contract the human approves",
                        "items": {
                            "type": "object",
                            "properties": {
                                "text": {
                                    "type": "string",
                                    "description": "One short line, in the human's words: what will be true for them, not a technical specification"
                                },
                                "check": {
                                    "type": "string",
                                    "description": "A read-only command that exits 0 once the criterion holds; it must hold for any correct implementation, and run in the sandbox with no network. Omit it for a criterion only a reader can judge"
                                }
                            },
                            "required": ["text"]
                        }
                    },
                    "risks": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Risks and open questions; empty when there are none"
                    }
                },
                "required": ["title", "goal", "steps", "doneWhen"]
            }
        }
    })
}

/// Validates the model's `propose_plan` arguments. The error is a result
/// the model reads, so it must say what to fix.
pub fn parse(args: &Value) -> Result<Plan, String> {
    let title = tool_args::single_line(&tool_args::required(NAME, args, "title")?, "'title'", 120)?;
    let goal = tool_args::single_line(&tool_args::required(NAME, args, "goal")?, "'goal'", 0)?;

    let steps = tool_args::required_array(NAME, args, "steps")?;
    let mut plan_steps = Vec::with_capacity(steps.len());
    for (index, step) in steps.iter().enumerate() {
        let what = format!("step {}", index + 1);
        let step_title = tool_args::single_line(
            &tool_args::required_in(step, "title", &what)?,
            &format!("{what}'s 'title'"),
            0,
        )?;
        let change = tool_args::single_line(
            &tool_args::required_in(step, "change", &what)?,
            &format!("{what}'s 'change'"),
            0,
        )?;
        let files = tool_args::string_list(step, "files", &what)?;
        plan_steps.push(PlanStep {
            title: step_title,
            files,
            change,
        });
    }

    let criteria = tool_args::required_array(NAME, args, "doneWhen")?;
    let mut done_when = Vec::with_capacity(criteria.len());
    for (index, item) in criteria.iter().enumerate() {
        let what = format!("criterion {}", index + 1);
        let text = tool_args::single_line(
            &tool_args::required_in(item, "text", &what)?,
            &format!("{what}'s 'text'"),
            0,
        )?;
        let check = match item.get("check") {
            // An empty check string means no check.
            None | Some(Value::Null) => None,
            Some(Value::String(s)) if s.trim().is_empty() => None,
            Some(Value::String(s)) => {
                let check = tool_args::single_line(s.trim(), &format!("{what}'s 'check'"), 0)?;
                if check.contains('`') {
                    return Err(format!(
                        "{what}'s 'check' must not contain a backtick: it would break the rendered markdown."
                    ));
                }
                Some(check)
            }
            Some(_) => {
                return Err(format!("{what}'s 'check' must be a string."));
            }
        };
        done_when.push(PlannedCriterion { text, check });
    }

    let risks = tool_args::string_list(args, "risks", "the plan")?;
    Ok(Plan {
        title,
        goal,
        steps: plan_steps,
        done_when,
        risks,
    })
}

pub fn event(call_id: &str, plan: Plan) -> Event {
    Event::PlanProposed {
        call_id: call_id.into(),
        markdown: plan.to_markdown(),
        plan,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn valid() -> Value {
        json!({
            "title": "Add the plan tool",
            "goal": "The plan agent ends its turn with a propose_plan call",
            "steps": [
                {
                    "title": "Write the domain",
                    "files": ["src/domain/plan.rs"],
                    "change": "The plan's vocabulary"
                }
            ],
            "doneWhen": [
                {"text": "The build passes", "check": "cargo build"},
                {"text": "The code stays clean"}
            ],
            "risks": ["None"]
        })
    }

    #[test]
    fn parse_trims_every_string() {
        let p = parse(&json!({
            "title": " Title ",
            "goal": " Goal ",
            "steps": [
                {
                    "title": " Write ",
                    "files": [" src/a.rs ", " "],
                    "change": " Do it "
                }
            ],
            "doneWhen": [
                {"text": " It works ", "check": " cargo test "}
            ],
            "risks": [" Risk ", " "]
        }))
        .unwrap();
        assert_eq!(p.title, "Title");
        assert_eq!(p.goal, "Goal");
        assert_eq!(p.steps[0].title, "Write");
        assert_eq!(p.steps[0].files, vec!["src/a.rs".to_string()]);
        assert_eq!(p.steps[0].change, "Do it");
        assert_eq!(p.done_when[0].text, "It works");
        assert_eq!(p.done_when[0].check.as_deref(), Some("cargo test"));
        assert_eq!(p.risks, vec!["Risk".to_string()]);
    }

    #[test]
    fn parse_names_the_missing_field() {
        let mut args = valid();
        args.as_object_mut().unwrap().remove("title");
        assert!(parse(&args).err().unwrap().contains("'title'"));
        args = valid();
        args.as_object_mut().unwrap().remove("goal");
        assert!(parse(&args).err().unwrap().contains("'goal'"));
        args = valid();
        args.as_object_mut().unwrap().remove("steps");
        assert!(parse(&args).err().unwrap().contains("'steps'"));
        args = valid();
        args.as_object_mut().unwrap().remove("doneWhen");
        assert!(parse(&args).err().unwrap().contains("'doneWhen'"));
        assert!(
            parse(&json!({"title": " ", "goal": "g", "steps": [{}], "doneWhen": [{}]}))
                .err()
                .unwrap()
                .contains("'title'")
        );
    }

    #[test]
    fn parse_rejects_a_title_over_120_chars() {
        let mut args = valid();
        args["title"] = json!("a".repeat(121));
        assert!(parse(&args).err().unwrap().contains("120"));
        args["title"] = json!("a".repeat(120));
        assert!(parse(&args).is_ok());
    }

    #[test]
    fn parse_rejects_an_empty_step_list() {
        let mut args = valid();
        args["steps"] = json!([]);
        assert!(parse(&args).err().unwrap().contains("'steps'"));
    }

    #[test]
    fn parse_names_the_step_field_that_is_missing() {
        let mut args = valid();
        args["steps"][0].as_object_mut().unwrap().remove("title");
        let err = parse(&args).err().unwrap();
        assert!(err.contains("step 1"));
        assert!(err.contains("'title'"));
        args = valid();
        args["steps"][0].as_object_mut().unwrap().remove("change");
        let err = parse(&args).err().unwrap();
        assert!(err.contains("step 1"));
        assert!(err.contains("'change'"));
    }

    #[test]
    fn parse_requires_at_least_one_criterion() {
        let mut args = valid();
        args["doneWhen"] = json!([]);
        assert!(parse(&args).err().unwrap().contains("'doneWhen'"));
        args = valid();
        args["doneWhen"] = json!([{"check": "true"}]);
        let err = parse(&args).err().unwrap();
        assert!(err.contains("criterion 1"));
        assert!(err.contains("'text'"));
    }

    #[test]
    fn parse_rejects_a_newline_in_any_text() {
        let mut args = valid();
        args["title"] = json!("a\nb");
        assert_newline_error(parse(&args).err().unwrap(), "'title'");

        args = valid();
        args["goal"] = json!("a\nb");
        assert_newline_error(parse(&args).err().unwrap(), "'goal'");

        args = valid();
        args["steps"][0]["title"] = json!("a\nb");
        assert_newline_error(parse(&args).err().unwrap(), "step 1's 'title'");

        args = valid();
        args["steps"][0]["change"] = json!("a\nb");
        assert_newline_error(parse(&args).err().unwrap(), "step 1's 'change'");

        args = valid();
        args["doneWhen"][0]["text"] = json!("a\nb");
        assert_newline_error(parse(&args).err().unwrap(), "criterion 1's 'text'");

        args = valid();
        args["doneWhen"][0]["check"] = json!("a\nb");
        assert_newline_error(parse(&args).err().unwrap(), "criterion 1's 'check'");
    }

    fn assert_newline_error(err: String, label: &str) {
        assert!(err.contains(label), "{label}: {err}");
        assert!(err.contains("newline"), "{label}: {err}");
    }

    #[test]
    fn parse_rejects_a_backtick_in_the_check() {
        let mut args = valid();
        args["doneWhen"][0]["check"] = json!("cargo `build`");
        let err = parse(&args).err().unwrap();
        assert!(err.contains("backtick"));
        assert!(err.contains("criterion 1"));
    }

    #[test]
    fn an_empty_check_means_no_check() {
        let mut args = valid();
        args["doneWhen"][0]["check"] = json!("  ");
        let p = parse(&args).unwrap();
        assert_eq!(p.done_when[0].check, None);
        args = valid();
        args["doneWhen"][0].as_object_mut().unwrap().remove("check");
        let p = parse(&args).unwrap();
        assert_eq!(p.done_when[0].check, None);
    }

    #[test]
    fn risks_are_optional_and_trimmed() {
        let mut args = valid();
        args.as_object_mut().unwrap().remove("risks");
        let p = parse(&args).unwrap();
        assert!(p.risks.is_empty());
    }

    #[test]
    fn a_non_string_files_entry_is_named() {
        let mut args = valid();
        args["steps"][0]["files"] = json!([42]);
        let err = parse(&args).err().unwrap();
        assert!(err.contains("step 1"));
        assert!(err.contains("files[0]"));
    }
}
