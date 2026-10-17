# AGENTS.md — fennec-lead

## What this repository is

The lead layer of fennec: a developer leads one or several projects the way
they would lead a team. They state a request, validate "done when" once, leave,
and come back to a single view of what needs them. Execution lives in
`../fennec-os` (library); this repository only adds the human surface.
Scenarios: `../lead-scenarios.md`. Inventory and decisions: `../inventory.md`.

## Layout

```
src/main.rs   `fennec` binary: std HTTP server, routes, folder browser
src/state.rs  requests and projects in files; the single view (what needs you, running, landed)
src/work.rs   request life in the background: fennec-os spec, answer, solve, retry, land; rule offers
ui/           index.html + app.js, embedded in the binary
```

## Build, test, run

```bash
cargo build
cargo test
cargo clippy --all-targets  # must be clean
cargo fmt --all
fennec                      # http://127.0.0.1:7777
```

Env: `FENNEC_LEAD_HOME` (default `~/.local/share/fennec-lead`), `FENNEC_LEAD_ADDR`
(default `127.0.0.1:7777`), `FENNEC_LEAD_ATTEMPTS` (default 1), plus fennec-os's
`FENNEC_OS_*` (model endpoint and names are required to frame a request).

## Non-negotiable rules

1. **Every human interaction goes through the web UI.** No CLI or TUI surface
   for the user; `fennec-os`'s CLI is a debug tool only.
2. **Seven words on screen**: project, request, done when, proof, question, rule,
   team (made of roles; added 2026-10-17, see `docs/design/team.md`). Internal
   concepts (world, run, grant, permissions, limits, stage keys) never reach the UI.
3. **Nothing is "ready" without proof.** What could not be proven is shown as
   "not verified: X".
4. **Execution logic goes in fennec-os**, not here. This repository stays thin.
5. **No database.** State lives in files, as in fennec-os.
6. No new dependency without a stated reason in the commit body. Stdlib first.

## Style

Rust 2024, rustfmt defaults, clippy clean. UI: plain JS, no build step, served
by the binary. English for code, comments and docs; French for UI text.
Comments explain *why*.

## Commit format

Conventional Commits, imperative mood (`feat:`, `fix:`, `docs:`, `refactor:`,
`test:`, `chore:`).
