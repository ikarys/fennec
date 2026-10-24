# fennec — architecture vision

Written by hand at step 1; fennec proposes and maintains it from step 2.
Every task's design is checked against it, and the AI review reads it.

## Shape

One Cargo workspace, two crates, a dependency in one direction only:

- `crates/fennec-os` — the library. No UI, no HTTP for the human.
- `crates/fennec-lead` — the `fennec` binary: a local web server on
  127.0.0.1 and its UI (plain HTML/JS in `ui/`, embedded, no build step).
  It holds no execution logic: what runs agents, worlds and checks lives in
  `fennec-os`.

## Trust zones

1. **The human** — the UI and the configuration (`~/.config/fennec/`). The
   human is never blocked, only shown what they did.
2. **The trusted kernel** — `fennec-os/src/kernel/`: the guard, grants,
   rules, zones, plugins, the review queue, worlds (the sandbox boundary and
   the git operations on it). It decides and never runs what a model wrote.
   Everything it judges by is read from the project's `HEAD` or from the
   human's configuration, never from a world.
3. **The untrusted zone** — whatever a model outputs. It runs only inside a
   world (bubblewrap, git worktree). `fennec-os/src/engine/` drives the
   models: agent loop, tools, gateway, framing, solve, proof, roles.

A verdict always comes from code (exit codes, files, glob matches), never
from a model. A model's words are shown as "model", code's checks as
"verified".

## Conventions

- State lives in files (JSON, Markdown), never in a database.
- Std first; a new dependency needs a stated reason in its commit.
- Errors are `io::Error` in the library; no `unwrap()` outside tests.
- UI texts are English keys with their French in `ui/fr.js`.
- Tests never call a real model: scripted fakes stand in for it.

## Where things go

- A new check, rule or boundary: `kernel/`, read at `HEAD`.
- A new model-driven step: `engine/`, its result judged by kernel code.
- A new human decision: a route in `fennec-lead/src/main.rs`, its logic in
  `work.rs` or `state.rs`, its card in `ui/app.js`.
