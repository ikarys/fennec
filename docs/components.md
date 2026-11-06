# Components done

What fennec already does, one component per paragraph, in the order they were built.
The current direction and rules live in AGENTS.md.

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
Web search, done: the `web_search` tool (SearXNG only), its instance set on
the Settings page.
Browser and vision, done: the `browser` skill embedded as a default
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
Its commit message starts from an English Conventional subject the model proposes (indicative,
`domain/commit_subject.rs`), never over the human's edit.
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
Project analysis, done: the "Write the project's charter" tile opens a session on the read-only `analyze`
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
no tests, or more than half of whose lines (git blame) come from AI commits never integrated through fennec (an AI
`Co-Authored-By` trailer, no `Fennec-Session`), read from git (verified; `domain/risk.rs`, `risk.rs`), folded by module. A "Remédier" button
opens a session on the read-only `remedy` agent (`crates/fennec/agents/remedy.agent.md`) with the zones as its brief;
it proposes at most five test tasks with `propose_task` and lists the files the human should read first (indicative).
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
Comprehension dialogue, done (C10, part 2): an "Understand" button in the integration dialog opens a session on the
read-only `understand` agent (`crates/fennec/agents/understand.agent.md`), which asks the human two or three questions
about the branch, explains what they missed with `path:line` sources and checks again; not an exam, no score. The link is
`understanding.started` in the reviewed session's log; the dialogue counts as held once the human answered on the branch's
current head (verified; `domain/understanding.rs`). A branch that changes a sensitive zone without one gets an
`understandingMissing` blocker the human accepts.
The same agent holds the dialogue on any part of the project the human names (a module, a flow, a design choice): a
bulb button next to the Analyses tab's question form, which reads the current code and asks architecture-level questions.
Learning mode, dropped (2026-11-05): the `mentor` agent was `ask` with a "do not write it for me" instruction; the
human writes that in their message when they want it.
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
Rules, done: the human's rules, global, per project or granted to a session, owned by the `rules`
crate (model, matching, decision, its own SQLite file `rules.db`) and enforced by fennec's code, never the model: a
command matched on parsed tokens (through `sudo`, `sh -c`, `&&`, `find -exec`), a tool name, or a path read or written
(project-relative, absolute or `~/…`). A block refuses the call before it runs (`rule.blocked`, a "règle" badge on the
tool card, verified); workflow shell nodes too. Settings' Security tab (`#/settings/security`) shows, adds and deletes them
all; the project page shows its own read-only. An "ask" rule, or a file tool reaching outside the workdir, pauses
the turn with the call open (`rule.approval_requested`); the human answers on a card: once, for the session (a grant
on the path and its subdirectories, the command, the tool), always or never (a project rule). fennec then runs the
recorded call itself or refuses it, and the turn goes on: the model never retries. With the human away (night mode),
an "ask" is refused.
Rule bridge, done (C16): the `analyze` agent (on an existing project) and the `audit-architecture` agent end their answer with
a `## Executable rules` block of text and import rules for `.agents/architecture`, each line under a comment citing its
guideline; the human appends the new ones in one click (`architecture.rules_added`, `domain/rule_bridge.rs`,
`web/rule_bridge_routes.rs`). A guideline is advice; the gate's rule is not.

Composer agents, done: `composer: false` in an agent's frontmatter keeps it out of the composer's menu; it is for agents
fennec launches itself with a brief it builds (Analyses: `analyze`, `audit-architecture`, `audit-security`; gate: `narrate`,
`understand`; Risks: `remedy`; test node: `tests`), so the composer offers only agents one talks to freely (`plan`, `builder`, `ask`); `subagent: true` marks workflow-only ones (`review`, `verify`) (`domain/agent.rs`, `web/project_list_routes.rs`).
Produced architecture, done (C12): `.agents/architecture` also takes module rules (`<A> never imports <B>`), checked on
every dependency of the project map; the "Carte des modules" tab lists the breaks, draws them red, and counts the dependencies added or
removed since the committed `docs/map.md` (verified; `domain/drift.rs`).
Waivers, done (C7): an agent that needs a protected file calls `request_waiver` (path, reason); the turn pauses on
a card where only the human grants it, for 1, 8 or 24 hours, or refuses. A grant exempts exactly that path, in that
session, until it expires (the file tools and the sandbox), and writes a MADR file to the session's `docs/adr/`, which
goes with the branch to the integration gate (`domain/waiver.rs`, `agent/waiver.rs`, `web/waiver_routes.rs`). With the
human away, a request is refused.
Prompt queue, done: a message typed while a turn runs is queued (`queue.added`, asap mode) and shown above the
composer, each removable; the loop drops the queued messages into the turn between tool rounds (a slash command waits),
and the first one left starts the next turn when it ends (`domain/queue.rs`, `agent/queue.rs`). A stop drops the queue;
a pending approval or waiver holds it until the human answers, an unfinished workflow run until it ends (its same-session
agent nodes drop it in).
Sensitive zones, done (C2, proportionality): `sensitive <pattern>` lines in `.agents/architecture` (gitignore-like) mark
the paths where the human's review is mandatory; a branch that changes one gets a `sensitiveTouched` blocker the human
accepts, and those files lead the review map with a "zone sensible" signal (verified; `domain/architecture.rs`).
Plan deviation, done (C6): the gate reads the `Files:` lines of the run's approved plan; the branch's files no step lists
get an `outsidePlan` blocker the human accepts and a "hors plan" signal in the review map (verified; `domain/plan_scope.rs`).
Project deletion, done: a trash button on the project page deletes everything fennec holds for a project (sessions,
worktrees and branches, rules, ideas); its source folder only when the human ticks the box, never the home, the root or
fennec's store (`web/project_delete.rs`).
UI languages, done: English by default, French on the Settings page; every UI string comes from `web/src/i18n/messages.ts`,
whose `fr` is typed against `en`'s keys (a missing translation is a compile error); components call `useT()`, pure
lib functions take a `lang`. The server's display texts follow the same choice through `domain/lang.rs`; the context part labels, stored
French in the usage ledger, are translated at display time (`context_parts::display_label`).
Plugins, done: the human's own extensions, global only: defined in `~/.agents/plugins.json`, code in
`~/.agents/plugins/<name>/` (never read from a project), activated on Settings' Plugins tab (`#/settings/plugins`); a
changed definition or file deactivates one (SHA-256 code hash). Each runs out of process in bwrap (refused with the
sandbox off), its code read-only, no project mounted, the network only if granted; it gets the session events it asked
for over JSON-RPC on stdio and answers with data-only blocks (text, figures, badges, lists, progress bars, http(s)
links, tables) the core renders where the definition places them: the session's right-hand panel (shared or a panel
of its own), the project dashboard, the app's status bar or the Plugins tab, refreshed live through `pluginsVersion`
on `/api/notices`; Settings says why a definition was left out (`plugins.rs`, `plugins/`, `domain/plugin.rs`,
`store/plugins.rs`, `web/src/components/plugins/`); how to write one: `docs/plugins.md`.
Security gate, done (C23, part 1): the integration gate runs the security scanners found on PATH itself, in the
read-only sandbox without network (gitleaks on the branch's commits, cargo-audit when a Cargo.lock changed, semgrep with
the human's own `.semgrep.yml` on the changed files); each finding is a `securityFindings` blocker the human accepts, a
scanner missing or failing a status line; they run in the background, each one's progress shown in the dialog and
integrating refused until they end (verified, zero model tokens; `domain/security_scan.rs`, `security_scan.rs`).
Try the change, done: a play button in the session header and a "Tester" section in the integration dialog start
the project's servers in the session's worktree, with no git step for the human. Each shows its state (starting until
its port answers, ready, exited with its code), the URL it printed and its last output lines; a terminal opens in the
worktree for CLI projects. With no `.agents/servers.json`, a command guessed from the project (Justfile, package.json,
Makefile, Cargo) is saved on the human's confirmation; the main checkout's gitignored `.env*` files are linked, never
copied, and servers share the project's Cargo target (`web/servers.rs`, `domain/server.rs`).
Canvas, done: a document the human and the agent iterate on (HTML, Markdown, SVG, Mermaid, JSON, images, PDF), pinned from the
preview pane or a message's ```html/```md block, each version kept and browsable in a side pane; the human annotates
the whole page, a quote or a drawn region, and the note lands in the composer as text the agent reads. The `canvas`
crate owns it all (its own `canvas.db`, `FENNEC_CANVAS_DB`); a project's Canvas tab lists and deletes them, and deleting
a session or a project deletes its artifacts, never the file. A file's versions are taken when the canvas is read; a pinned block's, from the blocks of its format the agent writes after it (`canvas::revisions`).
The agent opens and revises one itself with the `canvas` tool (title, format, the whole document; the same title is the
same canvas, each call a version, `tools/canvas.rs`): it writes fennec's canvas store, never the project, so the read-only
agents have it too, and the pane opens on the canvas the agent just changed.
Budgets, done (C18): a monthly cost budget per project, set from a tile of the project page (`store/budgets.rs`, a
`budget:<project>` row of the settings table). The month's spend is the usage ledger priced at each model's price
(`domain/pricing.rs`, indicative); at 80 % the tile turns to attention and `/api/notices` raises a system notification;
once reached, fennec's kernel refuses to start any turn on the project (`turn_build`, every path: web, CLI, the queue,
workflow children; the message route answers 409 first so the typed text stays) until the human raises or removes it
(`domain/budget.rs`, `budget.rs`, `web/budget_routes.rs`).
Indicators, done (C19): an "Indicators" tab on the project page measures the project from recorded facts only (verified):
AI commits outside fennec, sensitive integrations without a comprehension dialogue, dialogues held, module rule breaks with
the violations accepted and the active waivers, scanner findings accepted, the month's cost per integration, the map and
ADRs, integrations tied to an approved plan, the blockers accepted per integration (`domain/indicators.rs`,
`web/indicator_routes.rs`, `store/project_events.rs`).
Test gate, done (C4): the default workflow writes the tests before the code: once the plan is approved, the `tests` agent
(`crates/fennec/agents/tests.agent.md`, `writes: tests`: its file tools refuse anything but test files) writes the tests derived
from the "Done when" criteria, and a human node asks to validate them. "Validate" records `tests.approved` with the test files
written so far (the kernel's choice, at the human's click); from the next turn on they are protected for the rest of the session
like `.agents/protected` (`domain/test_files.rs`, `turn_build.rs`), so the builder makes them pass without weakening them.
Contributions, done (C17, part 1): a "Contributions" tab on the project page lists the branches, local or remote, with
commits the human's branch lacks; a click adopts one into a new session's worktree (`worktree::adopt`: the branch's tip,
its base at the merge-base with the human's HEAD), so a colleague's work goes through the same integration gate as an
agent's (blockers, scanners, review map, comprehension dialogue). Only a listed branch is adopted; nothing merges on its
own (`git::contributions`, `web/contribution_routes.rs`).

Update, done: `fennec update [--check]` and the top bar's arrow read the latest GitHub release (`/releases/latest`'s
redirect, cached 6 h for the UI), compare versions (`domain/release.rs`) and, on the human's click only, run the
embedded `install.sh` over the binary (checksum verified, atomic rename); the server then re-executes itself, same PID,
so a systemd service keeps tracking it. Refused while a turn runs. `install.sh` writes and starts a systemd user service
(`update.rs`, `cli/update.rs`, `web/update_routes.rs`, `components/layout/UpdateNotice.tsx`).

Server port, done: `~/.config/fennec/settings.yaml` (`$XDG_CONFIG_HOME`) holds `port:`; `--port` wins over it, 7777
otherwise. Settings > Security edits it (the line replaced in place, other lines kept) and restarts the server, the page
following it to the new port (`domain/server_config.rs`, `server_config.rs`, `web/server_routes.rs`,
`components/settings/ServerSection.tsx`).

Changes page, done: the side panel's "Modified files" block keeps only the count and a link to `#/s/<id>/changes`,
which lists the branch's files in the gate's review order with their signals and shows the selected file's diff against
the session's base (the range the integration takes), colored and numbered on both sides, cut at 5,000 lines; an icon
opens the whole file in VS Code. The server parses the hunks and only diffs a path the branch changed
(`domain/diff.rs`, `integration::file_diff`, `web/diff_routes.rs`, `pages/ChangesPage.tsx`).

macOS sandbox, done: on macOS the commands' sandbox world (secrets hidden, protected files read-only, network
cut or not) is rendered as a Seatbelt profile run by `/usr/bin/sandbox-exec`, every path passed as a parameter;
the main checkout's dependency directories are linked into a worktree instead of mounted. Commands write their own
TMPDIR, not /tmp. Known gap: a protected file's parent directory can be renamed (`tools/sandbox/seatbelt.rs`,
`tools/sandbox.rs`, `turn_build.rs`).

Paths the human named, done: a file tool reading outside the project runs without an approval when the human
already wrote that path, or a folder above it, in a message or an answer of the session (`~/`, Windows
`\\wsl.localhost\<distro>\` paths included); writes, commands and the human's own Ask rules still ask
(`domain/named_paths.rs`, `agent/tool_calls.rs`).
