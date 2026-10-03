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
Not yet: prompt diversity;
cgroup limits; network-needing builds (deps must be in the local cache).

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
Not yet: adversarial agent proposing rules; telling the distiller what is
already known.

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

**P7 intent compiler v0** done (not yet verified live: the model server was
unreachable): `spec <project> <request>` runs an agent that writes acceptance
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
