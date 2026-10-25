# AGENTS.md — fennec

## What this repository is

**fennec's thesis** (the human, 2026-10-24): code written by AI is abundant;
understanding, mastery and responsibility are rare. fennec tools what is rare.
It is for professional developers who work with AI and want to stay the
authors of their software: they frame the work, the agent loop runs the steps,
then waits for their review. Nothing is integrated without a human decision.

The full definition lives in the Notion page "Définition de Fennec (v1)".

**Approach since 2026-10-25**: the earlier code (fennec-os, fennec-lead) is
removed (git history and `~/workspace/fennec_project` keep it). fennec is
rebuilt as a **Rust port of openfox** (https://github.com/co-l/openfox, MIT),
**one component at a time**, each one working end to end before the next.
Ported from openfox commit `0694574d` (v2.0.161); see THIRD_PARTY.md.

Component 1, done: the agent loop on the command line.
Component 2, done: the web UI (session tabs, ⌘K palette, home, themes).
Next, one PR each: bwrap world for tool execution →
planner/builder modes + verifier → criteria → compaction → sub-agents →
fennec's own layer (human-only integration, provenance, review gate).

## Layout

Clean architecture, applied where it pays: dependencies point inward
(adapters → agent → domain). The agent loop only knows the ports in
`agent/ports.rs`; a port exists because it decouples a layer or has a
second implementation (a test double, the coming bwrap toolbox), not for
show. One responsibility per file; no catch-all module (`utils`, `misc`,
`common`).

```
crates/fennec/src/
  main.rs            composition root: command line → settings → web or cli adapter
  config.rs          settings from the environment (model endpoint, store path)
  domain/            the vocabulary, no I/O
    event.rs           openfox events/types: Event, ToolCall, ToolResult, Role, DoneReason
    project.rs         Project, SessionSummary, SessionState
    conversation.rs    openfox events/fold-messages: the log read back as model messages
    id.rs, clock.rs
  agent/             application layer: the agent loop
    ports.rs           Model, EventLog, Toolbox: what the loop needs from outside
    turn.rs            openfox chat/agent-loop: the multi-turn loop
    round.rs           one model round: streaming, retries, continuation
    tool_calls.rs      openfox chat/execute-tools: runs and records tool calls
    prompt.rs          openfox chat/prompts: the system prompt
  tools/             Toolbox adapter on the host: one file per tool, path_guard, args
  llm/               Model adapter: openai.rs (client), sse.rs (stream parser)
  store.rs           EventLog adapter: SQLite (openfox's schema), projects and sessions
  git.rs             what fennec reads from a repository
  web/               HTTP adapter: http, guard (who may call), routes, event_stream (SSE), turns, assets
  cli/               terminal adapter: commands, transcript
crates/fennec/build.rs   embeds web/dist (placeholder page when not built)
web/src/           React + Vite + Tailwind v4 UI (same layout as openfox's web/src)
  pages/             one component per route: HomePage, SessionPage
  components/        shared UI; components/session/ the thread, tool cards, composer, context panel
  hooks/             useSessionEvents: the SSE stream of a session
  lib/               api (typed fetch), fold (events → thread view), format, theme
  styles/app.css     theme tokens: one CSS variable block per theme, mapped into Tailwind
docs/api.md        the HTTP API and event wire format, shared by server and UI
```

## Porting rules

- Each module's header comment names the openfox files it ports and what is
  not ported yet. Keep openfox's names (events, fields, tool names, prompts)
  so a later port maps 1:1.
- Port only what the current component needs; skip the rest explicitly.
- Port the matching openfox `*.test.ts` cases with the code.

## Trust zones

1. **The human**: the CLI/UI, the configuration. fennec never blocks them.
2. **The trusted kernel**: code that decides (integration, plugin activation).
   It never runs what a model wrote.
3. **The untrusted zone**: everything a model outputs. **Known gap**: tools
   run on the host with only a working-directory path guard (as in openfox);
   the bwrap world comes back as an early component. Until then, run fennec
   on throwaway repositories only.

## Build, test, run

```bash
cargo build
cargo test
cargo clippy --all-targets   # must be clean
cargo fmt --all
(cd web && npm ci && npm run build)  # before cargo build, to embed the UI
target/debug/fennec                  # http://127.0.0.1:7777
target/debug/fennec run "<prompt>"   # CLI, in the repository to work on
(cd web && npm run dev)              # UI dev server on :5173, proxies /api to :7777
```

Env: `FENNEC_LLM_URL`, `FENNEC_MODEL`, `FENNEC_API_KEY`, `FENNEC_MAX_TOKENS`,
`FENNEC_DB` (default `~/.local/share/fennec/fennec.db`).

## Non-negotiable rules

1. **Integration is always a human decision.** No agent, plugin or schedule
   commits to the human's branch on its own.
2. **Zero trust applies to models only.** An agent can never install or
   enable a plugin.
3. **Provenance is shown.** What the UI shows is marked as verified
   (deterministic) or produced by a model (indicative).
4. **The server only answers local pages**: `Host` and `Origin` must be
   127.0.0.1/localhost (DNS rebinding, CSRF) and POST bodies must be JSON.
   Never relax this: any web page could otherwise drive the agent.
5. **Storage is SQLite** (`rusqlite`, bundled), as in openfox. No other new
   dependency without a stated reason in the commit body. Stdlib first.
6. Licence: AGPL-3.0-only; openfox's MIT notice is kept in THIRD_PARTY.md.
   The repository stays private until contribution terms are decided.

## Style

Rust 2024, rustfmt defaults, clippy clean. English for code, comments, docs
and commit messages. Comments explain *why*.

## Commit format

Conventional Commits, imperative mood (`feat:`, `fix:`, `docs:`, `refactor:`,
`test:`, `chore:`).
