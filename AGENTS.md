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
Bypass detection, done: added lines that silence a lint, a type check or a test, and
tests or assertions removed on balance, are blockers the human accepts (`domain/bypass.rs`).
ADRs, done: the project page lists the MADR files of `docs/adr/`, protected
(`domain/adr.rs`, `adr.rs`). Not the agents' recorded decisions: those explain one change,
an ADR binds the project.
Project page and map, done: `#/project/<id>` (the "Projet" button of a home card) shows the
modules and their dependencies read from the imports (verified, `domain/module_map.rs`), and
writes them on the human's click to `docs/map.md` (`domain/map_doc.rs`), stale when the code moved:
committed, it gives the project's shape to the next developer or agent without reading the code.
Project analysis, done: an "Analyser le projet" button opens a session on the read-only `analyze`
agent (`crates/fennec/agents/analyze.agent.md`), which drafts AGENTS.md's philosophy, layers and rules,
each with its file:line source (indicative); the human reviews it.
Ideas backlog, done: the project page keeps the human's ideas (`idea.*` events per project,
`crates/backlog`, `store/ideas.rs`); "Cadrer" opens a plan session tied to the idea, whose plan card marks
it ready with the plan verbatim. "Lancer la file" runs the ready ideas one at a time, oldest first, through
the default workflow in each idea's session and worktree (`idea.started`); each shows its run's state, and
integrating stays the human's click.
Night mode, done: an "autonome" box runs the queue with the human away (`Turn::autonomous`): `ask_user` answers
itself (the agent decides and records it), `propose_criteria` is refused; per-idea caps on duration and tokens
(Settings, "Mode nuit") stop an idea and the queue goes on (`idea.halted`); each started idea shows its criteria
and the decisions taken, for the morning. Each queue run is recorded (`idea.queue_started`, `idea.queue_finished`, a
restart closes it); the board (groups by stage, one next action per idea, the night report until read) is computed by
the server.
Risk zones, done (C11): a "Risques" tab on the project page lists the source files that churn, have
no tests, or whose every line (git blame) comes from an AI commit never integrated through fennec (an AI
`Co-Authored-By` trailer, no `Fennec-Session`), read from git (verified; `domain/risk.rs`, `risk.rs`).
Questions, done (C10): the Analyses tab of the project page takes a question and opens a session on the
read-only `ask` agent (`crates/fennec/agents/ask.agent.md`), which answers with a `path:line` source per claim
(indicative).
Audits, done: the read-only `audit-architecture` and `audit-security` agents, launched from the Analyses
tab, report findings as cards the human turns into backlog tasks (never offered twice); the security
audit first runs the scanners found on PATH (Settings, never installed: the start of C23).
Review map, done (C9, part 1): the integration dialog lists the branch's files riskiest first, each with
its signals (the gate's checks per file, churn, untested, large), read from git (verified; `domain/review_map.rs`).
Narrative diff, done (C9, part 2): a "Raconter" button in the integration dialog opens a session on the
read-only `narrate` agent (`crates/fennec/agents/narrate.agent.md`), which tells the branch's change file by
file in the review order, each claim with its `path:line` source (indicative).
Project kickoff, done: the read-only `kickoff` agent ("Démarrer le projet" in the Analyses tab) asks what a new
project is for, has the human choose its stack and architecture (`record_decision`), drafts its AGENTS.md and
proposes its first tasks in build order, a walking skeleton first, with `propose_task` (`task.proposed`); the human
adds each to the backlog from a card in the thread (`task.accepted`). AGENTS.md stays protected from agents: the human
writes the draft to it in one click (`agents_md.written`).
Agent phases, done: a `phase:` in each agent's frontmatter (start, frame, build, understand, check); the server
sends the agents in that order and the composer's menu groups them under Démarrer, Cadrer, Construire, Comprendre,
Contrôler.
Agent suggestions, done: `suggest_agent` (the turn goes on) lets an agent suggest the human go on in a new
session with another agent, with a reason and a brief (`agent.suggested`); a card after the turn opens the new
session's page on that agent with the brief in the composer, and nothing starts until the human sends it.
Rules, done (part 1, block): the human's rules, global (Settings, "Règles") or per project (its "Règles" tab), stored
in SQLite (`crates/rules`, `store/rules.rs`) and enforced by fennec's code, never the model: a command matched on parsed
tokens (through `sudo`, `sh -c`, `&&`, `find -exec`), a tool name, or a path merged into the protected files. A match
refuses the call before it runs (`rule.blocked`, a "règle" badge on the tool card, verified); workflow shell nodes too.
An "ask" rule blocks until its approval flow (next PR).
Next, one PR each: fennec's own layer, the tools that keep the human the author
(capabilities C* of the Notion definition):
1. Rules, part 2: "ask" rules pause the turn for the human's approval (`rule.approval_requested`).
2. Produced architecture and drift (C12), waivers (C7).
Later: sub-agents as tools, when dogfooding shows sessions saturating their context.

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
    model_choice.rs    the model a session runs on: its last `model.changed`
    skill.rs           openfox skills/types: a SKILL.md package, its frontmatter, the text load_skill returns
    slash.rs           openfox tasks/slash: what a typed /compact or /<skill> stands for
    agent.rs           an agent (mode) and its reminder; frontmatter.rs the shared YAML header
    attachment.rs      images sent to the model: sniffing, data URLs, limits
    replay.rs          which message the human may replay
    criteria.rs        acceptance criteria: the plan's "Done when", checks, verdicts, provenance
    preflight.rs       a plan's checks run before any change: fails, passes already, cannot run
    protection.rs      protected files: the defaults, the gitignore-like patterns, the matcher
    test_files.rs      which paths are tests; the lines an integration appends to `.agents/protected`
    integration.rs     the review gate: what blocks an integration until the human accepts it
    diff.rs            the lines a unified diff adds and removes
    bypass.rs          what silences a gate: suppression markers added, tests or assertions removed
    adr.rs             architecture decision records: a MADR file read (number, title, status, date)
    module_map.rs      the project map: modules, their dependencies from the imports, their layout
    map_doc.rs         docs/map.md: the map as Markdown (Mermaid, table), stale or current
    risk.rs            risk zones: a commit's authorship from its trailers, the signals, the report
    agents_md.rs       the AGENTS.md draft at the end of an answer, which the human writes in one click
    task_proposal.rs   the backlog tasks an agent proposed with propose_task, and which the human accepted
    agent_suggestion.rs the agent switch an agent suggested, since the human last wrote
    review_map.rs      the review order of a branch's files: per-file signals (gate checks, churn, tests, size)
    workflow/          a workflow: definition (nodes, ports, edges), template ({{…}} names),
                       validate (the graph rules), run (the run folded from its events, the next step)
    doc.rs             the project's documentation: markdown files and ADRs, as the Doc tab lists them
    id.rs, clock.rs
  agent/             application layer: the agent loop
    ports.rs           Model, EventLog, Toolbox, ChildSessions, Commands: what the loop needs from outside
    turn.rs            openfox chat/agent-loop: the multi-turn loop
    round.rs           one model round: streaming, retries, continuation, timing
    delta_buffer.rs    coalesces streamed chunks into one event per 100 ms
    tool_calls.rs      openfox chat/execute-tools: runs and records tool calls
    ask_user.rs        openfox tools/ask: the loop's own tool; the turn ends waiting for the human
    propose_task.rs    the loop's propose_task tool: a backlog task proposed, the turn goes on
    suggest_agent.rs   the loop's own tool: suggests a new session on another agent (the human opens it)
    rule_check.rs      the human's rules checked before a tool call: the refusal and its `rule.blocked`
    preflight.rs       runs a plan's checks at the end of the planner's turn, sends it back once
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
                     store/settings.rs the stored preferences; store/ideas.rs the per-project backlog events;
                     store/rules.rs the human's rules (rows: config, not events)
  metering.rs        the usage ledger adapter: MeteredLog (an EventLog that also writes each model call
                     to the ledger) and the backfill of past calls; store/calls.rs reads them from the log
  git.rs             what fennec reads from a repository
  worktree.rs        a session's git worktree: a workflow never writes the human's tree; the main
                     checkout's node_modules/.venv mounted read-only into its sandbox
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
  adr.rs             the project's ADR files in docs/adr/, listed
  module_map.rs      the project map's source files (git ls-files) and docs/map.md read, written
  risk.rs            the risk zones measured from git: AI commits, churn, a parallel blame; the hot
                     files and the files tested on a branch, for the review map
  doc.rs             the project's markdown files read for the Doc tab
  web/               HTTP adapter: http, guard (who may call), routes, settings_routes, event_stream (SSE),
                     notifier (wakes streams on append, no polling), turns, assets,
                     workflow_routes, project_routes (the project page), project_dashboard (its overview tiles), idea_routes (its ideas board, HTTP only),
                     idea_queue (the queue's thread: starts ready ideas, night-mode caps, queue runs recorded),
                     model_routes (the composer's model choices, a session's model), rule_routes (the human's rules),
                     children (child sessions for workflow nodes),
                     preview (a project's files in a sandboxed origin), session_usage,
                     workflow_usage, context_view and insight (display-ready token figures: the UI computes none)
  cli/               terminal adapter: commands, transcript
crates/backlog/     the ideas backlog, a crate of its own and fennec-free, no I/O: its events folded, the queue
                     order, the night-mode caps, an idea's status, stage and next action, a run's summary
crates/rules/       the human's rules, a crate of its own and fennec-free, no I/O: a rule (scope, target, action),
                     the command pattern and the quote-aware splitter that matches it, which rule wins
crates/token-usage/ the usage ledger, a crate of its own and fennec-free (calls, tokens, speed, savings;
                     SQLite file `usage.db`, `FENNEC_USAGE_DB`): outlives sessions, reusable by other products
crates/llm-providers/ talking to models, a crate of its own and fennec-free: the OpenAI-compatible client
                     and its SSE parser, the /models listing, providers and their per-model parameters
                     (Provider, ModelRef, ModelPrefs, ReasoningEffort), API keys sealed at rest (MasterKey)
crates/fennec/build.rs   embeds web/dist (placeholder page when not built)
web/src/           React + Vite + Tailwind v4 UI (same layout as openfox's web/src)
  pages/             one component per route: HomePage, SessionPage, SettingsPage, InsightPage,
                     ProjectPage, WorkflowsPage (lazy)
  components/        ui/ the primitives every screen uses (Button, Badge, SectionLabel, Card, Panel, fields);
                     shared UI; session/ the thread, tool cards, question card, message stats, composer,
                     context panel; project/ the folder browser, the dashboard, the project map and ideas/
                     (the Tasks tab: queue bar, night report, stage groups, sub-tabs, done table, idea rows); layout/ the top bar;
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
   pattern where none existed (the review gate is to catch it). The human's rules are
   kernel code too; command rules match parsed tokens, so `eval`, a variable (`$X -rf`) or
   a script written then run slips past them: a guard rail, the sandbox stays the wall.

## Build, test, run

```bash
just                # build the UI and run the release server on :7777
just check          # the gate before any commit: fmt, clippy -D warnings, tests, UI build
just dev            # server + Vite dev server on :5173
just setup          # once per clone: the git hooks (Conventional Commits, no direct push to main/develop)
just promote        # open the PR develop → main; release-please then prepares the release
```

Branches, PRs, versions and binaries: `docs/releasing.md`. Feature branches go to
`develop` by PR (squash, Conventional title); versions are never edited by hand.

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
The model variables are defaults and define the `local` provider: the Settings page (stored in
SQLite) adds providers and overrides them per model, from the next turn on. Stored API keys are
sealed with `secret.key`, next to the store (lose it and the stored keys are lost).

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
Every commit, whoever writes it (human, agent, local model): the message is in
English, subject and body, and follows Conventional Commits; no exception.
