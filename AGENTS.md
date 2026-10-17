# AGENTS.md — fennec-os

## What this repository is

A local cognitive OS for coding agents. Its primitive is the **world**: a
reversible, forkable sandbox an agent runs in. Everything else (repeated
attempts, deterministic verification, memory, sleep) plugs into worlds.
Roadmap: `docs/roadmap.md`. Origin (French R&D transcripts, reasoning record,
not binding): `docs/history/`; synthesis note in the `iatraining` repo,
`notes/agent-architecture-reflexion.md`.

## Layout

```
src/world.rs     worlds: fork, sandbox (bubblewrap), diff, commit, rollback, drop
src/grant.rs     human-issued grant: a world writes named host paths, backed up until keep/revert
src/provider.rs  `Model` trait + OpenAI-compatible client
src/recipe.rs    replay a landed run's tool calls when the same goal returns
src/agent.rs     tool loop; every tool runs inside the sandbox; loops, idle runs and hung commands are stopped
src/solve.rs     task → worlds in turn until one passes verify → land (rebase + re-verify, one commit by the human) on the branch the rules ask for → push
src/spec.rs      request → acceptance tests + task (intent compiler), human-validated; retry from the tests; sparring findings
src/proof.rs     a ready run used as its user would: model proposes tries, pages and clicks/typing; harness runs them, captures before/after (proof.py)
src/rules.rs     project rules (`.fennec-os/rules.json` at HEAD), glob matching; rules said in words
                 (`~/.config/fennec/rules.md`, AGENTS.md `## Rules`): proposed, written once, given to agents
src/team.rs      named teams of roles that brief the agent per stage (frame, solve, prove):
                 `~/.config/fennec/teams/<team>/*.md` or built-in teams (`Développement`, `Études`) drawn
                 from `catalog/`; a project picks one in `.fennec-os/team.json`, overrides in `.fennec-os/team/`
                 a `prove` role's `check:` lines run on every proof: a failure is a veto
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
`FENNEC_OS_RO` (paths re-exposed read-only inside the hidden home),
`FENNEC_OS_MEMORY_MB` (address-space cap per sandboxed command, default 8192, 0 = none),
`FENNEC_OS_BASE_URL`, `FENNEC_OS_MODELS` (comma-separated; default: what the server lists), `FENNEC_OS_API_KEY`,
`FENNEC_OS_BROWSER` (headless browser for proof screenshots, default `chromium`).

Tests never call a real model: `tests/solve.rs` implements `Model` with
scripted fakes (`Script`, `Spy`, `Distiller`), and `tests/common` builds a
temp git project, fake home and world store. Store layout: `worlds/<id>` is a
git worktree on branch `world/<id>`; `meta/<id>` records its git dir.

## Non-negotiable rules

1. **Nothing leaves a world except through the harness.** The sandbox has no
   network, a hidden home, a private /tmp, and a read-only view of the
   project's git store. Never widen a default. The one exception is a
   `grant` issued by the human (files inside the home only, backed up first).
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
