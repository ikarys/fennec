# AGENTS.md — fennec-os

## What this repository is

**fennec's purpose** (the human, 2026-10-19): an assistant to build things with
AI **fast**, but **efficiently** and **safely**. Every feature is weighed against
these three; one that serves none of them is not built.

A local cognitive OS for coding agents. Its primitive is the **world**: a
reversible, forkable sandbox an agent runs in. Everything else (repeated
attempts, deterministic verification, memory, sleep) plugs into worlds.
Roadmap: `docs/roadmap.md`. Origin (French R&D transcripts, reasoning record,
not binding): `docs/history/`; synthesis note in the `iatraining` repo,
`notes/agent-architecture-reflexion.md`.

## Layout

```
src/world.rs     worlds: fork, sandbox (bubblewrap), diff, commit, rollback, drop; facts and forget for what a cut-short start or drop left;
                 park/unpark (work set aside gives the folder back); `Worlds::apart` forks apart whatever the project chose,
                 `Worlds::base` from a named branch when it exists
src/grant.rs     human-issued grant: a world writes named host paths, backed up until keep/revert
src/provider.rs  `Model` trait + OpenAI-compatible client; `ask` (one question), `Live` (the exchange so far, in a file)
src/recipe.rs    replay a landed run's tool calls when the same goal returns
src/chat.rs      a plain conversation in a folder the person chose (git or not): same sandbox, folder read-only until the person allows changes, skills; no world
src/outside.rs   the literal host paths a chat tool call names outside its folder: the chat asks, an opened folder stays read-write
src/agent.rs     the engine every agent runs on (task attempts, chat turns): criteria kept by the harness and checked
                 by commands, a reminder as the last message of every call, `done` refused while criteria are open,
                 compaction into the model's notes, cut replies continued; loops, idle runs and hung commands stopped
src/guard.rs     the person's rules on what agents run, read and write (`~/.config/fennec/guard.json`, never in a project;
                 defaults seeded once): deny, ask (a chat asks, a task does without and records it), allow; commands
                 split and normalized, paths resolved; checked by the engine before every call and every criterion check
src/tools.rs     the agent's tools, run in the sandbox: shell, read_file (numbered), edit_file (exact replace, read first), write_file
src/solve.rs     task → worlds in turn until one passes verify → land (rebase + re-verify, one commit by the human) on the branch the rules ask for → push
src/spec.rs      request → conversation (`discuss`) → `direct` work (no tests first, proof decides) or acceptance tests + task (`compile`, CLI and sparring); retry from the tests; amend a ready run from its work, or `fix` it from what its proof found; sparring findings
src/proof.rs     a ready run used as its user would: model proposes tries, pages and clicks/typing; harness runs them, captures before/after (proof.py)
                 `page`: the same product started for the human's browser (host network, a free port, 30 min)
src/rules.rs     project rules (`.fennec-os/rules.json` at HEAD), glob matching; rules said in words
                 (`~/.config/fennec/rules.md`, AGENTS.md `## Rules`): proposed, written once or taken back, given to agents;
                 AGENTS.md is committed alone where worlds go in the folder (`world::keep`), so it never holds it
src/team.rs      named teams of roles that brief the agent per stage (frame = the conversation and `compile`, solve, prove):
                 `~/.config/fennec/teams/<team>/*.md` or built-in teams (`Développement`, `Études`) drawn
                 from `catalog/`; a project picks one in `.fennec-os/team.json`, overrides in `.fennec-os/team/`
                 a `prove` role's `check:` lines run on every proof: a failure is a veto
                 a role's `model:` is the model its stage runs on instead of the settings'
src/skill.rs     skills (Agent Skills folders, `SKILL.md`) read where the person keeps them (~/.config/fennec/skills,
                 ~/.claude/skills and plugins, the project's .claude/skills); enabled once for every project
                 (`~/.config/fennec/skills.json`, the old `git config fennec.skill` migrated); listed to the agent,
                 mounted read-only at ~/.skills/<name>, traced from the reads
src/plugin.rs    plugins the person keeps (~/.config/fennec/plugins/<name>/plugin.json, enabled in plugins.json): run on the
                 host with an event on stdin (a question, work ready...), 10 s at most; `before_<step>` ones are hooks;
                 a `serve` program runs while enabled with `FENNEC_URL`/`FENNEC_TOKEN`, for the senders `allow` names
src/review.rs    review queue, rejections, corrections said twice, agenda
src/sleep.rs     landed runs → candidate know-how (`.fennec-os/knowhow.md`)
src/schedule.rs  background work (sleep) waits while interactive work (solve) runs
src/serve.rs     review page on 127.0.0.1: specs, runs, know-how; decisions run as jobs
src/main.rs      `fennec-os` CLI
tests/         integration tests against a real sandbox
docs/          roadmap and design notes
```

## Build, test, run

```bash
cargo build
cargo test                  # needs bwrap and unprivileged user namespaces
cargo test --test solve <name>   # one integration test; --lib for unit tests
cargo clippy --all-targets  # must be clean
cargo fmt --all
fennec-os fork <project> && fennec-os run <id> -- <cmd>
fennec-os solve <project> task.json --attempts 3 && fennec-os land <id>
fennec-os solve <project> task.json --grant <host path>   # one attempt; then grant diff|keep|revert <id>
fennec-os serve [--port N]  # prints a one-time URL; the review page
```

Env: `FENNEC_OS_HOME` (store, default `~/.local/share/fennec-os`),
`FENNEC_OS_RO` (paths re-exposed read-only inside the hidden home; default: the toolchains, `~/.cargo`, `~/.rustup`, `~/.local/bin`, pipx, uv Pythons, nvm),
`FENNEC_OS_MEMORY_MB` (memory cap per sandboxed command, `ulimit -d`, default 8192, 0 = none),
`FENNEC_OS_BASE_URL`, `FENNEC_OS_MODELS` (comma-separated; default: what the server lists), `FENNEC_OS_API_KEY`,
`FENNEC_OS_MAX_TOKENS` (a reply's budget, reasoning included; default 32768, 0 = the server's),
`FENNEC_OS_CONTEXT` (the model's context window in tokens, default 65536; a conversation past 70 % of it is compacted)
(each overrides `~/.config/fennec/model.json`, which fennec-lead's settings write),
`FENNEC_OS_BROWSER` (headless browser for proof screenshots, default `chromium`).

Tests never call a real model: `tests/solve.rs` implements `Model` with
scripted fakes (`Script`, `Spy`, `Distiller`), and `tests/common` builds a
temp git project, fake home and world store. Where a world lives is the
project's `git config fennec.workspace`: unset or `here` = the project folder
itself, checked out on branch `world/<id>` (one at a time, clean folder
required, landing moves the human's branch and checks it out again); `apart`
= a git worktree under `<parent>/<name>-fennec/`; anything else = that path.
`meta/<id>` records the git dir, the world's path, the project folder and the
branch to give back.

## Non-negotiable rules

1. **Nothing leaves a world except through the harness.** The sandbox has a
   hidden home, a private /tmp, and a read-only view of the project's git
   store. Never widen a default. Exceptions: a `grant` issued by the human
   (files inside the home only, backed up first), and the network for the
   agent's own work (`Worlds::sandbox_work`, chats), asked by the human on
   2026-10-20 so a project's dependencies can be installed; what judges the
   work (verify, proof) still runs without network.
2. **The agent never commits.** Checkpoints and landing are harness operations.
   The harness never takes a path or setting from a world's files (`.git`
   included): git dirs come from `<store>/meta/`, hooks are always off.
3. **The model never judges.** Verdicts come from code reading exit codes and
   files.
4. **No new dependency without a stated reason** in the commit body. Stdlib
   first.
5. Errors: typed `io::Error` in the library; no `unwrap()` outside tests.

## Style

Rust 2024, rustfmt defaults, clippy clean. English for code, comments and docs.
Comments explain *why*. Small modules, one responsibility each.

## Commit format

Conventional Commits, imperative mood (`feat:`, `fix:`, `docs:`, `refactor:`,
`test:`, `chore:`).
