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
rebuilt in Rust, **one component at a time**, each one working end to end
before the next. Its agent loop, web UI and session log started from openfox
(see "Origin" below); fennec's vocabulary is its own.

What is done: `docs/components.md`; add a paragraph there when a component lands.

Next, one PR each: fennec's own layer, the tools that keep the human the author
(capabilities C* of the Notion definition):
1. To be chosen among the remaining capabilities C* of the Notion definition.
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
    event.rs           the session log's vocabulary: Event, ToolCall, ToolResult, Role, DoneReason (wire names frozen)
    stats.rs           MessageStats: tokens (in, cached, out) and speeds of a call
    project.rs         Project, SessionSummary, SessionState
    conversation.rs    the log read back as model messages
    context_parts.rs   what the model's context is made of, by part (indicative: bytes, pro rata)
    open_calls.rs      tool calls left without a result (ask_user, stop, crash)
    compaction.rs      where the model's view starts, when to compact, the resume message
    preferences.rs     what the human chose on the Settings page; resolves a turn's model parameters
    model_choice.rs    the model a session runs on: its last `model.changed`
    skill.rs           a SKILL.md package, its frontmatter, the text load_skill returns
    slash.rs           what a typed /compact or /<skill> stands for
    agent.rs           an agent (mode) and its reminder; frontmatter.rs the shared YAML header
    attachment.rs      images sent to the model: sniffing, data URLs, limits
    replay.rs          which message the human may replay
    rule_approval.rs   the call paused for the human's approval, folded from the log; the four answers
    kernel_rules.rs    the rules fennec's kernel enforces itself: the git commands that push or move a branch, blocked
    waiver.rs          a protected file's waiver: the pending request, the active grants, the ADR a grant writes
    queue.rs           the messages queued while a turn runs: pending, those the loop may drop in, what a turn's end does
    criteria.rs        acceptance criteria: the plan's "Done when", checks, verdicts, provenance
    criteria_fold.rs   the criteria in force, folded from the log (a workflow run's state and the served thread share it)
    preflight.rs       a plan's checks run before any change: fails, passes already, cannot run
    protection.rs      protected files: the defaults, the gitignore-like patterns, the matcher
    test_files.rs      which paths are tests; the lines an integration appends to `.agents/protected`; the tests validated at the gate
    integration.rs     the review gate: what blocks an integration until the human accepts it
    diff.rs            the lines a unified diff adds and removes
    commit_subject.rs  the integration's commit subject the model proposes: its prompt, its answer checked
    bypass.rs          what silences a gate: suppression markers added, tests or assertions removed
    adr.rs             architecture decision records: a MADR file read (number, title, status, date)
    module_map.rs      the project map: modules, their dependencies from the imports, their layout
    map_doc.rs         docs/map.md: the map as Markdown (Mermaid, table), stale or current; its edges read back
    drift.rs           the produced architecture against the human's module rules and the committed docs/map.md
    risk.rs            risk zones: a commit's authorship from its trailers, the signals, the report;
                       risk/test_subject.rs which files need a test of their own (the untested signal skips the rest)
    agents_md.rs       the AGENTS.md draft at the end of an answer, which the human writes in one click
    rule_bridge.rs     the architecture rules an answer proposes, those not yet in `.agents/architecture`
    task_proposal.rs   the backlog tasks an agent proposed with propose_task, and which the human accepted
    agent_suggestion.rs the agent switch an agent suggested, since the human last wrote
    understanding.rs   the comprehension dialogue on a branch: its link, held once the human answered on the current head
    plan_scope.rs      the files the approved plan says it will touch; those a branch changed outside it
    review_map.rs      the review order of a branch's files: per-file signals (gate checks, churn, tests, size)
    workflow/          a workflow: definition (nodes, ports, edges), template ({{…}} names),
                       validate (the graph rules), run (the run folded from its events, the next step)
    doc.rs             the project's documentation: markdown files and ADRs, as the Doc tab lists them
    mcp.rs             MCP servers as the human configured them; server.rs the project's servers (`.agents/servers.json`)
    scanner.rs         the security scanners fennec knows (the gate, the audit-security agent);
    security_scan.rs   the scanners' JSON reports parsed into findings; which lockfiles a branch changed; attention.rs what a finished turn leaves for the human's eye
    audit.rs           an audit report parsed from the model's answer, and the backlog task each finding becomes
    workdir_notice.rs  tells the model its working directory moved; session_name.rs the session name's prompt and cleanup
    agent_file.rs      an agent as the Settings form edits it; session_skills.rs a session's skills folded from the log
    module_map/notes.rs what the map says about a module, read from its leading comment
    budget.rs          a project's monthly budget: its status (under, alert at 80 %, exceeded), the refusal and alert texts
    pricing.rs         the ledger's model rows priced at the models' prices, split by token kind
    indicators.rs      the project's indicators counted from the integrations the gate recorded
    lang.rs            the language the human reads fennec in (Settings); the server-built texts follow it
    id.rs, clock.rs
    (domain uses `rules::path`, the gitignore-like matcher, as a library: one matcher for rules, protection and architecture)
  agent/             application layer: the agent loop
    ports.rs           Model, EventLog, Toolbox, ChildSessions, Commands: what the loop needs from outside
    turn.rs            the multi-turn loop
    round.rs           one model round: streaming, retries, continuation, timing
    delta_buffer.rs    coalesces streamed chunks into one event per 100 ms
    tool_calls.rs      runs and records tool calls
    ask_user.rs        the loop's own tool; the turn ends waiting for the human
    propose_task.rs    the loop's propose_task tool: a backlog task proposed, the turn goes on
    suggest_agent.rs   the loop's own tool: suggests a new session on another agent (the human opens it)
    rule_check.rs      the rules crate's decision on a tool call: the refusal and its `rule.blocked`, the approval request
    rule_approval.rs   the human's answer to an approval: runs or refuses the recorded call, resumes the loop
    request_waiver.rs  the loop's own tool: asks the human to waive one protected file; the turn pauses
    waiver.rs          the human's answer to a waiver (recorded at the click): the call's result, the loop resumed
    queue.rs           the queued messages dropped into the turn between tool rounds
    preflight.rs       runs a plan's checks at the end of the planner's turn, sends it back once
    compaction.rs      the summary round and the new window
    prompt.rs          the system prompt: what fennec does around the agent, the working method, the style
    workflow.rs        the workflow walk and its jobs (start, answer, continue, resume, stop);
                       workflow/ nodes (one node run), fork (parallel branches), outcome (`Outcome:` line),
                       criteria (the criteria node and the agents' verdicts)
  tools/             Toolbox adapter: one file per tool, path_guard, args; sandbox (the bwrap
                     world of commands); rule_request (a tool call as the rules crate's requests, paths resolved here);
                     web_search queries the SearXNG instance from the Settings page; canvas writes the session's canvas store
  llm/               Model adapter: openai.rs, the Model port over llm-providers-connector (tool-call arguments parsed here)
                     concurrency.rs the local model's gate: one round at a time, the same session kept between
                     its rounds so the server's prompt cache stays warm (`FENNEC_MODEL_AFFINITY_SECS`)
  store.rs           EventLog adapter: SQLite (schema frozen: stored logs depend on it), projects and sessions;
                     store/settings.rs the stored preferences; store/ideas.rs the per-project backlog events
  metering.rs        the usage ledger adapter: MeteredLog (an EventLog that also writes each model call
                     to the ledger) and the backfill of past calls; store/calls.rs reads them from the log
  git.rs             what fennec reads from a repository
  worktree.rs        a session's git worktree: a workflow never writes the human's tree; the main
                     checkout's node_modules/.venv mounted read-only into its sandbox; one Cargo
                     target per project shared by its worktrees (`CARGO_TARGET_DIR`)
  integration.rs     the human's integration: preview of a worktree branch, cherry-pick, conflict abort
  instructions.rs    Settings' global instructions + the
                     AGENTS.md files above the workdir, for a new session's prompt
  agents.rs          the *.agent.md files: embedded defaults (crates/fennec/agents/), ~/.agents/agents,
                     the project's .agents/agents (the project wins)
  skills.rs          embedded defaults (crates/fennec/skills/), the SKILL.md
                     packages in ~/.agents/skills and the project's .agents/skills (the project wins)
  workflows.rs       the *.workflow.json files: embedded defaults (crates/fennec/workflows/),
                     ~/.agents/workflows, the project's .agents/workflows (the project wins)
  folders.rs         the folders the human browses to open a project
  protected.rs       the project's protected files: .agents/protected read, the existing paths found
  adr.rs             the project's ADR files in docs/adr/, listed; a waiver's ADR written
  module_map.rs      the project map's source files (git ls-files) and docs/map.md read, written
  risk.rs            the risk zones measured from git: AI commits, churn, a parallel blame; the hot
                     files and the files tested on a branch, for the review map
  doc.rs             the project's markdown files read for the Doc tab
  plugins.rs         the plugin manager: one sandboxed host per active plugin, server-wide; plugins/ config (the
                     definitions in ~/.agents, an activation's validity), host (one plugin's process and its blocks),
                     pump (the new session events fed to the hosts, woken by the notifier)
  budget.rs          a project's budget against the month's spend read from the ledger; the kernel's check before a turn
  chat.rs            a session with no project: runs in a scratch folder
  turn_build.rs      the composition of a Turn, shared by both adapters (CLI, web): store, model, tools, sandbox
  mcp.rs             the synchronous MCP client (JSON-RPC over stdio); servers.rs the project's servers on disk
  scanners.rs        the security scanners found on PATH
  security_scan.rs   the gate's scanners run in the read-only sandbox, cached per (base, branch)
  notifier.rs        wakes event streams when a turn appends to a log, so an open tab costs nothing idle
  architecture.rs    `.agents/architecture` read from disk
  agents/crud.rs     the human's own agents read, written, deleted (Settings)
  worktree/usage.rs  the disk the worktrees take, measured and freed on the human's click
  llm/concurrency.rs the model's concurrency gate (a process-wide counting semaphore)
  web/               HTTP adapter: http, guard (who may call), routes, settings_routes, event_stream (SSE),
                     notifier (wakes streams on append, no polling), turns, assets,
                     session_routes (one session: created, read, written to, replayed, compacted, streamed),
                     folder_routes (the folders browsed), project_list_routes (the home's projects, favorites, a project's commands and agents),
                     test_support (the App the web tests build), thread_view (the pending approval, waiver, queue, criteria, decisions and replayable messages, served), workflow_routes, project_routes (the project page), project_delete (a project deleted, its folder on request), project_dashboard (its overview tiles), idea_routes (its ideas board, HTTP only),
                     idea_queue (the queue's thread: starts ready ideas, night-mode caps, queue runs recorded),
                     model_routes (the composer's model choices, a session's model), rule_routes (the human's rules), rule_approval_routes (an approval answered, its rule stored), waiver_routes (a waiver answered, its ADR written),
                     children (child sessions for workflow nodes),
                     preview (a project's files in a sandboxed origin), session_usage,
                     workflow_usage, context_view and insight (display-ready token figures: the UI computes none),
                     notice_stream (the human's attention server-wide, SSE), idea_board (the ideas board, read side),
                     servers (a project's servers started and stopped), terminal, editor, file_manager (the human's own
                     tools opened on a folder), model_view, worktree_routes (disk usage), agent_routes,
                     audit_routes, backlog_routes, idea_assets, mcp_routes, proposal_routes, session_delete,
                     agents_md_routes, session_name (background naming), canvas_routes (the canvas crate over HTTP),
                     budget_routes (a project's budget, read and set),
                     contribution_routes (branches someone else wrote, adopted into a session for the gate),
                     indicator_routes (a project's indicators from its logs, git and the ledger)
  cli/               terminal adapter: commands, transcript
crates/backlog/     the ideas backlog, a crate of its own and fennec-free, no I/O: its events folded, the queue
                     order, the night-mode caps, an idea's status, stage and next action, a run's summary
crates/rules/       the human's rules, a crate of its own and fennec-free: the model, the command and path matchers,
                     the decision (block > allow > ask, narrowest scope), its SQLite store `rules.db` (`FENNEC_RULES_DB`)
crates/canvas/      the canvas, a crate of its own and fennec-free: artifacts (a pinned file or a message block), their
                     versions' bytes, the human's annotations and the message each becomes; SQLite file `canvas.db`
crates/token-usage/ the usage ledger, a crate of its own and fennec-free (calls, tokens, speed, savings;
                     SQLite file `usage.db`, `FENNEC_USAGE_DB`): outlives sessions, reusable by other products
llm-providers-connector/ (crates.io crate, github.com/ikarys/llm-providers-connector) talking to models, fennec-free: the OpenAI-compatible client
                     and its SSE parser, the /models listing, providers and their per-model parameters
                     (Provider, ModelRef, ModelPrefs, ReasoningEffort), API keys sealed at rest (MasterKey)
crates/fennec/build.rs   embeds web/dist (placeholder page when not built)
web/src/           React + Vite + Tailwind v4 UI
  pages/             one component per route: HomePage, SessionPage, SettingsPage, InsightPage,
                     ProjectPage, WorkflowsPage (lazy)
  components/        ui/ the primitives every screen uses (Button, Badge, SectionLabel, Card, Panel, fields);
                     shared UI; session/ the thread, tool cards, question card, message stats, composer,
                     context panel; project/ the folder browser, the dashboard, the project map and ideas/
                     (the Tasks tab: queue bar, night report, stage groups, sub-tabs, done table, idea rows); layout/ the top bar;
                     workflow/ the editor (canvas, node shapes, panels) and session/ a run in the thread;
                     preview/ the preview pane and previewable code blocks
  hooks/             useSessionThread: the SSE stream of a session, folded
  lib/               api (typed fetch), route (the hash as a route, tab paths, stored tabs), settingsForm (the Settings form
                     read from and written back as preferences), integration (the gate's words and button state), fold (events → thread view), format, theme,
                     attachments (images read in the browser), plan (the plan/report contract),
                     workflow (definition types and edits), workflowRun (a run folded from its events),
                     preview (what can be previewed, its URL), vision (the images the model saw),
                     panelBlocks (the context panel's blocks and pages)
  i18n/              messages (the en and fr dictionaries, `t`), index (`useT`, `useLang`: the language chosen in Settings)
  styles/app.css     theme tokens: one CSS variable block per theme, mapped into Tailwind
docs/api.md        the HTTP API and event wire format, shared by server and UI
docs/workflows.md  the workflow format, run semantics and editor
docs/ui.md         the UI's design rules: tokens and primitives, never per-file styles
docs/dogfooding.md the frictions met while fennec builds fennec, and their status
docs/components.md what fennec already does, one paragraph per component, with its files
```

## Origin

fennec started as a Rust port of openfox (https://github.com/co-l/openfox, MIT,
commit `0694574d`, v2.0.161); THIRD_PARTY.md keeps the notice. The code does not
advertise it: no "port of" headers, no origin names in comments, test names or UI
strings; name things after fennec's domain. What stays compatible on purpose is
named for what it is: the event wire names and the SQLite schema are frozen because
stored logs depend on them. Port only what the current component needs, with its
tests.

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
   a script written then run slips past them: a guard rail, the sandbox stays the wall. A file
   tool outside the workdir needs an allow rule or the human's approval; the sandbox does not read the rules (it still hides
   `~/.ssh` from commands). A session's worktree shares its `.git/worktrees/<id>`, so its refs, with the human's
   repository: with the sandbox off a command could push, move or delete a branch, and `git push` reaches the remote
   even sandboxed when the network is on. The kernel's own block rules (`domain/kernel_rules.rs`: `git push`,
   `update-ref`, `branch -D`, `worktree`, `gh pr merge`…) refuse those commands before they run, with the same
   token-matching limits as the human's rules.

## Build, test, run

```bash
just                # build the UI and run the release server on :7777
just check          # the gate before any commit: fmt, clippy -D warnings, tests, UI lint, UI tests, UI build
just test-web       # the Vitest tests of the UI's pure modules (folds, plan contract)
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
(cd web && npm ci && npm test)             # the UI's unit tests (Vitest)
(cd web && npm run build)  # before cargo build, to embed the UI
target/debug/fennec                  # http://127.0.0.1:7777
target/debug/fennec run "<prompt>"   # CLI, in the repository to work on
(cd web && npm run dev)              # UI dev server on :5173, proxies /api to :7777
```

Env: `FENNEC_LLM_URL`, `FENNEC_MODEL`, `FENNEC_API_KEY`, `FENNEC_MAX_TOKENS`,
`FENNEC_COMPACT_AT` (default 0.85, 0 disables), `FENNEC_DB` (default `~/.local/share/fennec/fennec.db`), `FENNEC_RULES_DB` (default `rules.db` next to it).
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
5. **Storage is SQLite** (`rusqlite`, bundled). No other new
   dependency without a stated reason in the commit body. Stdlib first.
6. Licence: AGPL-3.0-only; the MIT notice of the code fennec started from is kept in THIRD_PARTY.md.
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
- The UI is never dull (`docs/ui.md`, "Never dull"): lists sit in a `Card` with a tinted header row,
  openable rows and cards react with the accent and open as a whole, figures are `Tile`s with a
  tone, row actions are icons (the delete always visible, `danger`), meanings are tinted badges.
- Resources are a feature: fennec must stay light in RAM and CPU. Measure before and after a change that could cost them.

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
