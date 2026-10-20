# Roadmap

Spine first: a reversible world, N parallel attempts, deterministic
verification. Everything after plugs into it. From P2 on, the tool is usable
and shareable on its own.

| Phase | Deliverable | Done when |
|---|---|---|
| **P0 World** | Sandbox (bubblewrap) + git worktree; `fork`, `commit`, `rollback`, `diff`, `drop`. Btrfs snapshots for non-git projects | A command in a world has no network, cannot write outside it, and `rollback` restores the exact state |
| **P1 One agent, one verification** | An agent loop inside a world; its output is the world diff plus a `verify`; deterministic checks; landing a world onto the project | A task reaches land or discard with no permission list |
| **P2 N worlds** | N forks in parallel with diversity knobs (model, prompt, temperature); selection by the verifier | N attempts in about the wall time of one on local hardware |
| **P3 Queue and promotion** | Ported from fennec-harness: queue on failure or disagreement; correction → rule | A correction given twice proposes a rule |
| **P4 Agenda and sidekick v0** | Per-project agenda; a small model that summarises tool output and injects what is relevant | Resume without re-briefing; context stays bounded over a long session |
| **P5 Sleep v0** | Nightly job: winning trajectories → candidate skills; adversarial agent → candidate rules; a human promotes | The morning queue holds proposals from the night |
| **P6 Scheduler** | Priorities: interactive, background, sleep; background yields when the user works | Sleep and agents run without slowing interactive use |
| **P7 Research** | Automatic trajectory compilation (JIT), speculative tool execution, nightly LoRA, intent compiler, home cluster | Each idea kept or dropped |

Watch points:

- P0 is Linux-only (namespaces). macOS (`sandbox-exec`) comes later.
- Later, maybe (the human, 2026-10-20): a project's devcontainer as an
  option, set per project ("tools: the machine's / the devcontainer's").
  When the project has `.devcontainer/` and Docker runs, the agent works
  inside it: its toolchain comes from the project, not from what the machine
  has, and Docker also covers macOS and Windows. Offered for the tools, not
  as more safety: the isolation is the same namespaces as bubblewrap, and
  the person's rules on commands and files apply as they are. Costs: Docker
  installed and running, images of several GB per project, a VM on macOS and
  Windows. Until then the sandbox uses the machine's toolchains, named to
  the agent, with package caches in the project's `.fennec/`.
- P2 is only as good as its verifiers: weak criteria turn N attempts into a
  random pick. The intent compiler (P7) may need to move earlier.
- Worlds that cannot be forked (external effects, live state, hardware) need
  apply-with-timed-rollback instead.

## Status

**P0** done: git-backed worlds, sandbox, fork from a project or a world,
checkpoint, rollback, drop, CLI, integration tests. Not yet: btrfs snapshots
for non-git projects.

**P1 + P2** done: `solve` forks N worlds, runs one agent per world in parallel
(diversity: models × temperatures), verifies with the task's commands inside
the sandbox, keeps the smallest passing diff, drops the rest; `land`
fast-forwards the project. Integrity: `protect` paths fail an attempt that
touches them; a verify that changes the world fails; a world changed after
its verdict cannot land. Run records in `<store>/runs/`.
Live: 3 attempts of qwen3.8-27b on a Rust task, 3 PASS in 5.4 s wall time.
fennec-harness `harness-checks` was not reused: it pulls `harness-core`
(SQLite, tokio).
`land` rebases a world onto a project that moved and verifies again.
Security: the harness used to read the world's `.git` file (agent-writable)
to decide what to mount read-write, so an agent could get any host git store
mounted (reproduced: a hook planted in another repository). Git dirs now come
from harness metadata, the admin dir is read-only, hooks are off.
Memory: every sandboxed command runs under `ulimit -d` (default 8 GiB,
`FENNEC_OS_MEMORY_MB`); writable memory per process only (`-v` killed
Chromium, which reserves over 64 GiB), no CPU, pid or disk caps.
Not yet: prompt diversity;
cgroup limits; network-needing builds (deps must be in the local cache).

**Grant v0** done (files only): `grant give <world> <path>...` binds host paths
read-write in that world's sandbox after backing them up (`cp -a`, reflink on
Btrfs); `grant diff` shows what changed, `grant keep` deletes the backup,
`grant revert` restores it. Paths must be inside the home, never the home or
the store; one grant per world; a world with a grant cannot be dropped.
Limits: a config file the agent edits runs later *outside* the sandbox, so the
diff must be read before `keep`. Not yet: sockets and commands (Hyprland,
D-Bus); irreversible effects; an expiry.
Pending grants are surfaced, never decided: `grant list`, a one-line reminder
on stderr from any command, `grant remind` for a desktop notification (timer
in `docs/grant-reminder.md`). A `give` killed before its record is cleared by
the next `give`.
`solve <project> task.json --grant <path>` (repeatable) gives the one attempt
those paths: exactly one model, no recipe replay, the paths are named in the
agent's prompt. Pass → the host stays changed and the world waits for
`grant diff`, then `grant keep` or `grant revert`. Fail → the harness reverts
the host itself. The verdict is still the task's `verify` commands, which see
the host paths; host changes are not judged by `protect` or path rules.

**P3** done: project rules (`forbidden_path`, `paired_change`,
`standing_command`; `block` or `warn`) read from `.fennec-os/rules.json` at the
project's HEAD; the rules file is implicitly forbidden to runs. `queue`,
`show`, `reject <run> [correction]` (counts the same correction per project),
`promote` (writes the rule with its origin; the human commits it).
Ceiling: "the same correction" is exact after normalisation; paraphrases need
a model to propose groupings.

**P4 v0** done, without a second model: the agenda is built from this
project's run records (landed + agent summary, rejected + correction, failed +
reasons; latest 12, one line each) and goes into every new agent's system
prompt; promoted rules go in as criteria. Context is bounded by eliding old
tool output and old file contents past 48 KB, down to half at once so prefix
cache invalidations stay rare.
Live: a rejection ("test invalid inputs like IIII and empty string") reached
the next run's prompt and its tests covered both.
Not yet: a sidekick model that summarises instead of eliding; agenda entries
written by the human.

**P5 v0** done (know-how notes; replayable recipes are P7): `sleep` distils
each landed run's winning trajectory (commands, first line of each result,
summary) into at most 3 candidate facts; `notes`, `keep`, `dismiss`. Kept
facts go to `.fennec-os/knowhow.md`, read from HEAD into every system prompt.
The whole `.fennec-os/` dir is forbidden to runs. Dismissed candidates are
never proposed again (exact match after normalisation).
Live: one run gave 3 candidates in 8 s; 2 kept (the working test command, a
parsing pitfall), 1 dismissed (restated a rule already injected).
Not yet: telling the distiller what is already known.

**P5 sparring v0** done: `spar <project>` runs a background agent that hunts
one defect and proves it with a failing test in a new file. A finding is a
spec (`origin: spar`): same harness checks, same review page, accept to have
it fixed, reject with a reason. Earlier findings and their verdicts go into
the next round so nothing is reported twice. Its model yields to interactive
work before every turn, not only at the start.
Live: started during a 27-minute solve, it waited the whole time without
touching the GPU, then took 10 turns (about 30 s) and answered `none` on a
correct `to_roman`; honest, not yet a finding.
Not yet: findings turned into rules rather than tests; a nightly schedule
(cron calls `spar`).

**P6 v0** done (fennec-os's own load only): `solve` holds a shared lock on
`<store>/interactive.lock` for its whole run; `sleep` takes the exclusive lock
before each distillation, so it waits while any `solve` runs. Locks are
advisory `flock`s: they die with the process, no stale state.
Not yet: yielding to other clients of the model server (the current server
answers 404 on `/metrics`, so its load is not visible); yielding to the
user at the keyboard; preempting a distillation already sent.

**P7 recipes v0** done: when the latest run on the same goal (after
normalisation) in the same project landed, its winning tool calls are replayed
as an attempt of their own, before any model. Pass → no model call. Fail →
the models run as usual. A rejected or failed latest run retires the recipe;
a trajectory whose written files were elided by context bounding is not
replayed.
Live: from_roman reverted, same task again → the recipe replayed 7 steps,
`cargo test` passed, 0.3 s and no model call (71 s with the model).
Limits: exact goal match only; write_file replays old contents, so recipes
pay off for command-shaped tasks (regenerate, bump, migrate) and for redoing
work that was undone, less for edits on moved code (the checks catch it).
Not yet: matching by intent; parametrised recipes.

**P7 intent compiler v0** done: `spec <project> <request>` runs an agent that writes acceptance
tests only and ends with `goal:` and `verify:` lines. The harness then checks
what it can decide: a goal and a command were given, files were written, none
under `.fennec-os` or a forbidden path, and the commands **fail** now (tests
that already pass do not test the request). A valid spec keeps its world for
the human: `spec show`, `spec accept` (the attempts fork from the spec's world,
its files are protected, its commands are the verify), `spec reject` with a
correction that reaches the next compile of the same request.
Limits: "fails" means any non-zero exit, so tests that do not compile pass
the check; the human reading the diff is the guard. The spec agent may write
non-test files; protecting them then blocks the implementation.
Not yet: clarifying questions before writing; recipes matched through the
compiled goal; a review surface richer than the CLI (no TUI: decision open).

**Review surface v0** done: `serve` prints a loopback URL carrying a secret
for this start; opening it sets a `SameSite=Strict` cookie. The page lists
specs to validate, runs waiting, know-how candidates, and takes a new request.
A spec page shows request, goal, verify, checks and tests, and offers: ask
the compiler a question (advice only, stored as a thread on the spec), accept
and solve, reject, reject and recompile with the correction. A run page shows
attempts, checks, rule violations and the winning diff, with land and reject.
Every decision runs as a job in a self-refreshing frame, so a form being
typed is never wiped. Guards: loopback bind, Host check (DNS rebinding),
cookie on every request, CSP without any script, all output escaped: a diff
is written by an agent and must not act through the page. Stdlib HTTP, no
new dependency. `Worlds::open` now refuses non-hex ids (`..` named the store).
Not yet: promoting a rule from the page (the CLI hint is shown); questions on
runs; streaming an agent's steps while it works; jobs survive only the process.

**Live pass, 2026-10-03** (qwen3.8-27b, the full loop through the review
page: request, spec, question, reject and recompile, accept, solve, land).
What it found and changed:
- Rust keeps unit tests in the file they test. Protecting the spec's files
  then forbade the implementation: 3 attempts, 27 minutes, all failed on
  `changed src/lib.rs`. Spec tests now go in new files (checked by the
  harness), which are protected whole. With that: 2 attempts, both passed,
  84 s, landed. The "only add" check for sparring became redundant: removed.
- qwen ended 3 of 5 specs with prose instead of `goal:`/`verify:`. The
  harness now asks once more for the form, without tools, and still parses
  and checks the answer; the raw summary is kept. The parser also accepts
  `**Goal:**`, `- verify:` and commands on the following lines.
- `solve` waited for its slowest attempt. After the first pass, the others
  now get as long again as it took, then stop at their next turn.
- On one GPU already at 99 % with one model, N attempts are not "the wall
  time of one": they share decode speed. Two attempts is the right default
  on this hardware.
- Records were written in place; the page could read a half-written one.
  Run, spec and candidate records are now written aside and renamed.
- The compiler answered a question about its tests in 3 s, correctly, and
  said it could not change files itself.

**Profiling v0** done: every agent turn is timed (start offset, model time,
each tool's time) with the server's token usage (prompt, cached, output,
reasoning); every check is timed; every attempt has its wall time and start.
A stopped or failed attempt keeps its turns. `profile <run|spec>` splits
the wall time into model, tools, verify and harness, with a table per turn;
the review page shows the split per attempt.
First live profile (`is_roman`, 2 attempts, both passed in 17 s): the model
is 97 % of the wall time; tools 2 %, verify and harness under 1 %. Turns
cost 1 to 5 s each; most are reads (`cat`, `ls`) that run in 10 ms and still
pay a full model turn. Reasoning was a quarter of the output tokens, and
78 % of the prompt was served from the prefix cache. Output speed floor:
51 to 59 tok/s with two agents sharing the GPU. The lever is the number of
turns, not the tools or the harness.

**P2 revised: attempts run in turn.** Measured on this server (ninfer,
speculative decoding `dflash2`): one request decodes at 165 tok/s; two
decoding at once get 90 to 98 each, about 188 together. Speculative decoding
already uses the compute a single request leaves free, so parallel attempts
split the speed instead of sharing spare capacity, and attempts of one run
start together and decode together: the worst case. Several agents on
different projects overlap far less (the server's log shows 176 to 203
tok/s per request under its usual load), which is why they feel fast.
`solve` now runs attempts one after the other, forks each world when its
attempt starts, and stops at the first pass; the grace deadline for
stragglers and smallest-diff selection are gone with the parallelism.
Live, same `is_roman` task: 7 s and one attempt, against 17 s for two
parallel attempts; output floor 202 tok/s against 51 to 59.
Not yet: parallel attempts behind a flag for a server that batches for
free; reading the server's `requests.jsonl` to split model time into queue,
prefill, decode.

**Lessons between attempts** done: a failed attempt's lesson goes to the
next one: the harness's reason, the files it changed, and the tail of each
failing blocking check's output; not the agent's own account. Each attempt
still starts from a fresh world. The lessons go in the goal message, so the
system prompt, and the server's prefix cache, stay the same across attempts.
Live: not exercised yet. On a task with hidden protected tests, the first
attempt read them and passed in 11 s; forcing a failure would only test a
fake lesson.

**Fewer turns: tried and dropped.** Recorded runs open with two to four
turns of `ls` and `cat`, so the agent was given the tracked files and the
contents of the named and smallest files up front (16 KB budget). Measured
on `is_roman`, three runs each: turns fell from 8 to 5.3 on average, wall
time rose from 9.4 s to 16.7 s. Read turns cost 0.3 to 0.6 s each: little
output, and the input is prefilled fast and cached. With the whole file in
view, the agent rewrote it with `write_file` (2,000 to 3,000 output tokens,
12 s in one turn) instead of appending 250 tokens, and reasoned twice as
much. The cost is output tokens, decoded at about 165 tok/s, not turns.
In passing attempts, about 40 % of output tokens are reasoning and 13 % are
`write_file` contents; the rest are commands, arguments and summaries.
