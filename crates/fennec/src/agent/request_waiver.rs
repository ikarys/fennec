//! The `request_waiver` tool, owned by the loop: it pauses the turn to ask
//! the human to waive the protection of one file. Nothing waits in memory:
//! the turn ends with `waiting_for_user` and the answer arrives through the
//! waiver card, so a pending request survives a restart.

use serde_json::{Value, json};

pub const NAME: &str = "request_waiver";

/// The OpenAI tool definition.
pub fn definition() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": NAME,
            "description": "Ask the human to waive the protection of one protected file so you can change it. The turn pauses until the human grants it for a limited time or refuses. Use it only when the approved task cannot be done without changing that file; give the reason the human will read.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The protected file, project-relative",
                    },
                    "reason": {
                        "type": "string",
                        "description": "Why the change is required",
                    },
                },
                "required": ["path", "reason"],
            },
        },
    })
}

/// Validates and normalises the model's `request_waiver` arguments. The error
/// is a result the model reads, so it must say what to fix.
pub fn parse(args: &Value) -> Result<(String, String), String> {
    let path = args
        .get("path")
        .and_then(Value::as_str)
        .and_then(clean_path)
        .ok_or_else(|| {
            "request_waiver requires 'path': the protected file, relative to the project root, without '..'."
                .to_string()
        })?;
    let reason = args
        .get("reason")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|reason| !reason.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            "request_waiver requires a non-empty 'reason' string. Explain why the change is required."
                .to_string()
        })?;
    Ok((path, reason))
}

/// Trims the path and strips one leading `./`. A waiver names one file of the
/// project, matched exactly: an absolute path, a `..` or a control character
/// (it ends up in the ADR the human reads) is refused.
fn clean_path(value: &str) -> Option<String> {
    let trimmed = value.trim();
    let stripped = trimmed.strip_prefix("./").unwrap_or(trimmed);
    if stripped.is_empty()
        || stripped.starts_with('/')
        || stripped.split('/').any(|part| part == "..")
        || stripped.chars().any(char::is_control)
    {
        None
    } else {
        Some(stripped.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_requires_non_empty_path_and_reason() {
        assert!(parse(&json!({})).is_err());
        assert!(parse(&json!({"path": ""})).is_err());
        assert!(parse(&json!({"path": "   "})).is_err());
        assert!(parse(&json!({"path": 42})).is_err());
        assert!(parse(&json!({"path": "a"})).is_err());
        assert!(parse(&json!({"path": "a", "reason": ""})).is_err());
        assert!(parse(&json!({"path": "a", "reason": "   "})).is_err());
        assert!(parse(&json!({"path": "a", "reason": 42})).is_err());
    }

    #[test]
    fn parse_trims_and_strips_a_leading_dot_slash() {
        let (path, reason) =
            parse(&json!({"path": "  ./AGENTS.md  ", "reason": "  why  "})).unwrap();
        assert_eq!(path, "AGENTS.md");
        assert_eq!(reason, "why");
    }

    #[test]
    fn parse_rejects_a_path_that_is_only_dot_slash() {
        assert!(parse(&json!({"path": "./", "reason": "why"})).is_err());
    }

    #[test]
    fn parse_rejects_paths_outside_one_project_file() {
        for path in ["/etc/passwd", "../x", "a/../../b", "a\nb"] {
            assert!(
                parse(&json!({"path": path, "reason": "r"})).is_err(),
                "{path}"
            );
        }
    }

    #[test]
    fn definition_matches_the_contract() {
        let d = definition();
        assert_eq!(d["function"]["name"], "request_waiver");
        assert_eq!(
            d["function"]["parameters"]["required"],
            json!(["path", "reason"])
        );
    }
}
