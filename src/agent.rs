//! The engine every agent runs on: a task's attempt (`run`) and a chat's
//! turn (`chat`) alike. One loop: ask the model, run its tool calls in the
//! sandbox, again, until it finishes or the harness stops it.
//!
//! The model is framed at every step, not only by the first prompt (seen:
//! a local model given the goal once drifted, re-read the same files and
//! ran out of steps three times on a plain scaffold):
//! - criteria: what must hold when the work is done, kept by the harness,
//!   each with a check command when one can prove it; the model marks them,
//!   the check decides (the model never judges);
//! - a reminder, the last message of every call: the step, the criteria and
//!   what to do next. It is never kept: the conversation stays an
//!   append-only prefix the server caches;
//! - `done` is refused while criteria are open or their checks fail (twice,
//!   then the outer verification judges);
//! - the context is compacted into the model's own summary when it grows
//!   past the server's window, instead of cutting what the model read (cut,
//!   it read the same files again and again);
//! - loops, idle runs and hung commands are stopped, as before.

use std::collections::HashSet;
use std::io;
use std::time::Instant;

use serde_json::{Value, json};

use crate::guard::{Guard, Verdict};
use crate::provider::Model;
use crate::tools::{self, Sandbox};
use crate::world::{World, Worlds};

pub(crate) use crate::tools::truncate;

/// The same call with the same result this many times in a row is a loop:
/// the model is told once, then stopped.
const REPEAT_NUDGE: usize = 3;
pub(crate) const REPEAT_STOP: usize = 5;
/// The solve nudges at these idle steps; the framer reads a lot before it
/// writes a single test, so it is given room by [`Watch::framing`].
const IDLE_NUDGE: usize = 8;
const IDLE_HARD: usize = 12;
const IDLE_STOP: usize = 15;
/// A command killed by its timeout costs minutes; a second one ends the run.
const TIMEOUT_STOP: usize = 2;
/// `done` refused this many times at most: then the verification judges.
const REFUSALS: usize = 2;
/// A reply cut by the length limit is continued this many times at most.
const CUT_RETRIES: usize = 3;
/// A blank reply (the whole budget spent reasoning) turns reasoning off this
/// many calls, then the policy holds again: a permanent latch made the rest
/// of a long run think nothing, and a stuck model cannot reason its way out.
const BLANK_LATCH: usize = 4;
/// Two blank replies in a row end the run: the model has nothing to say.
const BLANK_STOP: usize = 2;

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
    /// The criteria as they ended: the journal shows what was met.
    pub criteria: Vec<Value>,
    /// Calls a rule wanted the person to approve, refused since nobody was
    /// there: what to allow before trying again.
    pub asked: Vec<Value>,
}

/// The person's answer to a rule's question, in a chat: `None` when the
/// turn was stopped meanwhile.
#[derive(Clone, Copy, PartialEq)]
pub enum Answer {
    No,
    Once,
    /// For the rest of the chat.
    Chat,
}

pub(crate) type Asker<'a> = &'a dyn Fn(Value) -> io::Result<Option<Answer>>;

/// What the harness says after watching a step change nothing.
#[derive(Debug, PartialEq)]
pub(crate) enum Stuck {
    /// A notice goes into the conversation; the loop keeps going.
    Nudge(String),
    /// The loop ends; the reason is recorded on the outcome.
    Stop,
}

/// Watches the loop for the ways a run gets stuck. Pure bookkeeping:
/// `call` says nudge (`Some(false)`) or stop (`Some(true)`); `step` says
/// which notice to send, or none.
pub struct Watch {
    last_call: String,
    repeats: usize,
    nudged: usize,
    files: String,
    idle: usize,
    timeouts: usize,
    nudged_idle: bool,
    nudged_idle_hard: bool,
    idle_nudge: usize,
    idle_hard: usize,
    idle_stop: usize,
}

impl Default for Watch {
    fn default() -> Self {
        Self {
            last_call: String::new(),
            repeats: 0,
            nudged: 0,
            files: String::new(),
            idle: 0,
            timeouts: 0,
            nudged_idle: false,
            nudged_idle_hard: false,
            idle_nudge: IDLE_NUDGE,
            idle_hard: IDLE_HARD,
            idle_stop: IDLE_STOP,
        }
    }
}

/// How many times the same call is nudged before the run ends on it: the
/// first notice is read, the second is not.
const REPEAT_NUDGES: usize = 2;
/// Only the head of the result joins the repeat key: a call that outputs a
/// growing log is a loop on its command, not on its last line.
const FINGERPRINT: usize = 256;

impl Watch {
    /// The framing attempt: it explores the code before it writes a single
    /// test, so the idle notices arrive late.
    pub const fn framing() -> Self {
        Self {
            last_call: String::new(),
            repeats: 0,
            nudged: 0,
            files: String::new(),
            idle: 0,
            timeouts: 0,
            nudged_idle: false,
            nudged_idle_hard: false,
            idle_nudge: 15,
            idle_hard: 20,
            idle_stop: 25,
        }
    }

    /// `call` is the tool's arguments, canonicalized by the caller; the
    /// result's first lines join them. A changed file, a changed output, a
    /// changed argument: the run is moving, the count starts over.
    pub(crate) fn call(&mut self, call: &str, result: &str) -> Option<bool> {
        let fingerprint: String = result.chars().take(FINGERPRINT).collect();
        let key = format!("{call}\n{fingerprint}");
        if key == self.last_call {
            self.repeats += 1;
        } else {
            (self.last_call, self.repeats, self.nudged) = (key, 1, 0);
        }
        if self.repeats >= REPEAT_STOP {
            Some(true)
        } else if self.repeats >= REPEAT_NUDGE && self.nudged < REPEAT_NUDGES {
            self.nudged += 1;
            Some(false)
        } else {
            None
        }
    }

    fn step(&mut self, files: String) -> Option<Stuck> {
        if files == self.files {
            self.idle += 1;
        } else {
            (
                self.files,
                self.idle,
                self.nudged_idle,
                self.nudged_idle_hard,
            ) = (files, 0, false, false);
        }
        if self.idle >= self.idle_stop {
            Some(Stuck::Stop)
        } else if self.idle >= self.idle_hard && !self.nudged_idle_hard {
            self.nudged_idle_hard = true;
            Some(Stuck::Nudge(format!(
                "{} steps without a single file changed. Edit now — change a file \
                 that moves the work forward, in this step — or call `done` and say exactly \
                 what blocks you.",
                self.idle_hard
            )))
        } else if self.idle >= self.idle_nudge && !self.nudged_idle {
            self.nudged_idle = true;
            Some(Stuck::Nudge(format!(
                "{} steps without changing any file. Stop exploring: make the change \
                 now, or call `done` and say what blocks you.",
                self.idle_nudge
            )))
        } else {
            None
        }
    }
}

/// A criterion: with a check it is work to finish (`open`), without one a
/// constraint to respect (`rule`), which the harness checks afterwards.
pub fn criterion(text: &str, check: Option<&str>) -> Value {
    json!({"text": text, "check": check, "state": if check.is_some() { "open" } else { "rule" }})
}

/// A step of an approved plan: to finish, nothing runs to prove it.
pub(crate) fn step_criterion(text: &str) -> Value {
    json!({"text": text, "check": null, "state": "open"})
}

/// Done or proven: what `done` no longer waits for.
fn met(c: &Value) -> bool {
    matches!(c["state"].as_str(), Some("done" | "passed" | "rule"))
}

/// A message the harness adds to the conversation: sent as the person's,
/// hidden from them on the page.
pub(crate) fn harness(text: &str) -> Value {
    json!({"role": "user", "content": text, "harness": true})
}

/// The tokens past which the conversation is compacted: most of the
/// server's window, leaving room for the reply.
/// ponytail: one window for every model (`FENNEC_OS_CONTEXT`, default 64k
/// tokens); ask the server (`max_model_len`) if models differ widely.
fn compact_at() -> u64 {
    let window = std::env::var("FENNEC_OS_CONTEXT")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(65_536);
    window * 7 / 10
}

/// One loop's state, the same for a task and a chat.
pub(crate) struct Engine<'a> {
    model: &'a dyn Model,
    pub(crate) criteria: Vec<Value>,
    pub(crate) watch: Watch,
    pub(crate) turns: Vec<Value>,
    /// Files read (or written) in this loop: an edit needs one.
    read: HashSet<String>,
    refusals: usize,
    /// Reasoning is skipped on routine turns under the `adaptive` policy.
    think: bool,
    policy: String,
    /// A reply with nothing in it: the model spent its whole budget
    /// reasoning. Asked again the same way it does it again (seen: four
    /// times in six, 2-3 minutes each on a local model), so the next
    /// [`BLANK_LATCH`] calls go without reasoning; after them the policy
    /// holds again, so a stuck model can think its way out.
    blank: usize,
    cuts: usize,
    /// Model calls made: the parent's and a `task` child's steps share the
    /// same budget, so the loop counts calls, not iterations.
    steps: usize,
    /// A task's loop refuses `done` while the criteria list is empty: the
    /// work must be planned before it is judged. A chat's plan steps are
    /// approved by the person, not written by the model: it stays false.
    pub(crate) require_criteria: bool,
    /// The reply `ask` just got was cut by the length limit, calls included.
    last_cut: bool,
    /// The reply `ask` just got was blank (no call, no text): a chat ends on
    /// two of them in a row.
    pub(crate) last_blank: bool,
    /// The `task` tool, offered only in a task's loop: the child runs in the
    /// same world, read-only, and its steps come out of the parent's budget.
    task_worlds: Option<&'a Worlds>,
    task_world: Option<&'a World>,
    task_budget: usize,
    /// What the last call cost in prompt tokens: the size of the context.
    context: u64,
    start: Instant,
    /// The person's rules on commands and files, checked before each call.
    pub(crate) guard: Option<Guard>,
    /// Who answers a rule's question; nobody in a task: refused, recorded.
    pub(crate) asker: Option<Asker<'a>>,
    /// Rules the person allowed for the rest of the chat.
    pub(crate) allowed: Vec<Value>,
    pub(crate) asked: Vec<Value>,
    /// The role's tools line: `None` runs with every tool.
    pub(crate) tools: Option<Vec<crate::team::Tool>>,
}

impl<'a> Engine<'a> {
    pub(crate) fn new(model: &'a dyn Model, criteria: Vec<Value>) -> Self {
        Self {
            model,
            criteria,
            watch: Watch::default(),
            turns: Vec::new(),
            read: HashSet::new(),
            refusals: 0,
            // Non-thinking first: on the local server (ninfer, measured
            // 2026-10-21) a thinking request never stores its prompt KV, so
            // the next turn's prefix recompute is cold (0 cached tokens);
            // the first non-thinking turn stores it and every later turn
            // reuses it (measured: 10048/10055 cached on a repeated call).
            // Thinking comes back after a tool error, one turn only.
            think: false,
            // FENNEC_OS_THINK=always|adaptive|never wins over the settings'
            // think (always, since a thinking model that skips routine
            // reasoning answers routine turns with a non-thinking model);
            // adaptive skips reasoning on routine turns so the server's
            // prefix cache pays.
            policy: std::env::var("FENNEC_OS_THINK")
                .ok()
                .or_else(|| crate::provider::Settings::current().ok().map(|s| s.think))
                .unwrap_or_else(|| crate::provider::DEFAULT_THINK.to_string()),
            blank: 0,
            cuts: 0,
            steps: 0,
            context: 0,
            start: Instant::now(),
            guard: None,
            asker: None,
            allowed: Vec::new(),
            asked: Vec::new(),
            tools: None,
            require_criteria: false,
            last_cut: false,
            last_blank: false,
            task_worlds: None,
            task_world: None,
            task_budget: 0,
        }
    }

    /// What the person's rules say of a call: `None` to run it, or what the
    /// model is told instead.
    fn guarded(&mut self, name: &str, args: &Value) -> Option<String> {
        // The harness's own rule, before the person's: no rule of theirs lifts it.
        if name == "shell"
            && let Some(flag) = crate::guard::stray_install(args["command"].as_str().unwrap_or(""))
        {
            return Some(format!(
                "error: refused by the harness: `{flag}` installs packages into the source tree \
                 or the home. Install into the project's virtual environment (`.venv`) instead."
            ));
        }
        let verdict = self.guard.as_ref()?.check(name, args);
        let (rule, target) = match verdict {
            Verdict::Allow => return None,
            Verdict::Deny(rule, target) => {
                return Some(format!(
                    "error: refused by the person's rule `{}` ({}) on `{target}`. Do without it; \
                     do not try another way around the rule.",
                    rule["pattern"].as_str().unwrap_or(""),
                    rule["why"].as_str().unwrap_or("")
                ));
            }
            Verdict::Ask(rule, target) => (rule, target),
        };
        if self.allowed.contains(&rule) {
            return None;
        }
        let why = rule["why"].as_str().unwrap_or("").to_string();
        let Some(asker) = self.asker else {
            self.asked
                .push(json!({"rule": rule, "target": target, "tool": name}));
            return Some(format!(
                "error: the person's rules ask them first ({why}: `{target}`), and nobody can \
                 answer during this task. Do without it, and say in your summary what you needed."
            ));
        };
        let question = json!({"tool": "rule", "rule": rule.clone(), "target": target,
                              "call": format!("{name} {}", short(args))});
        match asker(question) {
            Ok(Some(Answer::Once)) => None,
            Ok(Some(Answer::Chat)) => {
                self.allowed.push(rule);
                None
            }
            Ok(Some(Answer::No)) => Some(format!(
                "error: the person refused ({why}: `{target}`); it did not run. Do not retry it."
            )),
            Ok(None) => Some("error: the person stopped the turn; it did not run".to_string()),
            Err(e) => Some(format!("error: {e}")),
        }
    }

    /// One model call. The reply comes back as it goes into the
    /// conversation; the turn record (`self.turns`, last) keeps what it
    /// cost. `true` with it: the reply was cut by the length limit.
    pub(crate) fn ask(&mut self, window: &[Value], tools: &Value) -> io::Result<(Value, bool)> {
        let think = self.blank == 0
            && match self.policy.as_str() {
                "never" => false,
                "adaptive" => self.think,
                _ => true,
            };
        self.steps += 1;
        let at = self.start.elapsed().as_millis() as u64;
        let asked = Instant::now();
        let reply = if think {
            self.model.chat(window, tools)
        } else {
            self.model.chat_quick(window, tools)
        };
        let mut turn = json!({"step": self.turns.len() + 1, "at_ms": at,
                              "model_ms": asked.elapsed().as_millis() as u64, "think": think});
        let mut reply = match reply {
            Ok(r) => r,
            Err(e) => {
                self.turns.push(turn);
                return Err(e);
            }
        };
        let mut cut = false;
        if let Some(obj) = reply.as_object_mut() {
            if let Some(usage) = obj.remove("usage") {
                turn["usage"] = usage;
            }
            cut = obj.remove("finish_reason").is_some_and(|f| f == "length");
        }
        // The server's count when it gives one; four bytes a token if not.
        self.context = turn["usage"]["prompt_tokens"]
            .as_u64()
            .unwrap_or_else(|| window.iter().map(|m| m.to_string().len() as u64 / 4).sum());
        reply["role"] = json!("assistant");
        let calls = reply["tool_calls"]
            .as_array()
            .is_some_and(|c| !c.is_empty());
        let text = reply["content"]
            .as_str()
            .is_some_and(|c| !c.rsplit("</think>").next().unwrap_or("").trim().is_empty());
        if !calls && !text {
            self.blank = BLANK_LATCH;
            self.last_blank = true;
        } else {
            self.blank = self.blank.saturating_sub(1);
            self.last_blank = false;
        }
        self.last_cut = cut;
        // Routine unless something goes wrong: a tool error reasons the next
        // turn, and the next tool call ends it. A text-only turn changes
        // nothing: on the local server a thinking turn stores no prompt KV,
        // so reasoning the loop back on itself after every pause would cost
        // the whole prefix cache.
        self.think = !calls && self.think;
        self.turns.push(turn);
        Ok((reply, cut && !calls))
    }

    /// Whether a cut reply is continued; the message that asks for it.
    pub(crate) fn continue_cut(&mut self) -> Option<Value> {
        self.cuts += 1;
        (self.cuts <= CUT_RETRIES).then(|| {
            harness(
                "Your reply was cut by the length limit. Continue where it stopped, shorter: \
                 one step at a time, and write long files in several edits.",
            )
        })
    }

    /// Runs one tool call (not `done`, nor the chat's own ones) and returns
    /// what goes back to the model, its time recorded in the last turn.
    pub(crate) fn tool(&mut self, sandbox: Sandbox, call: &Value) -> String {
        let name = call["function"]["name"].as_str().unwrap_or("");
        let started = Instant::now();
        let raw = call["function"]["arguments"].as_str().unwrap_or("{}");
        let result = match serde_json::from_str::<Value>(raw) {
            Err(e) => format!(
                "error: the arguments are not valid JSON ({e}); call the tool again with valid JSON"
            ),
            Ok(args) => {
                if name == "task" {
                    self.task_tool(sandbox, &args)
                } else {
                    self.run_tool(sandbox, name, &args)
                }
            }
        };
        if result.starts_with("error:")
            || (result.starts_with("exit ") && !result.starts_with("exit 0\n"))
        {
            self.think = true;
        }
        if let Some(turn) = self.turns.last_mut() {
            if !turn["tools"].is_array() {
                turn["tools"] = json!([]);
            }
            if let Some(t) = turn["tools"].as_array_mut() {
                t.push(json!({"name": name, "ms": started.elapsed().as_millis() as u64}));
            }
        }
        result
    }

    fn run_tool(&mut self, sandbox: Sandbox, name: &str, args: &Value) -> String {
        if let Some(refused) = self.guarded(name, args) {
            return refused;
        }
        if let Some(refused) = self.role_tool(name, args) {
            return refused;
        }
        let path = args["path"].as_str().unwrap_or("").trim_start_matches("./");
        match name {
            "criteria" => return self.criteria_call(sandbox, args),
            "edit_file" if !self.read.contains(path) => {
                return format!(
                    "error: read {path} with read_file before editing it: the edit quotes it exactly"
                );
            }
            _ => {}
        }
        match tools::execute(sandbox, name, args) {
            Ok(out) => {
                if matches!(name, "read_file" | "write_file" | "edit_file") {
                    self.read.insert(path.to_string());
                }
                if name == "read_file" {
                    out
                } else {
                    truncate(&out)
                }
            }
            Err(e) => format!("error: {e}"),
        }
    }

    /// The role's `tools` line, checked where the tool runs: what it does not
    /// list is refused, and a tool limited to a path is refused outside it.
    /// The brief only asks; this refuses.
    fn role_tool(&self, name: &str, args: &Value) -> Option<String> {
        let tools = self.tools.as_ref()?;
        // The harness keeps the criteria and `done` for every role.
        if matches!(name, "criteria" | "done") {
            return None;
        }
        let want = name.strip_suffix("_file").unwrap_or(name);
        let Some(t) = tools.iter().find(|t| t.name == want) else {
            return Some(format!(
                "error: refused: `{name}` is not allowed for this role (allowed: {})",
                crate::team::tool_line(tools)
            ));
        };
        if let Some(prefix) = &t.prefix {
            let path = args["path"].as_str().unwrap_or("").trim_start_matches("./");
            if !path.starts_with(prefix.as_str()) {
                return Some(format!(
                    "error: refused: this role may write `{want}` under `{prefix}`, not `{path}`"
                ));
            }
        }
        None
    }

    fn criteria_call(&mut self, sandbox: Sandbox, args: &Value) -> String {
        match args["action"].as_str() {
            Some("add") => {
                let Some(text) = args["text"].as_str().filter(|t| !t.trim().is_empty()) else {
                    return "error: `add` needs the criterion's `text`".to_string();
                };
                let check = args["check"].as_str().filter(|c| !c.trim().is_empty());
                let mut c = criterion(text.trim(), check);
                c["state"] = json!("open");
                self.criteria.push(c);
                format!("added.\n{}", render(&self.criteria))
            }
            Some("done") => {
                let n = args["number"].as_u64().unwrap_or(0) as usize;
                let Some(i) = n.checked_sub(1).filter(|i| *i < self.criteria.len()) else {
                    return format!(
                        "error: no criterion number {n}.\n{}",
                        render(&self.criteria)
                    );
                };
                let failed = self.check(sandbox, i);
                match failed {
                    None => format!("criterion {n} met.\n{}", render(&self.criteria)),
                    Some(out) => format!(
                        "criterion {n} is not met: its check failed.\n{out}\nFix it, then mark it done again."
                    ),
                }
            }
            _ => "error: `action` is `add` or `done`".to_string(),
        }
    }

    /// Marks criterion `i` done, its check deciding when it has one. The
    /// check's output when it failed.
    fn check(&mut self, sandbox: Sandbox, i: usize) -> Option<String> {
        let c = &mut self.criteria[i];
        let Some(check) = c["check"].as_str().map(str::to_string) else {
            if c["state"] != "rule" {
                c["state"] = json!("done");
            }
            return None;
        };
        // A check is a command the model wrote: the rules hold for it too,
        // without asking (nobody waits on a check).
        let refused =
            self.guard
                .as_ref()
                .and_then(|g| match g.check("shell", &json!({"command": check})) {
                    Verdict::Allow => None,
                    Verdict::Ask(r, t) | Verdict::Deny(r, t) => Some(format!(
                        "exit -1\nrefused by the person's rule `{}` on `{t}`",
                        r["pattern"].as_str().unwrap_or("")
                    )),
                });
        let out = refused.unwrap_or_else(|| {
            tools::shell(sandbox, &check).unwrap_or_else(|e| format!("exit -1\n{e}"))
        });
        if out.starts_with("exit 0\n") {
            c["state"] = json!("passed");
            c["failure"] = Value::Null;
            return None;
        }
        let tail: Vec<&str> = out.lines().collect();
        let tail = tail[tail.len().saturating_sub(15)..].join("\n");
        c["state"] = json!("failed");
        c["failure"] = json!(tail);
        Some(format!("`{check}`:\n{}", truncate(&out)))
    }

    /// Whether `done` is accepted. Every unproven check runs first. A
    /// refusal says what is left; after [`REFUSALS`] of them `done` goes
    /// through, the verification then judging.
    pub(crate) fn finish(&mut self, sandbox: Sandbox) -> Option<String> {
        if self.require_criteria && self.criteria.is_empty() {
            self.refusals += 1;
            if self.refusals >= REFUSALS {
                return None;
            }
            return Some(
                "Not finished: no criterion says what the work must meet. Add them with \
                 `criteria` (action add), at least two, each with a check command whenever one \
                 can prove it, then call `done` again."
                    .to_string(),
            );
        }
        for i in 0..self.criteria.len() {
            if self.criteria[i]["check"].is_string() && self.criteria[i]["state"] != "passed" {
                self.check(sandbox, i);
            }
        }
        if self.criteria.iter().all(met) || self.refusals >= REFUSALS {
            return None;
        }
        self.refusals += 1;
        Some(format!(
            "Not finished: some criteria are not met.\n{}\nFinish them (mark each with `criteria`), \
             then call `done` again. If one cannot be met, say why in the summary.",
            render(&self.criteria)
        ))
    }

    /// Criteria left to finish.
    pub(crate) fn open(&self) -> usize {
        self.criteria.iter().filter(|c| !met(c)).count()
    }

    /// The files read so far, sorted: the run log keeps them, a resume starts
    /// with them.
    pub(crate) fn read_set(&self) -> Vec<String> {
        let mut v: Vec<String> = self.read.iter().cloned().collect();
        v.sort();
        v
    }

    /// The `task` tool: a sub-agent in the same world, read-only, its own
    /// conversation, its own budget drawn from the parent's. Only its summary
    /// comes back, so the parent's context does not grow on its explorations.
    fn task_tool(&mut self, _sandbox: Sandbox, args: &Value) -> String {
        let (worlds, world) = match (self.task_worlds, self.task_world) {
            (Some(w), Some(d)) if self.task_budget > 0 => (w, d),
            _ => return "error: the task tool is not available here".to_string(),
        };
        let budget = self.task_budget;
        let goal = match args["goal"].as_str().filter(|g| !g.trim().is_empty()) {
            Some(g) => g.to_string(),
            None => {
                return "error: `task` needs a `goal`: what the sub-agent must find or decide"
                    .to_string();
            }
        };
        // Half of what is left, as the sub-agent's own budget; the rest stays
        // for the parent to do the work.
        let left = budget.saturating_sub(self.steps).max(1);
        let asked = args["steps"]
            .as_u64()
            .map(|s| s as usize)
            .unwrap_or(left / 2 + 1);
        let child_max = asked.min(left / 2 + 1).max(1);
        let outcome = crate::agent::run(
            worlds,
            world,
            self.model,
            &goal,
            Vec::new(),
            "",
            child_max,
            None,
            true,
            None,
            Watch::default(),
        );
        let child = match outcome {
            Ok(o) => o,
            Err(e) => return format!("error: the sub-agent failed: {e}"),
        };
        // The child's steps count against the parent's budget.
        self.steps += child.steps;
        let what = match (child.summary, child.stuck, child.error) {
            (Some(s), _, _) => s,
            (None, Some(s), _) => format!("stuck: {s}"),
            (None, None, Some(e)) => format!("failed: {e}"),
            (None, None, None) => "the sub-agent used its steps without a summary".to_string(),
        };
        const CAP: usize = 2_000;
        let mut out: String = what.chars().take(CAP).collect();
        if what.chars().count() > CAP {
            out.push_str("… (the sub-agent's answer was cut)");
        }
        out
    }

    /// The reminder sent as the last message of a call; `None` when there
    /// is nothing to frame (a chat without a plan).
    pub(crate) fn reminder(&self, step: usize, max: usize, mode: &str) -> Value {
        let mut text = format!("<system-reminder>\nStep {step} of at most {max}. {mode}\n");
        let next = self.criteria.iter().position(|c| !met(c));
        if self.criteria.is_empty() {
            text.push_str(
                "No criteria yet: add the ones this work must meet with `criteria` (action add), \
                 each with a check command whenever one can prove it.\n",
            );
        } else {
            text.push_str(&render(&self.criteria));
            match next {
                Some(i) => text.push_str(&format!(
                    "Next: criterion {} — make the change, then mark it done with `criteria`.\n",
                    i + 1
                )),
                None => text.push_str("Every criterion is met: call `done`.\n"),
            }
        }
        if step * 5 >= max * 4 {
            match next {
                Some(i) => text.push_str(&format!(
                    "Few steps are left. Finish criterion {i} now: make the change, run its \
                     check, mark it done. Do not end with what remains.\n",
                )),
                None => text.push_str(&format!(
                    "Few steps are left: {}. Call `done` now.",
                    if self.criteria.is_empty() {
                        "add the criteria first"
                    } else {
                        "every criterion is met"
                    }
                )),
            }
        }
        text.push_str("</system-reminder>");
        json!({"role": "user", "content": text})
    }

    pub(crate) fn needs_compaction(&self) -> bool {
        self.context > compact_at()
    }

    /// The context no longer counts: the conversation was cut and rebuilt.
    pub(crate) fn reset_context(&mut self) {
        self.context = 0;
    }

    /// The model's own summary of `window`, to go on with instead of it.
    pub(crate) fn compact(&mut self, window: &[Value]) -> io::Result<String> {
        let mut asked = window.to_vec();
        asked.push(json!({"role": "user", "content":
            "The conversation is about to be shortened. Write down, for yourself, everything you \
             need to go on: the goal, what is done (files changed, commands that work), what \
             failed and why, what remains, and the facts you will need (paths, names, errors, \
             decisions). Plain text, no tool call."}));
        let at = self.start.elapsed().as_millis() as u64;
        let (summary, exchange) = crate::provider::ask(self.model, &asked)?;
        for mut t in exchange["turns"].as_array().cloned().unwrap_or_default() {
            t["at_ms"] = json!(at);
            t["compaction"] = json!(true);
            self.turns.push(t);
        }
        self.context = 0;
        if summary.is_empty() {
            return Err(io::Error::other("the model gave no summary"));
        }
        Ok(summary)
    }
}

/// The criteria, numbered, as the model and the journal read them.
fn render(criteria: &[Value]) -> String {
    let mut out = String::from("Criteria:\n");
    for (i, c) in criteria.iter().enumerate() {
        let mark = match c["state"].as_str() {
            Some("passed") => "[x] (check passed)",
            Some("done") => "[x]",
            Some("failed") => "[ ] (check failed)",
            Some("rule") => "[rule]",
            _ => "[ ]",
        };
        out.push_str(&format!(
            "  {}. {mark} {}",
            i + 1,
            c["text"].as_str().unwrap_or("")
        ));
        if let Some(check) = c["check"].as_str() {
            out.push_str(&format!(" — check: `{check}`"));
        }
        out.push('\n');
        if let Some(f) = c["failure"].as_str().filter(|_| c["state"] == "failed") {
            let last = f.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("");
            out.push_str(&format!("     last failure: {last}\n"));
        }
    }
    out
}

const TASK: &str = "You work in a sandboxed copy of a project; the current directory is its root.\n\
You have network access: install what the project needs inside it, with the project's own \
tools (its manifest, lockfile or build file), since the home is emptied after each command and \
the checks that judge the work run without network. Dependencies go where the project's \
ecosystem keeps them (`.venv`, `node_modules`, `target`...), never committed; download caches \
are kept for you in the project's `.fennec/`, shared by all its copies (a Cargo `target/` too). \
Never install into the system or the home (`--user`, \
`--break-system-packages`, global installs), never copy packages into the source tree.\n\
You cannot commit: the harness checkpoints your work. Anything you break can be rolled back, \
so act directly instead of asking. If the goal is ambiguous about code structure, read the \
code and decide — do not ask; only ask the human about product decisions the goal does not cover.\n\
Plan before code: once you have read enough to scope the work, add the criteria it must meet \
with `criteria` (action add) — at least two, each in one line, each with a `check` (a bash \
command that exits 0 once it holds) whenever it can be proven: a test, a build, the content of \
a file. The harness keeps them, reminds you of them every step, and refuses `done` while one \
is open or the list is empty.\n\
Work in small verified steps: read a file with read_file before you touch it (it comes with \
line numbers and says how to read on with offset); change it with edit_file (its old_string \
must be the exact text, found once) or write_file for a new file, never with sed or a script; \
run the build and the tests with shell after each change (a command over 300 s is killed: \
bound it with timeout); mark each criterion done with `criteria` (action done) as it holds. \
Several independent reads may go in one reply: they run at once.\n\
Call `done` when every criterion is met. If one cannot be met, say why in the summary instead \
of looping; if you are stuck, call `done` with what is left and why.";

/// A sub-agent's brief: it reads, it answers, it changes nothing.
const TASK_RO: &str = "You work in a sandboxed copy of a project; the current directory is its \
root. You are a sub-agent: you read and you answer, you change nothing — there is no write \
tool. Read what the goal needs (read_file, shell for search with rg/grep/find, builds and \
tests to check a claim), then call `done` with a short, complete answer to the goal: the facts, \
the paths and the verdict. Do not pad; the parent agent reads only your summary.";

/// What the sandbox finds installed, so the agent picks among real tools
/// instead of guessing.
pub(crate) fn tools_line(worlds: &Worlds) -> String {
    match worlds.tools() {
        [] => String::new(),
        t => format!("\nInstalled on this machine: {}.", t.join(", ")),
    }
}

/// A task's attempt in `world`: `goal` to reach, `criteria` to meet (see
/// [`criterion`]), `context` added to the system prompt. `log`, when given,
/// is written atomically after every model call, so an attempt that crashes
/// in the middle resumes from it ([`resume`]).
#[allow(clippy::too_many_arguments)]
pub fn run(
    worlds: &Worlds,
    world: &World,
    model: &dyn Model,
    goal: &str,
    criteria: Vec<Value>,
    context: &str,
    max_steps: usize,
    tools: Option<&[crate::team::Tool]>,
    read_only: bool,
    log: Option<&std::path::Path>,
    watch: Watch,
) -> io::Result<Outcome> {
    run_in(
        worlds, world, model, goal, criteria, context, max_steps, tools, read_only, log, None,
        watch,
    )
}

/// What a crashed attempt left in its log: the whole conversation (the
/// system message first), where the model looks from, the criteria as they
/// stand, the files it read.
pub struct Resume {
    pub goal: String,
    /// The live log, without the system message; `from` indexes it.
    pub messages: Vec<Value>,
    pub from: usize,
    pub criteria: Vec<Value>,
    pub read: Vec<String>,
}

/// A crashed attempt back at work: the same world (its changes are kept),
/// its criteria and its reads, the conversation on where it stopped — its
/// last reply's calls, not run yet, are run again.
pub fn resume(
    worlds: &Worlds,
    world: &World,
    model: &dyn Model,
    state: Resume,
    max_steps: usize,
    tools: Option<&[crate::team::Tool]>,
    log: Option<&std::path::Path>,
) -> io::Result<Outcome> {
    let goal = state.goal.clone();
    run_in(
        worlds,
        world,
        model,
        &goal,
        Vec::new(),
        "",
        max_steps,
        tools,
        false,
        log,
        Some(state),
        Watch::default(),
    )
}

#[allow(clippy::too_many_arguments)]
fn run_in(
    worlds: &Worlds,
    world: &World,
    model: &dyn Model,
    goal: &str,
    criteria: Vec<Value>,
    context: &str,
    max_steps: usize,
    tools: Option<&[crate::team::Tool]>,
    read_only: bool,
    log: Option<&std::path::Path>,
    resumed: Option<Resume>,
    watch: Watch,
) -> io::Result<Outcome> {
    // An empty `tools` line means no restriction, like no line at all.
    let role_tools = tools.filter(|t| !t.is_empty()).map(|t| t.to_vec());
    let mut defs: Vec<Value> = tools::definitions(!read_only);
    if !read_only {
        defs.push(tools::criteria_tool());
        defs.push(tools::task_tool());
    }
    defs.push(tools::done_tool());
    let defs = Value::Array(defs);
    let system = json!({"role": "system",
    "content": format!(
        "{}{}{context}",
        if read_only { TASK_RO } else { TASK },
        tools_line(worlds)
    )});
    let log_path = log;
    let (log, from, criteria, read) = match resumed {
        Some(r) => (r.messages, r.from, r.criteria, r.read.into_iter().collect()),
        None => (
            vec![json!({"role": "user", "content": goal})],
            0,
            criteria,
            std::collections::HashSet::new(),
        ),
    };
    let mut engine = Engine::new(model, criteria);
    engine.watch = watch;
    engine.read = read;
    engine.tools = role_tools;
    // A direct run must say what good looks like before it may finish; a
    // team role is steered by its brief, a sub-agent is read-only.
    engine.require_criteria = !read_only && tools.is_none();
    engine.task_worlds = Some(worlds);
    engine.task_world = Some(world);
    engine.task_budget = max_steps;
    engine.guard = Some(Guard::load(
        &worlds.project(world)?,
        &world.path,
        &worlds.home,
    )?);
    drive(
        worlds, world, model, goal, system, engine, log, from, max_steps, defs, log_path,
    )
}

/// One loop, a task's or a sub-agent's: model calls and tools until `done`,
/// a stop the harness decides, or the budget.
#[allow(clippy::too_many_arguments)]
fn drive(
    worlds: &Worlds,
    world: &World,
    model: &dyn Model,
    goal: &str,
    system: Value,
    mut engine: Engine,
    mut log: Vec<Value>,
    mut from: usize,
    max_steps: usize,
    defs: Value,
    log_path: Option<&std::path::Path>,
) -> io::Result<Outcome> {
    let sandbox = |argv: &[String]| worlds.sandbox_work(world, argv);
    // The starting state counts as "no change yet".
    engine.watch.files = worlds.status(world).unwrap_or_default();
    let end = |engine: Engine, log: Vec<Value>, step, summary, stuck: Option<String>, error| {
        let mut messages = vec![system.clone()];
        messages.extend(log);
        let question = stuck
            .as_deref()
            .and_then(|why| ask(model, goal, &messages, why));
        if let Some(p) = log_path {
            write_run_log(p, world, goal, step, 0, &engine, &messages);
        }
        Ok(Outcome {
            summary,
            steps: step,
            messages,
            stuck,
            question,
            turns: engine.turns,
            error,
            criteria: engine.criteria,
            asked: engine.asked,
        })
    };
    const MODE: &str = "Change the files to meet the criteria; read before you edit, edit \
                        with edit_file, run the build and tests after each change.";
    let mut step = 0;
    let mut blanks = 0;
    // A resumed log may end on a reply whose calls never ran: they run now,
    // without a model call.
    let mut pending = log
        .last()
        .filter(|m| m["role"].as_str() == Some("assistant"))
        .and_then(|m| m["tool_calls"].as_array())
        .filter(|c| !c.is_empty())
        .cloned();
    while engine.steps < max_steps {
        step += 1;
        if engine.needs_compaction() {
            let window: Vec<Value> = std::iter::once(system.clone())
                .chain(log[from..].iter().cloned())
                .collect();
            let files = worlds.status(world).unwrap_or_default();
            match engine.compact(&window) {
                Ok(summary) => {
                    log.push(harness(&format!(
                        "The goal:\n{goal}\n\nThe conversation was shortened; your own notes on \
                         the work so far:\n{summary}\n\nThe criteria as they stand now:\n{}\n\
                         The files changed so far:\n{files}",
                        render(&engine.criteria)
                    )));
                    from = log.len() - 1;
                }
                // The model gave no notes (an empty summary is an error):
                // the goal and the end of the exchange stay, so the run
                // goes on instead of dying on it.
                Err(_) => {
                    const KEEP: usize = 12;
                    let tail: Vec<Value> = log
                        .iter()
                        .rev()
                        .take(KEEP)
                        .cloned()
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .collect();
                    log = vec![json!({"role": "user", "content": goal})];
                    log.extend(tail);
                    log.push(harness(
                        "The conversation was shortened and left no notes: the older messages \
                         are gone. The goal is above; re-read the files you need with read_file.",
                    ));
                    from = 0;
                    engine.context = 0;
                }
            }
        }
        let calls: Vec<Value> = match pending.take() {
            Some(c) => c,
            None => {
                let mut window: Vec<Value> = std::iter::once(system.clone())
                    .chain(log[from..].iter().cloned())
                    .collect();
                window.push(engine.reminder(step, max_steps, MODE));
                let (reply, cut) = match engine.ask(&window, &defs) {
                    Ok(r) => r,
                    Err(e) => return end(engine, log, step, None, None, Some(e.to_string())),
                };
                let calls = reply["tool_calls"].as_array().cloned().unwrap_or_default();
                let text: String = reply["content"]
                    .as_str()
                    .unwrap_or("")
                    .rsplit("</think>")
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                log.push(reply);
                if let Some(p) = log_path {
                    let mut messages = vec![system.clone()];
                    messages.extend(log.iter().cloned());
                    write_run_log(p, world, goal, step, from, &engine, &messages);
                }
                // A cut is the server's limit, not the agent's step: it is
                // continued without costing one.
                if cut
                    && calls.is_empty()
                    && let Some(more) = engine.continue_cut()
                {
                    log.push(more);
                    continue;
                }
                if calls.is_empty() && text.is_empty() {
                    blanks += 1;
                    if blanks >= BLANK_STOP {
                        let why = format!(
                            "stopped: the model gave an empty reply {BLANK_STOP} times in a row"
                        );
                        return end(engine, log, step, None, Some(why), None);
                    }
                    log.push(harness("Your reply was empty. Use a tool, or call `done`."));
                    continue;
                }
                blanks = 0;
                if calls.is_empty() {
                    log.push(harness("Use a tool, or call `done`."));
                    continue;
                }
                calls
            }
        };
        // `done` ends the run; it is settled first, the rest then runs.
        for call in &calls {
            if call["function"]["name"].as_str().unwrap_or("") != "done" {
                continue;
            }
            let refused = engine.finish(&sandbox);
            if let Some(why) = refused {
                log.push(json!({"role": "tool", "tool_call_id": call["id"], "content": why}));
                continue;
            }
            let args: Value = call["function"]["arguments"]
                .as_str()
                .and_then(|a| serde_json::from_str(a).ok())
                .unwrap_or(Value::Null);
            let summary = args["summary"].as_str().map(str::to_string);
            return end(engine, log, step, summary, None, None);
        }
        // The calls: the reads run at once, the rest one after the other,
        // and the results go back in the order the model made them.
        let mut results: Vec<Option<String>> = vec![None; calls.len()];
        let mut reads: Vec<(usize, Value)> = Vec::new();
        for (i, call) in calls.iter().enumerate() {
            let name = call["function"]["name"].as_str().unwrap_or("");
            if name != "read_file" {
                continue;
            }
            let args: Value = call["function"]["arguments"]
                .as_str()
                .and_then(|a| serde_json::from_str(a).ok())
                .unwrap_or(Value::Null);
            if let Some(refused) = engine.guarded(name, &args) {
                results[i] = Some(refused);
            } else {
                reads.push((i, args));
            }
        }
        let mut read_outs: Vec<(usize, String)> = Vec::new();
        std::thread::scope(|s| {
            let mut handles: Vec<(usize, std::thread::ScopedJoinHandle<'_, String>)> =
                Vec::with_capacity(reads.len());
            for (i, args) in &reads {
                let args = args.clone();
                handles.push((
                    *i,
                    s.spawn(move || {
                        let sb: Sandbox = &|argv: &[String]| worlds.sandbox_work(world, argv);
                        match tools::execute(sb, "read_file", &args) {
                            Ok(out) => out,
                            Err(e) => format!("error: {e}"),
                        }
                    }),
                ));
            }
            for (i, h) in handles {
                let out = h
                    .join()
                    .unwrap_or_else(|_| "error: the read's thread died".to_string());
                read_outs.push((i, out));
            }
        });
        for (k, (i, out)) in read_outs.into_iter().enumerate() {
            if out.starts_with("error:") {
                engine.think = true;
            }
            let path = reads[k].1["path"]
                .as_str()
                .unwrap_or("")
                .trim_start_matches("./")
                .to_string();
            if !path.is_empty() {
                engine.read.insert(path);
            }
            results[i] = Some(out);
        }
        for (i, call) in calls.iter().enumerate() {
            if results[i].is_some() {
                continue;
            }
            results[i] = Some(engine.tool(&sandbox, call));
        }
        for (i, call) in calls.iter().enumerate() {
            let mut result = results[i].take().unwrap_or_default();
            let name = call["function"]["name"].as_str().unwrap_or("");
            let args: Value = call["function"]["arguments"]
                .as_str()
                .and_then(|a| serde_json::from_str(a).ok())
                .unwrap_or(Value::Null);
            let shown = format!("{name} {}", short(&args));
            // `timeout` exits 124 (TERM) or 137 (KILL after the grace period).
            if name == "shell"
                && (result.starts_with("exit 124\n") || result.starts_with("exit 137\n"))
            {
                engine.watch.timeouts += 1;
                if engine.watch.timeouts >= TIMEOUT_STOP {
                    log.push(
                        json!({"role": "tool", "tool_call_id": call["id"], "content": result}),
                    );
                    let why = format!(
                        "stuck: a second command hung and was killed after {} s: {shown}",
                        tools::COMMAND_TIMEOUT_SECS
                    );
                    return end(engine, log, step, None, Some(why), None);
                }
                result = format!(
                    "timed out after {} s and was killed. Do not run it again as is: bound it \
                     (smaller input, `timeout`, no watch mode, no prompt) or do without it.\n{result}",
                    tools::COMMAND_TIMEOUT_SECS
                );
            }
            // `args` is already parsed and written back: canonical.
            let repeated = engine.watch.call(&args.to_string(), &result);
            log.push(json!({"role": "tool", "tool_call_id": call["id"], "content": result}));
            match repeated {
                Some(true) => {
                    let why = format!(
                        "stuck: the same call gave the same result {REPEAT_STOP} times in a row: {shown}"
                    );
                    return end(engine, log, step, None, Some(why), None);
                }
                Some(false) => log.push(harness(&format!(
                    "You have made this exact call {REPEAT_NUDGE} times with the same result: {shown}. \
                     Repeating it will not change anything. Try a different approach, or call `done` \
                     and say what blocks you."
                ))),
                None => {}
            }
        }
        // A reply cut with tools in it: the tools ran, the reply did not
        // finish — the model is told to write smaller, free of charge.
        if engine.last_cut && engine.continue_cut().is_some() {
            log.push(harness(
                "Your reply was cut by the length limit with tools in it. Write in smaller \
                 chunks: one step at a time, smaller edits, shorter messages.",
            ));
            continue;
        }
        match engine.watch.step(worlds.status(world).unwrap_or_default()) {
            Some(Stuck::Stop) => {
                let why = format!("stuck: no file changed in {} steps", engine.watch.idle_stop);
                return end(engine, log, step, None, Some(why), None);
            }
            Some(Stuck::Nudge(text)) => log.push(harness(&text)),
            None => {}
        }
    }
    let why = format!("stopped after {max_steps} steps without calling `done`");
    end(engine, log, step.min(max_steps), None, Some(why), None)
}

/// The run's own log, written atomically after every model call: the whole
/// conversation, the criteria, the reads. A run that crashes resumes from it
/// ([`resume`]); a write that fails never stops the work.
fn write_run_log(
    path: &std::path::Path,
    world: &World,
    goal: &str,
    step: usize,
    from: usize,
    engine: &Engine,
    messages: &[Value],
) {
    if let Some(parent) = path.parent()
        && std::fs::create_dir_all(parent).is_err()
    {
        return;
    }
    let now = json!({"world": world.id, "goal": goal, "steps": step, "from": from,
                     "log": messages, "criteria": engine.criteria, "read": engine.read_set()});
    let tmp = path.with_extension("tmp");
    if std::fs::write(&tmp, now.to_string()).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
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
         If an answer would unblock you and the answer is not in the code, the goal, or the \
         request (a product decision the goal does not cover, a requirement not stated, a \
         command or tool that hangs), ask it with the context needed to answer it in one line. \
         If the answer is in the code or in the goal, do not ask. \
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

/// A call in one line for a reason the human reads.
pub(crate) fn short(args: &Value) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::path::Path;
    use std::process::Command;
    use std::sync::Mutex;

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
    fn idle_steps_nudge_twice_then_stop() {
        let mut w = Watch::default();
        let verdicts: Vec<_> = (0..w.idle_stop).map(|_| w.step(String::new())).collect();
        assert!(matches!(verdicts[w.idle_nudge - 1], Some(Stuck::Nudge(_))));
        assert!(matches!(verdicts[w.idle_hard - 1], Some(Stuck::Nudge(_))));
        assert_eq!(
            verdicts
                .iter()
                .filter(|v| matches!(v, Some(Stuck::Nudge(_))))
                .count(),
            2
        );
        assert!(matches!(verdicts[w.idle_stop - 1], Some(Stuck::Stop)));
        // A changed file resets the count and both notices.
        let mut w = Watch::default();
        (0..w.idle_hard).for_each(|_| _ = w.step(String::new()));
        assert_eq!(w.step(" M a".into()), None);
        assert!(!w.nudged_idle && !w.nudged_idle_hard);
    }

    #[test]
    fn a_framing_watch_nudges_later_and_stops_later() {
        let mut w = Watch::framing();
        assert!((15, 20, 25) == (w.idle_nudge, w.idle_hard, w.idle_stop));
        let verdicts: Vec<_> = (0..w.idle_stop).map(|_| w.step(String::new())).collect();
        assert!(matches!(verdicts[w.idle_nudge - 1], Some(Stuck::Nudge(_))));
        assert!(matches!(verdicts[w.idle_hard - 1], Some(Stuck::Nudge(_))));
        assert_eq!(
            verdicts
                .iter()
                .filter(|v| matches!(v, Some(Stuck::Nudge(_))))
                .count(),
            2
        );
        assert!(matches!(verdicts[w.idle_stop - 1], Some(Stuck::Stop)));
    }

    #[test]
    fn a_different_result_is_not_a_repeat() {
        let mut w = Watch::default();
        for n in 0..REPEAT_STOP {
            assert_eq!(w.call("cargo test", &format!("exit 1 failure {n}")), None);
        }
    }

    struct Silent;
    impl Model for Silent {
        fn chat(&self, _: &[Value], _: &Value) -> io::Result<Value> {
            Ok(json!({"content": "notes"}))
        }
        fn label(&self) -> String {
            "silent".into()
        }
        fn variant<'s>(&'s self, _: &str) -> Box<dyn Model + 's> {
            Box::new(Silent)
        }
    }

    fn call(name: &str, args: Value) -> Value {
        json!({"id": "c", "function": {"name": name, "arguments": args.to_string()}})
    }

    #[test]
    fn criteria_are_checked_and_done_waits_for_them() {
        let dir = std::env::temp_dir().join(format!("fennec-os-engine-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sandbox = |argv: &[String]| {
            let mut c = Command::new(&argv[0]);
            c.args(&argv[1..]).current_dir(&dir);
            Ok(c)
        };
        let mut e = Engine::new(&Silent, vec![criterion("do not touch x", None)]);
        let added = e.tool(
            &sandbox,
            &call(
                "criteria",
                json!({"action": "add", "text": "f exists", "check": "test -f f"}),
            ),
        );
        assert!(added.contains("2. [ ] f exists"), "{added}");
        assert!(
            e.finish(&sandbox).is_some(),
            "an open criterion refuses done"
        );
        let failed = e.tool(
            &sandbox,
            &call("criteria", json!({"action": "done", "number": 2})),
        );
        assert!(failed.contains("not met"), "{failed}");
        // An edit needs a read first; a write counts as one.
        let edit = json!({"path": "f", "old_string": "a", "new_string": "b"});
        assert!(
            e.tool(&sandbox, &call("edit_file", edit.clone()))
                .contains("read f")
        );
        e.tool(
            &sandbox,
            &call("write_file", json!({"path": "f", "content": "a"})),
        );
        assert!(
            e.tool(&sandbox, &call("edit_file", edit))
                .contains("edited")
        );
        assert!(e.finish(&sandbox).is_none(), "the check now passes");
        assert_eq!(e.criteria[1]["state"], "passed");
        let reminder = e.reminder(9, 10, "Build.");
        let text = reminder["content"].as_str().unwrap();
        assert!(
            text.contains("call `done`") && text.contains("Few steps"),
            "{text}"
        );
        let bad = json!({"id": "c", "function": {"name": "shell", "arguments": "{oops"}});
        assert!(e.tool(&sandbox, &bad).contains("not valid JSON"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_direct_run_must_add_criteria_before_done() {
        let dir = std::env::temp_dir().join(format!("fennec-os-criteria-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sandbox = |argv: &[String]| {
            let mut c = Command::new(&argv[0]);
            c.args(&argv[1..]).current_dir(&dir);
            Ok(c)
        };
        // A direct run says what good looks like before it may finish.
        let mut e = Engine::new(&Silent, Vec::new());
        e.require_criteria = true;
        let refused = e.finish(&sandbox).unwrap();
        assert!(refused.contains("no criterion"), "{refused}");
        let _ = e.finish(&sandbox);
        assert!(
            e.finish(&sandbox).is_none(),
            "two refusals, then it goes through"
        );
        // A role, steered by its brief, is not asked for the list.
        let mut e = Engine::new(&Silent, Vec::new());
        assert!(e.finish(&sandbox).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_read_only_role_is_refused_by_the_system_not_the_brief() {
        // The neutral-brief lock: the agent is told only "do what you are
        // told"; the tools line, not the prompt, keeps its write off.
        let dir = std::env::temp_dir().join(format!("fennec-os-readonly-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.txt"), "here").unwrap();
        std::fs::create_dir_all(dir.join("relay")).unwrap();
        let sandbox = |argv: &[String]| {
            let mut c = Command::new(&argv[0]);
            c.args(&argv[1..]).current_dir(&dir);
            Ok(c)
        };
        let read = crate::team::Tool {
            name: "read".into(),
            prefix: None,
        };
        let write = crate::team::Tool {
            name: "write".into(),
            prefix: Some("relay/".into()),
        };
        let mut e = Engine::new(&Silent, Vec::new());
        e.tools = Some(vec![read.clone(), write.clone()]);
        let out = e.tool(
            &sandbox,
            &call("write_file", json!({"path": "new.txt", "content": "x"})),
        );
        assert!(out.contains("refused"), "{out}");
        assert!(
            !dir.join("new.txt").exists(),
            "the system refused, not the brief"
        );
        // Its own relay path is the one it may write.
        let out = e.tool(
            &sandbox,
            &call(
                "write_file",
                json!({"path": "relay/review.md", "content": "verdict"}),
            ),
        );
        assert!(dir.join("relay").join("review.md").exists(), "{out}");
        // `edit` is not on its list: the system refuses it too.
        assert!(
            e.tool(
                &sandbox,
                &call("edit_file", json!({"path": "relay/review.md", "old_string": "verdict", "new_string": "verdict: valide"}))
            )
            .contains("refused")
        );
        let out = e.tool(&sandbox, &call("shell", json!({"command": "ls"})));
        assert!(
            out.contains("refused"),
            "the role lists only read and write: {out}"
        );
        // Unrestricted, the same call runs.
        let mut e = Engine::new(&Silent, Vec::new());
        e.tool(
            &sandbox,
            &call("write_file", json!({"path": "free.txt", "content": "x"})),
        );
        assert!(dir.join("free.txt").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_persons_rules_refuse_ask_or_record() {
        let ran = std::sync::atomic::AtomicUsize::new(0);
        let sandbox = |_: &[String]| {
            ran.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(Command::new("true"))
        };
        let rules = vec![
            json!({"effect": "deny", "on": "command", "pattern": "rm -fr /", "why": "root"}),
            json!({"effect": "ask", "on": "read", "pattern": "**/.env", "why": "secrets"}),
        ];
        let guard = || Some(Guard::new(rules.clone(), Path::new("/p"), Path::new("/h")));
        let shell = |c: &str| call("shell", json!({"command": c}));
        // A task: nobody to ask, so refused and recorded.
        let mut e = Engine::new(&Silent, Vec::new());
        e.guard = guard();
        assert!(e.tool(&sandbox, &shell("rm -r -f /")).contains("refused"));
        assert!(e.tool(&sandbox, &shell("cat .env")).contains("nobody can"));
        assert_eq!(e.asked.len(), 1);
        assert_eq!(
            ran.load(std::sync::atomic::Ordering::Relaxed),
            0,
            "nothing ran"
        );
        // A chat: asked; allowed for the chat, it is not asked again.
        let asks = std::cell::Cell::new(0);
        let asker = |_: Value| {
            asks.set(asks.get() + 1);
            Ok(Some(Answer::Chat))
        };
        let mut e = Engine::new(&Silent, Vec::new());
        e.guard = guard();
        e.asker = Some(&asker);
        e.tool(&sandbox, &shell("cat .env"));
        e.tool(&sandbox, &call("read_file", json!({"path": ".env"})));
        assert_eq!(
            (asks.get(), ran.load(std::sync::atomic::Ordering::Relaxed)),
            (1, 2)
        );
    }

    #[test]
    fn done_goes_through_after_two_refusals() {
        let sandbox = |_: &[String]| Ok(Command::new("false"));
        let mut e = Engine::new(&Silent, vec![step_criterion("never done")]);
        assert!(e.finish(&sandbox).is_some());
        assert!(e.finish(&sandbox).is_some());
        assert!(e.finish(&sandbox).is_none());
    }

    #[test]
    fn a_large_context_is_compacted_into_the_models_notes() {
        struct Big(Mutex<VecDeque<Value>>);
        impl Model for Big {
            fn chat(&self, _: &[Value], _: &Value) -> io::Result<Value> {
                Ok(self.0.lock().unwrap().pop_front().unwrap())
            }
            fn label(&self) -> String {
                "big".into()
            }
            fn variant<'s>(&'s self, _: &str) -> Box<dyn Model + 's> {
                Box::new(Big(Mutex::new(VecDeque::new())))
            }
        }
        let model = Big(Mutex::new(VecDeque::from([
            json!({"content": "hi", "usage": {"prompt_tokens": 1_000_000}}),
            json!({"content": "my notes"}),
        ])));
        let mut e = Engine::new(&model, Vec::new());
        e.ask(&[json!({"role": "user", "content": "go"})], &json!([]))
            .unwrap();
        assert!(e.needs_compaction());
        assert_eq!(e.compact(&[]).unwrap(), "my notes");
        assert!(!e.needs_compaction());
        assert_eq!(e.turns.len(), 2);
    }
}
