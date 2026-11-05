//! The small argument validators the loop's own tools share (`propose_plan`,
//! `submit_report`): a required string, a required array, a single line, an
//! array of strings. The errors are results the model reads, so they say
//! what to fix; `tool` is the tool's name, for the messages.

use serde_json::Value;

/// A required, non-empty string field; the error names the tool and field.
pub fn required(tool: &str, args: &Value, field: &str) -> Result<String, String> {
    args.get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("{tool} requires a non-empty '{field}' string."))
}

/// A required, non-empty array of objects; the error says what to add.
pub fn required_array<'a>(
    tool: &str,
    args: &'a Value,
    field: &str,
) -> Result<&'a Vec<Value>, String> {
    match args.get(field) {
        Some(Value::Array(items)) if !items.is_empty() => Ok(items),
        _ => Err(format!("{tool} requires a non-empty '{field}' array.")),
    }
}

/// A required, non-empty string field of one item; the error names the item.
pub fn required_in(value: &Value, field: &str, what: &str) -> Result<String, String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("{what} has no non-empty '{field}' string."))
}

/// Trims, keeps the value one line, and caps its length when `max` is set.
pub fn single_line(value: &str, label: &str, max_chars: usize) -> Result<String, String> {
    if value.chars().count() > max_chars && max_chars > 0 {
        return Err(format!("{label} must be at most {max_chars} characters."));
    }
    if value.contains('\n') {
        return Err(format!("{label} must be a single line: drop the newline."));
    }
    Ok(value.to_string())
}

/// An optional array of strings: every entry trimmed, empties dropped.
pub fn string_list(value: &Value, field: &str, what: &str) -> Result<Vec<String>, String> {
    match value.get(field) {
        None => Ok(Vec::new()),
        Some(Value::Array(items)) => {
            let mut out = Vec::new();
            for (index, item) in items.iter().enumerate() {
                let entry = item
                    .as_str()
                    .ok_or_else(|| format!("{what}'s {field}[{index}] is not a string."))?
                    .trim();
                if !entry.is_empty() {
                    out.push(entry.to_string());
                }
            }
            Ok(out)
        }
        Some(_) => Err(format!("{what}'s '{field}' must be an array of strings.")),
    }
}
