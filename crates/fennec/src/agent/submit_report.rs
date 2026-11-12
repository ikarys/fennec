//! The `submit_report` tool, owned by the loop and fennec's own: the builder
//! agent ends its turn with it, fennec validates and records the report, and
//! renders it in the builder's "Final report" format.

use crate::agent::tool_args;
use crate::domain::event::Event;
use crate::domain::report::{ChangedFile, Report, ReportCriterion, ReportStep};
use serde_json::{Value, json};

pub const NAME: &str = "submit_report";

/// What the model reads back once the report is in the log.
pub const RECORDED: &str = "Report recorded. The human reviews it; end your turn here.";

/// The OpenAI tool definition.
pub fn definition() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": NAME,
            "description": "Submit your final report: this is how a build ends. Once the work is done, call it with the report instead of writing the report as text, and end your turn right after. The human reviews it to decide what to review and test.",
            "parameters": {
                "type": "object",
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "The plan's title"
                    },
                    "summary": {
                        "type": "string",
                        "description": "One or two sentences: what changed for the user"
                    },
                    "findings": {
                        "type": "string",
                        "description": "What the work found or measured that the human asked to know: benchmark numbers, an investigation's answer, a test run's result. Markdown, several lines allowed. Fill it whenever the task asks for a result, even when no criterion asks for it; empty otherwise"
                    },
                    "steps": {
                        "type": "array",
                        "description": "The plan's steps; mark a step done only for what you verified",
                        "items": {
                            "type": "object",
                            "properties": {
                                "title": {
                                    "type": "string",
                                    "description": "The step's title, as in the plan"
                                },
                                "done": {
                                    "type": "boolean",
                                    "description": "True only for a step you verified"
                                },
                                "note": {
                                    "type": "string",
                                    "description": "What was done, in one line; why it is not done, when done is false"
                                }
                            },
                            "required": ["title", "done"]
                        }
                    },
                    "doneWhen": {
                        "type": "array",
                        "description": "The plan's \"Done when\" criteria; mark one met only for what you verified; empty when there is no plan",
                        "items": {
                            "type": "object",
                            "properties": {
                                "text": {
                                    "type": "string",
                                    "description": "The criterion, as in the plan"
                                },
                                "met": {
                                    "type": "boolean",
                                    "description": "True only for a criterion you verified"
                                },
                                "how": {
                                    "type": "string",
                                    "description": "How you verified it"
                                }
                            },
                            "required": ["text", "met"]
                        }
                    },
                    "changed": {
                        "type": "array",
                        "description": "Every file the work changed; empty when none",
                        "items": {
                            "type": "object",
                            "properties": {
                                "path": {
                                    "type": "string",
                                    "description": "The file, project-relative"
                                },
                                "what": {
                                    "type": "string",
                                    "description": "What changed in it"
                                }
                            },
                            "required": ["path", "what"]
                        }
                    },
                    "test": {
                        "type": "string",
                        "description": "What the human should try by hand; empty when there is nothing"
                    },
                    "todo": {
                        "type": "string",
                        "description": "What remains or was found on the way; empty when there is nothing"
                    }
                },
                "required": ["title", "summary", "steps"]
            }
        }
    })
}

/// Validates the model's `submit_report` arguments. The error is a result
/// the model reads, so it must say what to fix.
pub fn parse(args: &Value) -> Result<Report, String> {
    let title = tool_args::single_line(&tool_args::required(NAME, args, "title")?, "'title'", 120)?;
    let summary =
        tool_args::single_line(&tool_args::required(NAME, args, "summary")?, "'summary'", 0)?;
    let findings = optional_text(args, "findings", "the report")?;

    let steps = tool_args::required_array(NAME, args, "steps")?;
    let mut report_steps = Vec::with_capacity(steps.len());
    for (index, step) in steps.iter().enumerate() {
        let what = format!("step {}", index + 1);
        let step_title = tool_args::single_line(
            &tool_args::required_in(step, "title", &what)?,
            &format!("{what}'s 'title'"),
            0,
        )?;
        let done = required_bool(step, "done", &what)?;
        let note = optional_single_line(step, "note", &what)?;
        report_steps.push(ReportStep {
            title: step_title,
            done,
            note,
        });
    }

    let criteria = object_list(args, "doneWhen")?;
    let mut done_when = Vec::with_capacity(criteria.len());
    for (index, item) in criteria.iter().enumerate() {
        let what = format!("criterion {}", index + 1);
        let text = tool_args::single_line(
            &tool_args::required_in(item, "text", &what)?,
            &format!("{what}'s 'text'"),
            0,
        )?;
        let met = required_bool(item, "met", &what)?;
        let how = optional_single_line(item, "how", &what)?;
        done_when.push(ReportCriterion { text, met, how });
    }

    let files = object_list(args, "changed")?;
    let mut changed = Vec::with_capacity(files.len());
    for (index, item) in files.iter().enumerate() {
        let what = format!("changed file {}", index + 1);
        let path = tool_args::single_line(
            &tool_args::required_in(item, "path", &what)?,
            &format!("{what}'s 'path'"),
            0,
        )?;
        let change = tool_args::single_line(
            &tool_args::required_in(item, "what", &what)?,
            &format!("{what}'s 'what'"),
            0,
        )?;
        changed.push(ChangedFile { path, what: change });
    }

    let test = optional_single_line(args, "test", "the report")?.unwrap_or_default();
    let todo = optional_single_line(args, "todo", "the report")?.unwrap_or_default();
    Ok(Report {
        title,
        summary,
        findings,
        steps: report_steps,
        done_when,
        changed,
        test,
        todo,
    })
}

pub fn event(call_id: &str, report: Report) -> Event {
    Event::ReportSubmitted {
        call_id: call_id.into(),
        markdown: report.to_markdown(),
        report,
    }
}

/// A required boolean field; the error names the item and the field.
fn required_bool(value: &Value, field: &str, what: &str) -> Result<bool, String> {
    value
        .get(field)
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("{what} has no '{field}' boolean."))
}

/// An optional array of objects: absent is empty, present must be an array.
fn object_list<'a>(args: &'a Value, field: &str) -> Result<&'a [Value], String> {
    match args.get(field) {
        None => Ok(&[]),
        Some(Value::Array(items)) => Ok(items),
        Some(_) => Err(format!("{NAME}'s '{field}' must be an array of objects.")),
    }
}

/// An optional string field: trimmed, one line; None when absent or empty.
fn optional_single_line(value: &Value, field: &str, what: &str) -> Result<Option<String>, String> {
    match value.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => {
            let s = s.trim();
            if s.is_empty() {
                Ok(None)
            } else {
                tool_args::single_line(s, &format!("{what}'s '{field}'"), 0).map(Some)
            }
        }
        Some(_) => Err(format!("{what}'s '{field}' must be a string.")),
    }
}

/// An optional multi-line string field: trimmed; empty when absent or null.
fn optional_text(value: &Value, field: &str, what: &str) -> Result<String, String> {
    match value.get(field) {
        None | Some(Value::Null) => Ok(String::new()),
        Some(Value::String(s)) => Ok(s.trim().to_string()),
        Some(_) => Err(format!("{what}'s '{field}' must be a string.")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn valid() -> Value {
        json!({
            "title": "Add the report tool",
            "summary": "The builder ends its turn with a submit_report call",
            "steps": [
                {
                    "title": "Write the domain",
                    "done": true,
                    "note": "The report's vocabulary"
                },
                {
                    "title": "Wire the tool",
                    "done": false
                }
            ],
            "doneWhen": [
                {"text": "The build passes", "met": true, "how": "just check"},
                {"text": "The code stays clean", "met": false}
            ],
            "changed": [
                {"path": "src/domain/report.rs", "what": "The vocabulary"}
            ],
            "test": "Open the UI",
            "todo": ""
        })
    }

    #[test]
    fn parse_trims_every_string_and_keeps_the_booleans() {
        let r = parse(&json!({
            "title": " Title ",
            "summary": " Summary ",
            "steps": [
                {"title": " Write ", "done": true, "note": " Do it "},
                {"title": "Wire", "done": false, "note": "  "}
            ],
            "doneWhen": [
                {"text": " It works ", "met": true, "how": " cargo test "},
                {"text": "Clean", "met": false}
            ],
            "changed": [
                {"path": " src/a.rs ", "what": " Do it "}
            ],
            "test": " Try it ",
            "todo": " None "
        }))
        .unwrap();
        assert_eq!(r.title, "Title");
        assert_eq!(r.summary, "Summary");
        assert_eq!(r.steps[0].title, "Write");
        assert!(r.steps[0].done);
        assert_eq!(r.steps[0].note.as_deref(), Some("Do it"));
        assert!(!r.steps[1].done);
        assert_eq!(r.steps[1].note, None);
        assert_eq!(r.done_when[0].text, "It works");
        assert!(r.done_when[0].met);
        assert_eq!(r.done_when[0].how.as_deref(), Some("cargo test"));
        assert!(!r.done_when[1].met);
        assert_eq!(r.done_when[1].how, None);
        assert_eq!(r.changed[0].path, "src/a.rs");
        assert_eq!(r.changed[0].what, "Do it");
        assert_eq!(r.test, "Try it");
        assert_eq!(r.todo, "None");
    }

    #[test]
    fn parse_names_the_missing_field() {
        let mut args = valid();
        args.as_object_mut().unwrap().remove("title");
        assert!(parse(&args).err().unwrap().contains("'title'"));
        args = valid();
        args.as_object_mut().unwrap().remove("summary");
        assert!(parse(&args).err().unwrap().contains("'summary'"));
        args = valid();
        args.as_object_mut().unwrap().remove("steps");
        assert!(parse(&args).err().unwrap().contains("'steps'"));
        assert!(
            parse(&json!({"summary": "s", "steps": [{}]}))
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
        args["steps"][0].as_object_mut().unwrap().remove("done");
        let err = parse(&args).err().unwrap();
        assert!(err.contains("step 1"));
        assert!(err.contains("'done'"));
    }

    #[test]
    fn done_when_and_changed_may_be_empty() {
        let mut args = valid();
        args["doneWhen"] = json!([]);
        args["changed"] = json!([]);
        let r = parse(&args).unwrap();
        assert!(r.done_when.is_empty());
        assert!(r.changed.is_empty());
        args = valid();
        args.as_object_mut().unwrap().remove("doneWhen");
        args.as_object_mut().unwrap().remove("changed");
        let r = parse(&args).unwrap();
        assert!(r.done_when.is_empty());
        assert!(r.changed.is_empty());
    }

    #[test]
    fn parse_names_the_criterion_field_that_is_missing() {
        let mut args = valid();
        args["doneWhen"][0].as_object_mut().unwrap().remove("met");
        let err = parse(&args).err().unwrap();
        assert!(err.contains("criterion 1"));
        assert!(err.contains("'met'"));
        args = valid();
        args["doneWhen"][0].as_object_mut().unwrap().remove("text");
        let err = parse(&args).err().unwrap();
        assert!(err.contains("criterion 1"));
        assert!(err.contains("'text'"));
    }

    #[test]
    fn parse_names_the_changed_file_field_that_is_missing() {
        let mut args = valid();
        args["changed"][0].as_object_mut().unwrap().remove("path");
        let err = parse(&args).err().unwrap();
        assert!(err.contains("changed file 1"));
        assert!(err.contains("'path'"));
        args = valid();
        args["changed"][0].as_object_mut().unwrap().remove("what");
        let err = parse(&args).err().unwrap();
        assert!(err.contains("changed file 1"));
        assert!(err.contains("'what'"));
    }

    #[test]
    fn parse_rejects_a_newline_in_any_text() {
        let mut args = valid();
        args["title"] = json!("a\nb");
        assert_newline_error(parse(&args).err().unwrap(), "'title'");

        args = valid();
        args["summary"] = json!("a\nb");
        assert_newline_error(parse(&args).err().unwrap(), "'summary'");

        args = valid();
        args["steps"][0]["title"] = json!("a\nb");
        assert_newline_error(parse(&args).err().unwrap(), "step 1's 'title'");

        args = valid();
        args["steps"][0]["note"] = json!("a\nb");
        assert_newline_error(parse(&args).err().unwrap(), "step 1's 'note'");

        args = valid();
        args["doneWhen"][0]["text"] = json!("a\nb");
        assert_newline_error(parse(&args).err().unwrap(), "criterion 1's 'text'");

        args = valid();
        args["doneWhen"][0]["how"] = json!("a\nb");
        assert_newline_error(parse(&args).err().unwrap(), "criterion 1's 'how'");

        args = valid();
        args["changed"][0]["what"] = json!("a\nb");
        assert_newline_error(parse(&args).err().unwrap(), "changed file 1's 'what'");

        args = valid();
        args["test"] = json!("a\nb");
        assert_newline_error(parse(&args).err().unwrap(), "the report's 'test'");
    }

    fn assert_newline_error(err: String, label: &str) {
        assert!(err.contains(label), "{label}: {err}");
        assert!(err.contains("newline"), "{label}: {err}");
    }

    #[test]
    fn a_non_boolean_done_is_named() {
        let mut args = valid();
        args["steps"][0]["done"] = json!("yes");
        let err = parse(&args).err().unwrap();
        assert!(err.contains("step 1"));
        assert!(err.contains("'done'"));
    }

    #[test]
    fn test_and_todo_are_optional() {
        let mut args = valid();
        args.as_object_mut().unwrap().remove("test");
        args.as_object_mut().unwrap().remove("todo");
        let r = parse(&args).unwrap();
        assert_eq!(r.test, "");
        assert_eq!(r.todo, "");
    }

    #[test]
    fn findings_are_optional_and_multi_line() {
        let mut args = valid();
        args["findings"] = json!("  p50 12 ms\n p99 40 ms  ");
        let r = parse(&args).unwrap();
        assert_eq!(r.findings, "p50 12 ms\n p99 40 ms");
        args = valid();
        args.as_object_mut().unwrap().remove("findings");
        let r = parse(&args).unwrap();
        assert_eq!(r.findings, "");
    }
}
