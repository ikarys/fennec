//! Reading a tool call's JSON arguments, with the errors the model sees.

use serde_json::Value;

pub(super) fn string<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    match args.get(key) {
        Some(Value::String(value)) => Ok(value),
        Some(_) => Err(format!("Argument {key} must be a string")),
        None => Err(format!("Missing required argument: {key}")),
    }
}

pub(super) fn boolean(args: &Value, key: &str) -> Result<bool, String> {
    match args.get(key) {
        Some(Value::Bool(value)) => Ok(*value),
        Some(_) => Err(format!("Argument {key} must be a boolean")),
        None => Ok(false),
    }
}

pub(super) fn number(args: &Value, key: &str, default: i64) -> Result<i64, String> {
    match args.get(key) {
        Some(Value::Number(value)) => value
            .as_i64()
            .ok_or_else(|| format!("Argument {key} must be an integer")),
        Some(_) => Err(format!("Argument {key} must be a number")),
        None => Ok(default),
    }
}
