//! The agent loop. Every tool runs inside the world's sandbox: the harness
//! never touches world files itself, so a symlink cannot lead it outside.
//! There is no permission list: whatever the agent does stays in its world.

use std::io::{self, Write};
use std::process::Stdio;

use serde_json::{Value, json};

use crate::provider::Model;
use crate::world::{World, Worlds};

const OUTPUT_LIMIT: usize = 8_000;
const COMMAND_TIMEOUT_SECS: u32 = 300;
/// Bytes of conversation past which old tool output is elided.
const CONTEXT_BUDGET: usize = 48_000;
/// The latest messages are never elided: the model is acting on them.
const KEEP_RECENT: usize = 6;

pub struct Outcome {
    /// The agent's own account; never used as a verdict.
    pub summary: Option<String>,
    pub steps: usize,
    pub messages: Vec<Value>,
}

pub fn run(
    worlds: &Worlds,
    world: &World,
    model: &dyn Model,
    goal: &str,
    criteria: &[String],
    context: &str,
    max_steps: usize,
) -> io::Result<Outcome> {
    let tools = tools();
    let mut system = format!(
        "You work in a sandboxed copy of a project; the current directory is its root.\n\
         There is no network. You cannot commit: the harness checkpoints your work.\n\
         Anything you break can be rolled back, so act directly instead of asking.\n\
         When you are finished, call `done`. Your work is then judged only by these \
         criteria:\n{}",
        criteria
            .iter()
            .map(|c| format!("  - {c}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    system.push_str(context);
    let mut messages = vec![
        json!({"role": "system", "content": system}),
        json!({"role": "user", "content": goal}),
    ];
    for step in 1..=max_steps {
        bound(&mut messages);
        let mut reply = model.chat(&messages, &tools)?;
        // Reasoning is the model's scratch space; sending it back costs prefill.
        if let Some(obj) = reply.as_object_mut() {
            obj.remove("reasoning_content");
            obj.remove("reasoning");
        }
        reply["role"] = json!("assistant");
        let calls = reply["tool_calls"].as_array().cloned().unwrap_or_default();
        messages.push(reply);
        if calls.is_empty() {
            messages.push(json!({"role": "user", "content": "Use a tool, or call `done`."}));
            continue;
        }
        for call in calls {
            let name = call["function"]["name"].as_str().unwrap_or("");
            let args: Value = call["function"]["arguments"]
                .as_str()
                .and_then(|a| serde_json::from_str(a).ok())
                .unwrap_or(Value::Null);
            if name == "done" {
                return Ok(Outcome {
                    summary: args["summary"].as_str().map(str::to_string),
                    steps: step,
                    messages,
                });
            }
            let result = match execute(worlds, world, name, &args) {
                Ok(out) => out,
                Err(e) => format!("error: {e}"),
            };
            messages.push(json!({
                "role": "tool",
                "tool_call_id": call["id"],
                "content": truncate(&result),
            }));
        }
    }
    Ok(Outcome {
        summary: None,
        steps: max_steps,
        messages,
    })
}

fn execute(worlds: &Worlds, world: &World, name: &str, args: &Value) -> io::Result<String> {
    let arg = |k: &str| {
        args[k]
            .as_str()
            .ok_or_else(|| io::Error::other(format!("missing string argument `{k}`")))
    };
    match name {
        "shell" => {
            let argv = [
                "timeout".to_string(),
                "-k".to_string(),
                "5".to_string(),
                COMMAND_TIMEOUT_SECS.to_string(),
                "sh".to_string(),
                "-c".to_string(),
                arg("command")?.to_string(),
            ];
            let out = worlds
                .sandbox_command(world, &argv)?
                .stdin(Stdio::null())
                .output()?;
            Ok(format!(
                "exit {}\n{}{}",
                out.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ))
        }
        "write_file" => {
            let argv = [
                "sh".to_string(),
                "-c".to_string(),
                r#"mkdir -p "$(dirname "$1")" && cat > "$1""#.to_string(),
                "sh".to_string(),
                arg("path")?.to_string(),
            ];
            let content = arg("content")?;
            let mut child = worlds
                .sandbox_command(world, &argv)?
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()?;
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(content.as_bytes())?;
            }
            let out = child.wait_with_output()?;
            if out.status.success() {
                Ok(format!("wrote {} bytes", content.len()))
            } else {
                Err(io::Error::other(
                    String::from_utf8_lossy(&out.stderr).trim().to_string(),
                ))
            }
        }
        _ => Err(io::Error::other(format!("unknown tool `{name}`"))),
    }
}

/// Keeps the conversation under budget by eliding old tool output and old
/// file contents, oldest first. It cuts down to half the budget at once: each
/// cut changes the prefix and costs a fresh prefill, so cuts must be rare.
fn bound(messages: &mut [Value]) {
    let size = |m: &Value| m.to_string().len();
    let mut total: usize = messages.iter().map(size).sum();
    let end = messages.len().saturating_sub(KEEP_RECENT);
    if total <= CONTEXT_BUDGET || end <= 2 {
        return;
    }
    // The system prompt and the goal (0, 1) are never elided.
    for m in &mut messages[2..end] {
        if total <= CONTEXT_BUDGET / 2 {
            break;
        }
        let before = size(m);
        elide(m);
        total = total + size(m) - before;
    }
}

fn elide(m: &mut Value) {
    if m["role"] == "tool"
        && let Some(c) = m["content"].as_str()
        && c.len() > 200
    {
        let first = c.lines().next().unwrap_or("");
        m["content"] = json!(format!(
            "{first}\n[{} bytes elided to bound the context; run again if needed]",
            c.len()
        ));
    }
    for call in m["tool_calls"].as_array_mut().into_iter().flatten() {
        let Some(mut args) = call["function"]["arguments"]
            .as_str()
            .and_then(|a| serde_json::from_str::<Value>(a).ok())
        else {
            continue;
        };
        if let Some(c) = args["content"].as_str()
            && c.len() > 200
        {
            args["content"] = json!(format!("[{} bytes elided]", c.len()));
            call["function"]["arguments"] = json!(args.to_string());
        }
    }
}

/// Keep the head and the tail: errors usually sit at the end.
fn truncate(s: &str) -> String {
    if s.len() <= OUTPUT_LIMIT {
        return s.to_string();
    }
    let half = OUTPUT_LIMIT / 2;
    let head_end = (0..=half)
        .rev()
        .find(|&i| s.is_char_boundary(i))
        .unwrap_or(0);
    let tail_start = (s.len() - half..s.len())
        .find(|&i| s.is_char_boundary(i))
        .unwrap_or(s.len());
    format!(
        "{}\n[... {} bytes cut ...]\n{}",
        &s[..head_end],
        tail_start - head_end,
        &s[tail_start..]
    )
}

fn tools() -> Value {
    let tool = |name: &str, description: &str, params: Value| {
        json!({"type": "function", "function": {
            "name": name, "description": description,
            "parameters": {"type": "object", "properties": params,
                           "required": params.as_object().map(|p| p.keys().cloned().collect::<Vec<_>>())}
        }})
    };
    let string = |d: &str| json!({"type": "string", "description": d});
    json!([
        tool(
            "shell",
            "Run a shell command in the project root. Returns exit code and output.",
            json!({"command": string("sh -c script")})
        ),
        tool(
            "write_file",
            "Create or overwrite a file with the given content.",
            json!({"path": string("relative to the project root"), "content": string("full file content")})
        ),
        tool(
            "done",
            "Finish the task.",
            json!({"summary": string("what you changed and why")})
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bound_elides_old_output_and_keeps_the_frame() {
        let big = "exit 0\n".to_string() + &"x".repeat(10_000);
        let mut messages = vec![
            json!({"role": "system", "content": big}),
            json!({"role": "user", "content": "goal"}),
        ];
        for i in 0..10 {
            let args = json!({"path": "f", "content": "y".repeat(5_000)}).to_string();
            messages.push(json!({"role": "assistant", "tool_calls": [{"id": i, "function": {"name": "write_file", "arguments": args}}]}));
            messages.push(json!({"role": "tool", "content": big}));
        }
        let size = |m: &[Value]| m.iter().map(|m| m.to_string().len()).sum::<usize>();
        let before = size(&messages);
        bound(&mut messages);
        assert!(size(&messages) < before / 2);
        assert_eq!(messages[0]["content"].as_str().unwrap().len(), big.len());
        assert!(
            messages[3]["content"]
                .as_str()
                .unwrap()
                .starts_with("exit 0\n[")
        );
        let last = messages.last().unwrap();
        assert_eq!(last["content"].as_str().unwrap().len(), big.len());
        let args: Value = serde_json::from_str(
            messages[2]["tool_calls"][0]["function"]["arguments"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(args["path"], "f");
    }
}
