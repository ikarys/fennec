//! The tools an agent works with, each run inside a sandbox: the harness
//! never touches the files itself, so a symlink cannot lead it outside.
//! Files are read and edited through dedicated tools, never through `sed`:
//! a substitution gone wrong corrupts a file without saying so (seen: a
//! `package.json` broken by one, then 40 steps lost repairing it).

use std::io::{self, Write};
use std::process::{Command, Stdio};

use serde_json::{Value, json};

pub(crate) const OUTPUT_LIMIT: usize = 8_000;
pub(crate) const COMMAND_TIMEOUT_SECS: u32 = 300;
/// A read shows this many lines at most; the rest is read with `offset`.
const READ_LINES: usize = 600;
const READ_BYTES: usize = 40_000;

/// Builds the command that runs `argv` inside the sandbox (a world's, or a
/// chat's folder).
pub(crate) type Sandbox<'a> = &'a dyn Fn(&[String]) -> io::Result<Command>;

/// Runs one tool call. `Err` is a failed call the model is told about.
pub(crate) fn execute(sandbox: Sandbox, name: &str, args: &Value) -> io::Result<String> {
    match name {
        "shell" => shell(sandbox, arg(args, "command")?),
        "read_file" => read_file(sandbox, args),
        "write_file" => {
            let content = arg(args, "content")?;
            write(sandbox, arg(args, "path")?, content)?;
            Ok(format!("wrote {} bytes", content.len()))
        }
        "edit_file" => edit_file(sandbox, args),
        _ => Err(io::Error::other(format!("unknown tool `{name}`"))),
    }
}

pub(crate) fn arg<'a>(args: &'a Value, k: &str) -> io::Result<&'a str> {
    args[k]
        .as_str()
        .ok_or_else(|| io::Error::other(format!("missing string argument `{k}`")))
}

/// `exit <code>\n<stdout><stderr>`, under a timeout: the same shell as the
/// checks that judge the work (`solve::verify`).
pub(crate) fn shell(sandbox: Sandbox, command: &str) -> io::Result<String> {
    let argv = [
        "timeout".to_string(),
        "-k".to_string(),
        "5".to_string(),
        COMMAND_TIMEOUT_SECS.to_string(),
        "bash".to_string(),
        "-c".to_string(),
        command.to_string(),
    ];
    let out = sandbox(&argv)?.stdin(Stdio::null()).output()?;
    Ok(format!(
        "exit {}\n{}{}",
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    ))
}

/// The whole file, as the sandbox sees it.
fn cat(sandbox: Sandbox, path: &str) -> io::Result<String> {
    let argv = ["cat".to_string(), "--".to_string(), path.to_string()];
    let out = sandbox(&argv)?.stdin(Stdio::null()).output()?;
    if !out.status.success() {
        return Err(io::Error::other(
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ));
    }
    String::from_utf8(out.stdout).map_err(|_| io::Error::other(format!("{path} is not text")))
}

fn write(sandbox: Sandbox, path: &str, content: &str) -> io::Result<()> {
    let argv = [
        "sh".to_string(),
        "-c".to_string(),
        r#"mkdir -p "$(dirname "$1")" && cat > "$1""#.to_string(),
        "sh".to_string(),
        path.to_string(),
    ];
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
        Ok(())
    } else {
        Err(io::Error::other(
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ))
    }
}

/// Numbered lines from `offset` (1-based), bounded: an edit quotes them
/// without the numbers.
fn read_file(sandbox: Sandbox, args: &Value) -> io::Result<String> {
    let path = arg(args, "path")?;
    let text = cat(sandbox, path)?;
    let from = args["offset"].as_u64().unwrap_or(1).max(1) as usize;
    let limit = args["limit"]
        .as_u64()
        .map_or(READ_LINES, |l| (l as usize).clamp(1, READ_LINES));
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return Ok(format!("{path} is empty"));
    }
    let mut out = String::new();
    let mut next = None;
    for (i, line) in lines.iter().enumerate().skip(from - 1).take(limit) {
        if out.len() + line.len() > READ_BYTES {
            next = Some(i + 1);
            break;
        }
        out.push_str(&format!("{:>5}\t{line}\n", i + 1));
    }
    let end = from - 1 + limit;
    if next.is_none() && end < lines.len() {
        next = Some(end + 1);
    }
    if let Some(n) = next {
        out.push_str(&format!(
            "[{} lines in all; read on with offset={n}]\n",
            lines.len()
        ));
    }
    Ok(out)
}

/// An exact replacement: the text to replace must be found, once unless
/// `replace_all`, so an edit never lands somewhere unexpected.
fn edit_file(sandbox: Sandbox, args: &Value) -> io::Result<String> {
    let path = arg(args, "path")?;
    let old = arg(args, "old_string")?;
    let new = arg(args, "new_string")?;
    if old.is_empty() {
        return Err(io::Error::other(
            "`old_string` is empty: use write_file to create a file",
        ));
    }
    let text = cat(sandbox, path)?;
    let found = text.matches(old).count();
    let all = args["replace_all"].as_bool().unwrap_or(false);
    match found {
        0 => Err(io::Error::other(format!(
            "`old_string` is not in {path}: read it again and quote it exactly, without line numbers"
        ))),
        n if n > 1 && !all => Err(io::Error::other(format!(
            "`old_string` is in {path} {n} times: quote more lines around it, or set replace_all"
        ))),
        n => {
            write(sandbox, path, &text.replace(old, new))?;
            Ok(format!("edited {path}: {n} replacement(s)"))
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

fn tool(name: &str, description: &str, params: Value, required: &[&str]) -> Value {
    json!({"type": "function", "function": {
        "name": name, "description": description,
        "parameters": {"type": "object", "properties": params, "required": required}
    }})
}

fn string(d: &str) -> Value {
    json!({"type": "string", "description": d})
}

/// The tools that read and change the folder; `write` adds the ones that
/// change it.
pub(crate) fn definitions(write: bool) -> Vec<Value> {
    let int = |d: &str| json!({"type": "integer", "description": d});
    let mut tools = vec![
        tool(
            "shell",
            "Run a bash command in the project root (build, tests, git, search with rg/grep/find). \
             Returns the exit code and output. Never edit files with it: use edit_file.",
            json!({"command": string("bash script")}),
            &["command"],
        ),
        tool(
            "read_file",
            "Read a text file, with line numbers. Read a file before editing it.",
            json!({"path": string("relative to the project root"),
                   "offset": int("first line to read, from 1"),
                   "limit": int("how many lines")}),
            &["path"],
        ),
    ];
    if write {
        tools.push(tool(
            "edit_file",
            "Replace exact text in a file you have read. `old_string` is quoted exactly as in \
             the file, without line numbers, with enough lines around it to be found once.",
            json!({"path": string("relative to the project root"),
                   "old_string": string("the exact text to replace"),
                   "new_string": string("its replacement"),
                   "replace_all": {"type": "boolean", "description": "replace every occurrence"}}),
            &["path", "old_string", "new_string"],
        ));
        tools.push(tool(
            "write_file",
            "Create a file, or rewrite a small one entirely. To change part of a file, use edit_file.",
            json!({"path": string("relative to the project root"), "content": string("full file content")}),
            &["path", "content"],
        ));
    }
    tools
}

/// The tool that keeps the criteria of the work in sight (see `agent`).
pub(crate) fn criteria_tool() -> Value {
    tool(
        "criteria",
        "Track what must be true when the work is finished. `add` a criterion (with a `check`, \
         a bash command that exits 0 once it holds, whenever one can prove it); mark one \
         `done` when you finished it: its check runs then, and decides.",
        json!({"action": {"type": "string", "enum": ["add", "done"]},
               "text": string("add: the criterion, in one line"),
               "check": string("add: a bash command that exits 0 once the criterion holds"),
               "number": {"type": "integer", "description": "done: the criterion's number"}}),
        &["action"],
    )
}

pub(crate) fn done_tool() -> Value {
    tool(
        "done",
        "Finish the task, once every criterion is done.",
        json!({"summary": string("what you changed and why, for the person who asked, in their language")}),
        &["summary"],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A "sandbox" that runs the command as is, in `dir`.
    fn local(dir: &std::path::Path) -> impl Fn(&[String]) -> io::Result<Command> + '_ {
        move |argv: &[String]| {
            let mut c = Command::new(&argv[0]);
            c.args(&argv[1..]).current_dir(dir);
            Ok(c)
        }
    }

    #[test]
    fn edit_reads_numbers_and_replaces_exactly_once() {
        let dir = std::env::temp_dir().join(format!("fennec-os-tools-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sb = local(&dir);
        execute(
            &sb,
            "write_file",
            &json!({"path": "a/p.json", "content": "{\n  \"build\": \"tsc -b\",\n  \"x\": 1\n}\n"}),
        )
        .unwrap();
        let read = execute(&sb, "read_file", &json!({"path": "a/p.json"})).unwrap();
        assert!(read.contains("    2\t  \"build\": \"tsc -b\","), "{read}");
        let edit = |old: &str, new: &str| {
            execute(
                &sb,
                "edit_file",
                &json!({"path": "a/p.json", "old_string": old, "new_string": new}),
            )
        };
        assert!(edit("nope", "x").is_err());
        assert!(edit("\"", "'").unwrap_err().to_string().contains("times"));
        edit("\"tsc -b\"", "\"tsc && vite build\"").unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join("a/p.json")).unwrap(),
            "{\n  \"build\": \"tsc && vite build\",\n  \"x\": 1\n}\n"
        );
        let part = execute(
            &sb,
            "read_file",
            &json!({"path": "a/p.json", "offset": 2, "limit": 1}),
        )
        .unwrap();
        assert!(part.starts_with("    2\t"), "{part}");
        assert!(part.contains("offset=3"), "{part}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
