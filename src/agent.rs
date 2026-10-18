//! The agent loop. Every tool runs inside the world's sandbox: the harness
//! never touches world files itself, so a symlink cannot lead it outside.
//! There is no permission list: whatever the agent does stays in its world.

use std::io::{self, Write};
use std::process::Stdio;
use std::time::Instant;

use serde_json::{Value, json};

use crate::provider::Model;
use crate::world::{World, Worlds};

const OUTPUT_LIMIT: usize = 8_000;
const COMMAND_TIMEOUT_SECS: u32 = 300;
/// Bytes of conversation past which old tool output is elided.
const CONTEXT_BUDGET: usize = 48_000;
/// The latest messages are never elided: the model is acting on them.
const KEEP_RECENT: usize = 6;
/// The same call with the same result this many times in a row is a loop:
/// the model is told once, then stopped.
const REPEAT_NUDGE: usize = 3;
pub(crate) const REPEAT_STOP: usize = 5;
/// Steps that change no file in the world: told once, then stopped.
const IDLE_NUDGE: usize = 15;
const IDLE_STOP: usize = 25;
/// A command killed by its timeout costs minutes; a second one ends the run.
const TIMEOUT_STOP: usize = 2;

pub struct Outcome {
    /// The agent's own account; never used as a verdict.
    pub summary: Option<String>,
    pub steps: usize,
    pub messages: Vec<Value>,
    /// Why the harness stopped the agent before it called `done`.
    pub stuck: Option<String>,
    /// When stopped: the one question whose answer, in the model's view,
    /// would unblock it. `None` when it has none.
    pub question: Option<String>,
    /// One entry per turn, for profiling: when it started, how long the
    /// model took and what the server says it cost, how long each tool took.
    pub turns: Vec<Value>,
    /// Why the loop ended early (the model failed or was stopped). The turns
    /// before it are kept: a stopped attempt shows where its time went too.
    pub error: Option<String>,
}

/// Watches the loop for the three ways a run gets stuck. Pure bookkeeping:
/// each method says whether to nudge (`Some(false)`) or stop (`Some(true)`).
#[derive(Default)]
pub(crate) struct Watch {
    last_call: String,
    repeats: usize,
    files: String,
    idle: usize,
    timeouts: usize,
    nudged_repeat: bool,
    nudged_idle: bool,
}

impl Watch {
    pub(crate) fn call(&mut self, call: &str, result: &str) -> Option<bool> {
        let key = format!("{call}\n{result}");
        if key == self.last_call {
            self.repeats += 1;
        } else {
            (self.last_call, self.repeats) = (key, 1);
        }
        if self.repeats >= REPEAT_STOP {
            Some(true)
        } else if self.repeats >= REPEAT_NUDGE && !self.nudged_repeat {
            self.nudged_repeat = true;
            Some(false)
        } else {
            None
        }
    }

    fn step(&mut self, files: String) -> Option<bool> {
        if files == self.files {
            self.idle += 1;
        } else {
            (self.files, self.idle, self.nudged_idle) = (files, 0, false);
        }
        if self.idle >= IDLE_STOP {
            Some(true)
        } else if self.idle >= IDLE_NUDGE && !self.nudged_idle {
            self.nudged_idle = true;
            Some(false)
        } else {
            None
        }
    }
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
    let start = Instant::now();
    let mut turns = Vec::new();
    // EXPERIMENT: FENNEC_OS_THINK=always|adaptive|never, to be measured and
    // removed.
    let policy = std::env::var("FENNEC_OS_THINK").unwrap_or_else(|_| "always".to_string());
    let mut think = true;
    // A reply with nothing in it: the model spent its whole budget reasoning.
    // Asked again the same way it does it again (seen: four times in six),
    // so the next turn goes without reasoning, whatever the policy.
    let mut blank = false;
    let ms = |since: Instant| since.elapsed().as_millis() as u64;
    // The starting state counts as "no change yet".
    let mut watch = Watch {
        files: worlds.git(world, &["status", "--porcelain"])?,
        ..Watch::default()
    };
    let stop = |messages: Vec<Value>, turns: Vec<Value>, step, why: String| {
        Ok(Outcome {
            summary: None,
            steps: step,
            question: ask(model, goal, &messages, &why),
            messages,
            turns,
            stuck: Some(why),
            error: None,
        })
    };
    for step in 1..=max_steps {
        bound(&mut messages);
        let at = start.elapsed().as_millis() as u64;
        let asked = Instant::now();
        let think_now = !blank
            && match policy.as_str() {
                "never" => false,
                "adaptive" => think,
                _ => true,
            };
        let chat = if think_now {
            model.chat(&messages, &tools)
        } else {
            model.chat_quick(&messages, &tools)
        };
        let mut turn =
            json!({"step": step, "at_ms": at, "model_ms": ms(asked), "think": think_now});
        let mut reply = match chat {
            Ok(reply) => reply,
            Err(e) => {
                turns.push(turn);
                return Ok(Outcome {
                    summary: None,
                    steps: step,
                    messages,
                    turns,
                    stuck: None,
                    question: None,
                    error: Some(e.to_string()),
                });
            }
        };
        // Reasoning is the model's scratch space; sending it back costs
        // prefill. Usage is for the profile, not the conversation.
        if let Some(obj) = reply.as_object_mut() {
            obj.remove("reasoning_content");
            obj.remove("reasoning");
            if let Some(usage) = obj.remove("usage") {
                turn["usage"] = usage;
            }
        }
        reply["role"] = json!("assistant");
        blank = reply["tool_calls"].as_array().is_none_or(|c| c.is_empty())
            && reply["content"]
                .as_str()
                .is_none_or(|c| c.trim().is_empty());
        let calls = reply["tool_calls"].as_array().cloned().unwrap_or_default();
        messages.push(reply);
        if calls.is_empty() {
            think = true;
            turns.push(turn);
            messages.push(json!({"role": "user", "content": "Use a tool, or call `done`."}));
            continue;
        }
        let mut tool_times = Vec::new();
        // Routine unless something went wrong: then the next turn reasons.
        think = false;
        for call in calls {
            let name = call["function"]["name"].as_str().unwrap_or("");
            let args: Value = call["function"]["arguments"]
                .as_str()
                .and_then(|a| serde_json::from_str(a).ok())
                .unwrap_or(Value::Null);
            if name == "done" {
                turn["tools"] = json!(tool_times);
                turns.push(turn);
                return Ok(Outcome {
                    summary: args["summary"].as_str().map(str::to_string),
                    steps: step,
                    messages,
                    stuck: None,
                    question: None,
                    turns,
                    error: None,
                });
            }
            let ran = Instant::now();
            let sandbox = |argv: &[String]| worlds.sandbox_command(world, argv);
            let mut result = match execute(&sandbox, name, &args) {
                Ok(out) => out,
                Err(e) => format!("error: {e}"),
            };
            tool_times.push(json!({"name": name, "ms": ms(ran)}));
            if result.starts_with("error:")
                || (result.starts_with("exit ") && !result.starts_with("exit 0\n"))
            {
                think = true;
            }
            let shown = format!("{name} {}", short(&args));
            // `timeout` exits 124 (TERM) or 137 (KILL after the grace period).
            if name == "shell"
                && (result.starts_with("exit 124\n") || result.starts_with("exit 137\n"))
            {
                watch.timeouts += 1;
                if watch.timeouts >= TIMEOUT_STOP {
                    messages.push(json!({"role": "tool", "tool_call_id": call["id"], "content": truncate(&result)}));
                    turn["tools"] = json!(tool_times);
                    turns.push(turn);
                    return stop(
                        messages,
                        turns,
                        step,
                        format!(
                            "stuck: a second command hung and was killed after {COMMAND_TIMEOUT_SECS} s: {shown}"
                        ),
                    );
                }
                result = format!(
                    "timed out after {COMMAND_TIMEOUT_SECS} s and was killed. Do not run it again as is: \
                     bound it (smaller input, `timeout`, no watch mode, no prompt) or do without it.\n{result}"
                );
            }
            let repeated = watch.call(&call["function"]["arguments"].to_string(), &result);
            messages.push(json!({
                "role": "tool",
                "tool_call_id": call["id"],
                "content": truncate(&result),
            }));
            match repeated {
                Some(true) => {
                    turn["tools"] = json!(tool_times);
                    turns.push(turn);
                    return stop(messages, turns, step, format!("stuck: the same call gave the same result {REPEAT_STOP} times in a row: {shown}"));
                }
                Some(false) => messages.push(json!({"role": "user", "content": format!(
                    "You have made this exact call {REPEAT_NUDGE} times with the same result: {shown}. \
                     Repeating it will not change anything. Try a different approach, or call `done` \
                     and say what blocks you."
                )})),
                None => {}
            }
        }
        turn["tools"] = json!(tool_times);
        turns.push(turn);
        match watch.step(worlds.git(world, &["status", "--porcelain"])?) {
            Some(true) => {
                return stop(messages, turns, step, format!("stuck: no file changed in {IDLE_STOP} steps"));
            }
            Some(false) => messages.push(json!({"role": "user", "content": format!(
                "{IDLE_NUDGE} steps without changing any file. Stop exploring: make the change now, \
                 or call `done` and say what blocks you."
            )})),
            None => {}
        }
    }
    stop(
        messages,
        turns,
        max_steps,
        format!("stopped after {max_steps} steps without calling `done`"),
    )
}

/// Asks a stopped model for the one question a human could answer. It gets
/// the goal and the end of its own work as plain text, not the tool history:
/// a stop can come between a call and its result.
fn ask(model: &dyn Model, goal: &str, messages: &[Value], why: &str) -> Option<String> {
    let recent: Vec<String> = messages
        .iter()
        .rev()
        .take(8)
        .rev()
        .map(|m| {
            let text = m["content"]
                .as_str()
                .map_or_else(|| m["tool_calls"].to_string(), str::to_string);
            let cut: String = text.chars().take(1500).collect();
            format!("[{}] {cut}", m["role"].as_str().unwrap_or(""))
        })
        .collect();
    let prompt = format!(
        "You were working on this goal:\n{goal}\n\nThe end of your work:\n{}\n\n\
         The harness stopped you: {why}.\n\
         A human who knows the project, but has not read your work, can answer one question. \
         If an answer would unblock you (a choice only they can make, a missing fact, a command \
         or tool that hangs), ask it with the context needed to answer it in one line. \
         Reply in exactly this form, nothing else:\nquestion: <your question, or none>",
        recent.join("\n")
    );
    let reply = model
        .chat(&[json!({"role": "user", "content": prompt})], &json!([]))
        .ok()?;
    let content = reply["content"].as_str()?;
    // Some servers leave the reasoning inline.
    let content = content.rsplit("</think>").next().unwrap_or(content);
    parse_question(content)
}

pub(crate) fn parse_question(reply: &str) -> Option<String> {
    let q = reply
        .lines()
        .find_map(|l| l.trim().strip_prefix("question:"))?
        .trim()
        .trim_matches('`')
        .trim();
    (!q.is_empty() && !q.trim_end_matches('.').eq_ignore_ascii_case("none")).then(|| q.to_string())
}

/// Runs one tool call; `sandbox` builds the command that runs inside the
/// sandbox (a world's, or a chat's folder).
pub(crate) fn execute(
    sandbox: &dyn Fn(&[String]) -> io::Result<std::process::Command>,
    name: &str,
    args: &Value,
) -> io::Result<String> {
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
                // The same shell as the checks that judge the work (`solve::verify`).
                "bash".to_string(),
                "-c".to_string(),
                arg("command")?.to_string(),
            ];
            let out = sandbox(&argv)?.stdin(Stdio::null()).output()?;
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
            let mut child = sandbox(&argv)?
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
/// A call in one line for a reason the human reads.
fn short(args: &Value) -> String {
    let text = args["command"]
        .as_str()
        .or(args["path"].as_str())
        .map_or_else(|| args.to_string(), str::to_string);
    let line = text.lines().next().unwrap_or("");
    match line.char_indices().nth(120) {
        Some((i, _)) => format!("`{}…`", &line[..i]),
        None => format!("`{line}`"),
    }
}

pub(crate) fn bound(messages: &mut [Value]) {
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
pub(crate) fn truncate(s: &str) -> String {
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

pub(crate) fn tools() -> Value {
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
            json!({"summary": string("what you changed and why, for the person who asked, in their language")})
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn question_parsing() {
        assert_eq!(
            parse_question("Hmm.\nquestion: `Which port?`").as_deref(),
            Some("Which port?")
        );
        assert_eq!(parse_question("question: none."), None);
        assert_eq!(parse_question("I am stuck."), None);
    }

    #[test]
    fn idle_steps_nudge_once_then_stop() {
        let mut w = Watch::default();
        let verdicts: Vec<_> = (0..IDLE_STOP).map(|_| w.step(String::new())).collect();
        assert_eq!(verdicts[IDLE_NUDGE - 1], Some(false));
        assert_eq!(verdicts.iter().filter(|v| **v == Some(false)).count(), 1);
        assert_eq!(verdicts[IDLE_STOP - 1], Some(true));
        // A changed file resets the count.
        let mut w = Watch::default();
        (0..IDLE_STOP - 1).for_each(|_| _ = w.step(String::new()));
        assert_eq!(w.step(" M a".into()), None);
        assert_eq!(w.idle, 0);
    }

    #[test]
    fn a_different_result_is_not_a_repeat() {
        let mut w = Watch::default();
        for n in 0..REPEAT_STOP {
            assert_eq!(w.call("cargo test", &format!("exit 1 failure {n}")), None);
        }
    }

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
