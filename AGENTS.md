# AGENTS.md — fennec-os

## What this repository is

A local cognitive OS for coding agents. Its primitive is the **world**: a
reversible, forkable sandbox an agent runs in. Everything else (parallel
attempts, deterministic verification, memory, sleep) plugs into worlds.
Roadmap: `docs/roadmap.md`. Origin (French R&D transcripts, reasoning record,
not binding): `docs/history/`; synthesis note in the `iatraining` repo,
`notes/agent-architecture-reflexion.md`.

## Layout

```
src/world.rs     worlds: fork, sandbox (bubblewrap), diff, commit, rollback, drop
src/provider.rs  `Model` trait + OpenAI-compatible client
src/recipe.rs    replay a landed run's tool calls when the same goal returns
src/agent.rs     tool loop; every tool runs inside the sandbox
src/solve.rs     task → N worlds → verify → select → land (rebase + re-verify)
src/spec.rs      request → acceptance tests + task (intent compiler), human-validated
src/rules.rs     project rules (`.fennec-os/rules.json` at HEAD), glob matching
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
fennec-os serve [--port N]  # prints a one-time URL; the review page
```

Env: `FENNEC_OS_HOME` (store, default `~/.local/share/fennec-os`),
`FENNEC_OS_RO` (paths re-exposed read-only inside the hidden home),
`FENNEC_OS_BASE_URL`, `FENNEC_OS_MODELS` (comma-separated), `FENNEC_OS_API_KEY`.

Tests never call a real model: `tests/solve.rs` implements `Model` with
scripted fakes (`Script`, `Spy`, `Distiller`), and `tests/common` builds a
temp git project, fake home and world store. Store layout: `worlds/<id>` is a
git worktree on branch `world/<id>`; `meta/<id>` records its git dir.

## Non-negotiable rules

1. **Nothing leaves a world except through the harness.** The sandbox has no
   network, a hidden home, a private /tmp, and a read-only view of the
   project's git store. Never widen a default.
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
