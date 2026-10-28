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
Human pause, done: ask_user (the turn ends, the answer is the next input),
clean stop, open tool calls closed after a stop or a crash.
Compaction, done: automatic at a threshold and manual; the new window opens
from the model's summary plus facts taken from the log (files changed, the
last request verbatim); the log keeps everything.
Settings page, done: per-model parameters, display defaults, a thread loaded
lazily beyond the last exchanges.
Shared conventions, done (nothing tool-specific): global instructions and the
AGENTS.md files in a new session's prompt; skills from `~/.agents/skills` and
the project's `.agents/skills` (SKILL.md packages), listed in the prompt and
read with `load_skill`; `/compact` and `/<skill>` typed in a message, with
a composer menu.
Agents, done: `*.agent.md` files (embedded defaults, `~/.agents/agents`,
the project's `.agents/agents`); two modes, plan (read-only by construction,
the default) and builder, with a fixed plan and report format rendered as
cards. Vision: images attached to a message, and read_file returns images.
Replay: a typed message sent again, through an append-only `session.rewound`.
Skills panel, done: the skills of a session in the context panel, those
loaded so far and those still active (in the model's view: not dropped by
a compaction, not cut by a replay), folded from the log.
Workflows, done (`docs/workflows.md`): graphs of agent, shell, human,
condition, parallel/join nodes wired by named ports, drawn in a React Flow
editor; loops capped per edge, pauses on edges or step by step; the run is
folded from `workflow.*` events; fresh agents and branches in child sessions.
The default workflow runs the criteria checks, verifies in a fresh context
(`verify`, `review` sub-agents) and sends the findings back to the builder.
Preview: ```html and ```md blocks of a message, and the .html/.md files an
agent wrote, rendered in fennec (a sandboxed origin for model HTML).
Dogfooding, started (`docs/dogfooding.md`): a session that runs a workflow
works in its own git worktree and branch; the human cherry-picks.
Web search, done: openfox's `web_search` (SearXNG only), its instance set on
the Settings page.
Browser and vision, done: openfox's `browser` skill embedded as a default
(`@playwright/cli` through run_command, screenshots read back with read_file,
kept out of the worktree's commits), and a Vision section in the context panel:
the images the model saw, those a compaction dropped dimmed.
Sandbox, done: every command (run_command, workflow shell nodes) runs in a bwrap
world (`tools/sandbox.rs`): the system read-only, the workdir writable, secrets
and fennec's store hidden, no network by default; the Settings page turns it or
the network off.
Criteria, done: the "Done when" list of the approved plan is the run's contract
(`criteria.set`); a `criteria` node runs each criterion's `check:` command
itself (verified), an agent node with `verdicts` judges the others
(indicative); the context panel shows both, with their provenance.
Protected files, done: fennec's defaults (`.agents`, AGENTS.md, lockfiles, CI)
and the project's `.agents/protected`; the file tools refuse them, the
sandbox mounts them read-only (`domain/protection.rs`, `protected.rs`).
Integration, done: an "Intégrer" button runs the cherry-pick of a session's
worktree branch into the human's repository, on the human's click only, after
a review gate (commits, files, criteria, protected files touched) whose
blockers the human must accept one by one (`integration.applied` records it).
Decision registry, done: `record_decision` (the turn goes on) records a choice, its reason
and the alternatives as `decision.recorded`; a "Décisions" block in the context panel.
Protected tests, done: an unchecked "Protéger les tests intégrés" box in the
integration dialog appends the integrated test files to `.agents/protected`
(`domain/test_files.rs`).
Architecture guard, done: `.agents/architecture` (`<path prefix> forbids <text>`) is
checked on the lines an integration adds; a violation is a blocker the human accepts
(`domain/architecture.rs`, `architecture.rs`).
Next, one PR each: the rest of fennec's own layer → sub-agents as tools, when dogfooding shows sessions
saturating their context while exploring.

## Layout

Clean architecture, applied where it pays: dependencies point inward
(adapters → agent → domain). The agent loop only knows the ports in
`agent/ports.rs`; a port exists because it decouples a layer or has a
second implementation (a test double), not for show. One responsibility per file; no catch-all module (`utils`, `misc`,
`common`).

```
crates/fennec/src/
  main.rs            composition root: command line → settings → web or cli adapter
  config.rs          settings from the environment (model endpoint, store path): the defaults
  domain/            the vocabulary, no I/O
    event.rs           openfox events/types: Event, ToolCall, ToolResult, Role, DoneReason
    stats.rs           openfox MessageStats: tokens (in, cached, out) and speeds of a call
    project.rs         Project, SessionSummary, SessionState
    conversation.rs    openfox events/fold-messages: the log read back as model messages
    context_parts.rs   what the model's context is made of, by part (indicative: bytes, pro rata)
    open_calls.rs      tool calls left without a result (ask_user, stop, crash)
    compaction.rs      where the model's view starts, when to compact, the resume message
    preferences.rs     what the human chose on the Settings page; resolves a turn's model parameters
    skill.rs           openfox skills/types: a SKILL.md package, its frontmatter, the text load_skill returns
    slash.rs           openfox tasks/slash: what a typed /compact or /<skill> stands for
    agent.rs           an agent (mode) and its reminder; frontmatter.rs the shared YAML header
    attachment.rs      images sent to the model: sniffing, data URLs, limits
    replay.rs          which message the human may replay
    criteria.rs        acceptance criteria: the plan's "Done when", checks, verdicts, provenance
    protection.rs      protected files: the defaults, the gitignore-like patterns, the matcher
    test_files.rs      which paths are tests; the lines an integration appends to `.agents/protected`
    integration.rs     the review gate: what blocks an integration until the human accepts it
    workflow/          a workflow: definition (nodes, ports, edges), template ({{…}} names),
                       validate (the graph rules), run (the run folded from its events, the next step)
    id.rs, clock.rs
  agent/             application layer: the agent loop
    ports.rs           Model, EventLog, Toolbox, ChildSessions, Commands: what the loop needs from outside
    turn.rs            openfox chat/agent-loop: the multi-turn loop
    round.rs           one model round: streaming, retries, continuation, timing
    delta_buffer.rs    coalesces streamed chunks into one event per 100 ms
    tool_calls.rs      openfox chat/execute-tools: runs and records tool calls
    ask_user.rs        openfox tools/ask: the loop's own tool; the turn ends waiting for the human
    compaction.rs      openfox context/compactor: the summary round and the new window
    prompt.rs          openfox chat/prompts: the system prompt, plus fennec's working method
    workflow.rs        the workflow walk and its jobs (start, answer, continue, resume, stop);
                       workflow/ nodes (one node run), fork (parallel branches), outcome (`Outcome:` line),
                       criteria (the criteria node and the agents' verdicts)
  tools/             Toolbox adapter: one file per tool, path_guard, args; sandbox (the bwrap
                     world of commands);
                     web_search queries the SearXNG instance from the Settings page
  llm/               Model adapter: openai.rs, the Model port over llm-providers (tool-call arguments parsed here)
  store.rs           EventLog adapter: SQLite (openfox's schema), projects and sessions;
                     store/settings.rs the stored preferences
  metering.rs        the usage ledger adapter: MeteredLog (an EventLog that also writes each model call
                     to the ledger) and the backfill of past calls; store/calls.rs reads them from the log
  git.rs             what fennec reads from a repository
  worktree.rs        a session's git worktree: a workflow never writes the human's tree
  integration.rs     the human's integration: preview of a worktree branch, cherry-pick, conflict abort
  instructions.rs    openfox context/instructions: Settings' global instructions + the
                     AGENTS.md files above the workdir, for a new session's prompt
  agents.rs          the *.agent.md files: embedded defaults (crates/fennec/agents/), ~/.agents/agents,
                     the project's .agents/agents (the project wins)
  skills.rs          openfox skills/registry: embedded defaults (crates/fennec/skills/), the SKILL.md
                     packages in ~/.agents/skills and the project's .agents/skills (the project wins)
  workflows.rs       the *.workflow.json files: embedded defaults (crates/fennec/workflows/),
                     ~/.agents/workflows, the project's .agents/workflows (the project wins)
  folders.rs         the folders the human browses to open a project
  protected.rs       the project's protected files: .agents/protected read, the existing paths found
  web/               HTTP adapter: http, guard (who may call), routes, event_stream (SSE),
                     notifier (wakes streams on append, no polling), turns, assets,
                     workflow_routes, children (child sessions for workflow nodes),
                     preview (a project's files in a sandboxed origin), session_usage,
                     workflow_usage, context_view and insight (display-ready token figures: the UI computes none)
  cli/               terminal adapter: commands, transcript
crates/token-usage/ the usage ledger, a crate of its own and fennec-free (calls, tokens, speed, savings;
                     SQLite file `usage.db`, `FENNEC_USAGE_DB`): outlives sessions, reusable by other products
crates/llm-providers/ talking to models, a crate of its own and fennec-free: the OpenAI-compatible client
                     and its SSE parser, the /models listing, per-model parameters (ReasoningEffort, ModelPrefs)
crates/fennec/build.rs   embeds web/dist (placeholder page when not built)
web/src/           React + Vite + Tailwind v4 UI (same layout as openfox's web/src)
  pages/             one component per route: HomePage, SessionPage, SettingsPage, InsightPage,
                     WorkflowsPage (lazy)
  components/        ui/ the primitives every screen uses (Button, Badge, SectionLabel, Card, Panel, fields);
                     shared UI; session/ the thread, tool cards, question card, message stats, composer,
                     context panel; project/ the folder browser; layout/ the top bar;
                     workflow/ the editor (canvas, node shapes, panels) and session/ a run in the thread;
                     preview/ the preview pane and previewable code blocks
  hooks/             useSessionThread: the SSE stream of a session, folded
  lib/               api (typed fetch), fold (events → thread view), format, theme,
                     attachments (images read in the browser), plan (the plan/report contract),
                     workflow (definition types and edits), workflowRun (a run folded from its events),
                     preview (what can be previewed, its URL), vision (the images the model saw),
                     panelBlocks (the context panel's blocks and pages)
  styles/app.css     theme tokens: one CSS variable block per theme, mapped into Tailwind
docs/api.md        the HTTP API and event wire format, shared by server and UI
docs/workflows.md  the workflow format, run semantics and editor
docs/ui.md         the UI's design rules: tokens and primitives, never per-file styles
docs/dogfooding.md the frictions met while fennec builds fennec, and their status
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
3. **The untrusted zone**: everything a model outputs. Commands run in the
   bwrap world (`tools/sandbox.rs`) unless the human turns it off. Protected
   files are refused to the file tools and read-only in the sandbox. **Known
   gaps**: the file tools run in fennec's process with a working-directory
   path guard; a command can still create a new file matching a protected
   pattern where none existed (the review gate is to catch it).

## Build, test, run

```bash
just                # build the UI and run the release server on :7777
just check          # the gate before any commit: fmt, clippy -D warnings, tests, UI build
just dev            # server + Vite dev server on :5173
```

Without just:

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
`FENNEC_COMPACT_AT` (default 0.85, 0 disables), `FENNEC_DB` (default `~/.local/share/fennec/fennec.db`).
The model variables are defaults: the Settings page (stored in SQLite) overrides them
per model, from the next turn on.

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

**Keep the architecture and the code clean — every change, not in a later
clean-up.** DDD, KISS, clean code, clean architecture and SOLID, applied as
far as they help, never blindly:

- Every change leaves the structure at least as clean as it found it. When
  a file takes a second responsibility or a function stops fitting on a
  screen, split it in the same change.
- Respect the layers in "Layout": the domain does no I/O, the agent sees
  only its ports, adapters never reach into each other's internals.
- Name things after the domain; prefer types (enums, newtypes) to strings
  and flags.
- No god file, no catch-all module (`utils`, `misc`, `common`, `helpers`),
  no file loose at a source root (Rust or web).
- Code written by a model (the local LLM included) is reviewed against
  these rules before it is committed, not only against the tests.
- Resources are a feature: fennec must stay light in RAM and CPU (openfox
  is not). Measure before and after a change that could cost them.

**The prompt only grows.** The local inference server reuses its cache only
for an exact prefix of an earlier request. Within a session, never rewrite
what was already sent: the system prompt and the tool definitions are frozen
at the first turn (`turn.snapshot`), earlier messages fold back byte for byte, reasoning is
sent back as streamed. Anything that edits history (compaction, a changed
prompt) costs a full re-read of the session: do it deliberately, measured
with the per-message stats.

## Commit format

Conventional Commits, imperative mood (`feat:`, `fix:`, `docs:`, `refactor:`,
`test:`, `chore:`).
