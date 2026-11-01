# fennec HTTP API (web UI ↔ server)

The server listens on `127.0.0.1:7777`. JSON in, JSON out. Errors are
`{"error": "<message>"}` with a 4xx/5xx status. Request bodies are limited to
32 MiB.

## Events

Every session is an append-only event log (`crates/fennec/src/domain/event.rs`),
the same wire format as openfox's `TurnEvent`: `{"type": "...", "data": {...}}`
with camelCase fields.

| type | data |
|---|---|
| `session.initialized` | `{workdir}` |
| `message.start` | `{messageId, role: "user"\|"assistant", content?, isSystemGenerated?, metadata?: {type: "agent", name, color, kind: "definition"}, attachments?: [{id, filename, mimeType, size, data}]}` (metadata set on the agent reminder the loop injects as a system-generated user message when the agent changes; attachments are images the human sent: png/jpeg/gif/webp/bmp/svg, 5 MB each, at most 10 per message, `data` is a base64 data URL) |
| `message.delta` | `{messageId, content, tokensPerSecond?}` (text chunk; the generation speed so far, estimated by the server from the characters streamed, absent in the first half second) |
| `message.thinking` | `{messageId, content, tokensPerSecond?}` (reasoning chunk, same estimate) |
| `message.done` | `{messageId, partial?, stats?}`; `stats` (assistant messages): `{model, totalTime (s), prefillTokens, cachedPromptTokens, generationTokens, prefillSpeed, generationSpeed}`, speeds in tokens/s, prefill speed over new (non-cached) tokens |
| `tool.call` | `{messageId, toolCall: {id, name, arguments, parseError?, rawArguments?}}`; `rawArguments` is the JSON text the model streamed, sent back to it verbatim (re-serialized, the keys would be sorted) |
| `tool.output` | `{messageId, toolCallId, stream: "stdout"\|"stderr", content}` |
| `tool.result` | `{messageId, toolCallId, result: {success, output?, error?, durationMs, truncated, metadata?: {mimeType, dataUrl, path, size}}}` (`metadata` when `read_file` returned an image) |
| `context.state` | `{currentTokens, maxTokens, compactAt?}` (`compactAt`: the turn's compaction threshold, 0 = off; absent in older events) |
| `context.compacted` | `{beforeTokens, afterTokens, summary}`: the cut where a new context window started from `summary`; `afterTokens` is an estimate until the next model call reports the real count |
| `running.changed` | `{isRunning}` |
| `chat.done` | `{messageId, reason: "complete"\|"stopped"\|"error"\|"truncated"\|"waiting_for_user", durationMs?}` (wall clock of the agent loop that ended; absent for compaction-only turns and older logs) |
| `chat.error` | `{error, recoverable}` |
| `chat.ask_user` | `{callId, question, type?: "text"\|"confirm"\|"choice", options?: [{value, label, description?}]}`: the model asked the human a question; the turn ends and the answer arrives as the next turn's input |
| `turn.snapshot` | `{cachedSystemPrompt, cachedTools?}`: the session's system prompt and tool definitions, frozen at its first turn (a session from before `cachedTools` gets a second snapshot that freezes them) |
| `mode.changed` | `{mode, auto, reason?}`: the human switched the session's agent; it applies from the next turn |
| `model.changed` | `{model: {provider, model}}`: the human chose the session's model (fennec's own); it applies from the next turn, and a workflow's child sessions inherit it |
| `session.rewound` | `{messageId}`: fennec's replay cut; the model's view drops that message and everything after it, the log keeps them |
| `session.name_generated` | `{name}`: the model-generated session title; the server renames the session and wakes the event stream |
| `workflow.started` | `{runId, workflow, task, stepByStep}`: `workflow` is the whole definition, frozen for the run |
| `workflow.node.started` | `{runId, nodeId, pass, via?, childSessionId?, startedAt?}`: `via` is `{from, port}`, the edge taken; `startedAt` is unix ms, so a running node's elapsed time survives a reload (absent in older logs) |
| `workflow.human.asked` | `{runId, nodeId, question}`: a human node's question with its `{{…}}` names filled in, right after the node starts |
| `workflow.node.finished` | `{runId, nodeId, port, output, exitCode?, durationMs?}`: `durationMs` is the node's working time measured by fennec (none for a human node); its tokens are in the usage ledger (`GET …/usage/workflow`) |
| `criteria.set` | `{criteria: [{id, description, check?}]}`: the criteria, all pending: the approved plan's, or a proposal the human accepted |
| `criteria.preflight` | `{messageId, results: [{criterionId, command, verdict, exitCode?, output}]}`: the plan's checks, run by fennec before any change at the end of the planner's turn (verified); `verdict` is `fails` (expected), `passesAlready` or `cannotRun`; the last two send the plan back to the planner once |
| `criteria.proposed` | `{callId, criteria: [{id, description, check?}], reason}`: an agent's corrected criteria (`propose_criteria`), shown with a `chat.ask_user` of the same `callId` whose question lists only what changes against the criteria in force (modified, added, removed, then the unchanged ids); the human's `Accept` answer records them as `criteria.set` |
| `decision.recorded` | `{callId, title, choice, reason, alternatives}`: a design decision an agent recorded with `record_decision` (indicative: produced by a model); the turn goes on |
| `criterion.updated` | `{criterionId, status}`; `status` is `{type: "pending"}` or `{type: "passed" \| "failed", provenance}`, `provenance` `{kind: "verified", command, exitCode?, output}` (fennec ran the check) or `{kind: "indicative", agent, reason}` (an agent's verdict) |
| `integration.applied` | `{branch, base, commits, head, message, acknowledged, protectedTests}`: the human integrated the session's branch; `message` is the integration commit message, `acknowledged` lists the blockers they accepted, `protectedTests` the test files they protected with it (absent in older logs) |
| `audit.tasks_created` | `{findings, ideaIds}`: the human turned audit findings into backlog ideas; `findings` are the report finding numbers, `ideaIds` the created ideas |
| `task.proposed` | `{callId, title, text}`: a backlog task an agent proposed with `propose_task` (indicative: produced by a model); the turn goes on |
| `task.accepted` | `{callId, ideaId}`: the human added the proposed task `callId` to the project's backlog as `ideaId` |
| `agents_md.written` | `{replaced}`: the human wrote the AGENTS.md draft of the last answer to the project's root; `replaced` when a file was there |
| `agent.suggested` | `{callId, agent, reason, brief}`: the `suggest_agent` tool: an agent suggests the human go on in a new session with another agent; nothing switches until the human opens it |
| `rule.blocked` | `{callId, ruleId, kind, pattern, scope, subject}`: fennec refused a tool call by one of the human's rules (fennec's own check, verified), before the call's failed `tool.result`; `kind` `command`/`tool`/`path`, `scope` `global`/`project`/`session`, `subject` the command, tool name or path refused (workdir-relative inside it, absolute outside) |
| `rule.approval_requested` | `{callId, subject, ruleId, kind, pattern, access, folders}`:; `folders` (a path only) the folders the human may grant instead, narrowest first, never `/` or `~/` fennec paused a tool call for the human (an "ask" rule, `ruleId` its id, or a file tool outside the workdir, `ruleId` null); the call stays open, no `tool.result`, and the turn ends `waiting_for_user`; `kind`/`pattern`/`access` are the rule the `session`, `always` and `never` answers store (`access` `read`/`write`/`readWrite` for a path, else null) |
| `rule.approval_answered` | `{callId, choice, ruleId}`: the human's answer (`once`/`session`/`always`/`never`), `ruleId` the rule it stored (null for `once`); fennec then appends the call's `tool.result` (run, or refused for `never`) and the turn goes on |
| `waiver.requested` | `{callId, path, reason}`: the agent's `request_waiver`: it asks the human to waive the protection of one project file; the call stays open, no `tool.result`, and the turn ends `waiting_for_user` (refused at once while the human is away) |
| `waiver.answered` | `{callId, granted, expiresAt, adr}`: the human's answer, recorded at the click; a grant exempts exactly `path` from protection (file tools and sandbox) in this session until `expiresAt` (ms), and `adr` is the MADR file written for it (`docs/adr/NNNN-waiver-<slug>.md` in the session's workdir); fennec then appends the call's `tool.result` and the turn goes on |
| `queue.added` | `{queueId, mode, content, queuedAt}`: a message typed while a turn runs; `mode` is `asap` (openfox's `completion` is not ported); `queuedAt` is epoch ms |
| `queue.drained` | `{queueId}`: the queued message was dropped into the running turn between tool rounds, or started the next turn; the resulting `message.start` / next job follows it in the log |
| `queue.cancelled` | `{queueId}`: the human removed the queued message, or a stop dropped the whole queue |
| `worktree.discarded` | `{branch, commits}`: the human discarded the session's worktree and branch; `commits` are the agent's commits that were dropped |
| `workflow.paused` | `{runId, from, port, to}`: waiting before `to` |
| `workflow.pause_requested` | `{runId}`: the human asked to pause at the next edge (after the current node) |
| `workflow.resumed` | `{runId, comment}` |
| `workflow.finished` | `{runId, outcome, reason?, stats?}`: `outcome` is `done`, `blocked`, `stopped` or `error`; `stats` are facts fennec computed from the log: `{workingMs, nodes: [{nodeId, label, runs, workingMs}], reruns, filesChanged, criteriaTotal, criteriaPassed, criteriaVerified}` (the time waiting for the human excluded; tokens are in the usage ledger) |

Tools: `read_file {path, offset?, limit?}`, `write_file {path, content}`,
`edit_file {path, old_string, new_string, replace_all?}`,
`run_command {command, cwd?, timeout?}`,
`ask_user {question, type?, options?}` and `propose_criteria {criteria, reason}`
(handled by the loop, the turn ends waiting for the human), and
`record_decision {title, choice, reason, alternatives?}` (handled by the loop,
the turn goes on; the result is `Decision recorded.`).

Agents: the session runs as one top-level agent, `plan` (`read_file`,
`load_skill`, `ask_user`: read-only) by default or `builder` (every tool),
defined by `*.agent.md` files (embedded defaults, `~/.agents/agents`, the
project's `.agents/agents`, later overriding by `id`); every tool definition
is always sent so the prompt prefix stays cached, and a call the agent does
not allow fails with `Tool '<name>' is not available in '<mode>' mode.
Available: ...`.

## Session state

`"running"` while a turn runs, else the reason of the last `chat.done`
(`"complete"`, `"stopped"`, `"error"`, `"truncated"`, `"waiting_for_user"`),
or `"idle"` if none.

## Settings

The Settings page stores `Preferences` in SQLite. Environment variables are
defaults; stored preferences override them per turn, without a restart.

A turn runs on the session's model (its last `model.changed`, else its parent
session's), else on `selected`, the default model of the Settings page.
`favorites` lists the models the composer offers first, in the human's order.
A model belongs to a provider. The provider `local` is the
environment's endpoint (`FENNEC_LLM_URL`, `FENNEC_API_KEY`); a stored entry
with id `local` overrides only what it sets. The model id is sent to the
provider as is: `local` never appears on the wire.

Precedence for a turn:

1. stored per-model value in `providers[provider].models[model]`
2. for `contextWindow` only, the local endpoint's startup `/models` listing
3. the environment/built-in default (`FENNEC_*`, or 128 000 tokens)

API keys (`apiKey`): `{"env": "VAR"}` names an environment variable read at
each call; `{"sealed": "<hex>"}` is a key encrypted (ChaCha20-Poly1305) with
the master key `secret.key` next to the store (mode 0600, hidden from the
sandbox with the store); `{"plain": "<key>"}` is accepted by `POST` only and
sealed before it is stored: a key in clear never reaches the database.

Preferences written before providers (`model`, `models` at the top) are read
as the `local` provider's, and written back in the new shape.

`Preferences` JSON (camelCase, all fields optional on read):

```json
{
  "selected": "{provider, model} | null",
  "providers": [
    {
      "id": "string, without '/'",
      "name": "string",
      "baseUrl": "string (empty for local: the environment's)",
      "apiKey": "{env} | {sealed} | {plain} | null",
      "models": {
        "<model id>": {
          "maxTokens": "number | null",
          "contextWindow": "number | null",
          "compactionThreshold": "number | null",
          "reasoningEffort": "\"none\" | \"minimal\" | \"low\" | \"medium\" | \"high\" | \"xhigh\" | \"max\" | null"
        }
      }
    }
  ],
  "display": {
    "reasoningOpen": true,
    "toolResultsOpen": false,
    "fontSize": 14,
    "panelPages": null
  },
  "globalInstructions": "string | null",
  "search": {
    "searxngUrl": "string | null",
    "searxngApiKey": "string | null"
  },
  "sandbox": {
    "enabled": true,
    "network": false
  }
}
```

Validation: `maxTokens >= 1`, `contextWindow >= 1024`,
`compactionThreshold` in `0.0..=1.0` (NaN rejected), model ids non-empty,
`fontSize` in `13..=16` (default 14), `panelPages` 1..=12 pages with
non-empty names ≤40 chars, unique names, and non-empty block ids (`null`
means the UI's default pages), `searxngUrl` blank or starting with `http://`
or `https://`. `reasoningEffort` is sent as the
`reasoning_effort` field of the model request when set; `null` omits it.

`globalInstructions` (openfox name) and the AGENTS.md files from the
filesystem root down to the session's working directory go into the system
prompt of a new session; an existing session keeps the prompt of its first
turn.

`search` (openfox `search.*`, SearXNG only) is the instance the `web_search`
tool queries (`GET {searxngUrl}/search?format=json&q=…`, with
`Authorization: Bearer {searxngApiKey}` when set). The tool runs in fennec,
not in a command: the model only chooses the query. `webFetch` (fennec's own, default true) allows the `web_fetch` tool, which also runs in fennec; the model chooses the URL, loopback and private addresses are refused.

`sandbox` (fennec's own) is how commands run: the `run_command` tool and the
workflow shell nodes. Enabled (the default), each command runs in bwrap: the
system read-only, the working directory writable, `/tmp` private, the secrets
under `$HOME` (`.ssh`, `.aws`, `.netrc`…) and fennec's data directory hidden,
the environment's credential-looking variables removed, every process ended
with the command, and no network unless `network` is true (npm and cargo are
then set offline). Disabled, commands run on the host, as in openfox.

Protected files: a model may read them, never write them. They are fennec's
defaults (`.agents`, `AGENTS.md`, `CLAUDE.md`, the lockfiles `Cargo.lock`,
`package-lock.json`, `yarn.lock`, `pnpm-lock.yaml`, the CI `.github`,
`.gitlab-ci.yml`, the release configuration `release-please-config.json`, `.release-please-manifest.json`, the git hooks `.githooks`, and the accepted ADRs `docs/adr`) plus the lines of the project's `.agents/protected`
(gitignore-like: `#` comments, `*`, `?`, `**`, a leading `/` anchors, a
pattern without `/` matches at any depth, a directory protects its content).
`write_file` and `edit_file` refuse them; in the sandbox they are mounted
read-only. The human edits them as usual.

## Endpoints

- `GET /api/config` → `{model, contextWindow, compactAt}`: the resolved parameters of the
  stored preferences; `compactAt` is the fraction at which the session compacts (0 disables)
- `GET /api/settings` → `{preferences, listings, defaults, skills, optInWorkflows, scanners, sandboxHealth}`
  - `scanners`: `[{id, name, covers, install, url, path}]`, the security scanners the `audit-security`
    agent runs; `path` is where fennec found the executable on PATH, null when absent (verified); fennec
    never installs one, `install` is the command shown to the human
  - `sandboxHealth`: `{status: ready|missing|blocked, detail}`, whether bwrap runs here, probed on each
    call (verified); `detail` is bwrap's error when blocked (on recent Ubuntu, AppArmor's user namespace
    restriction, see docs/releasing.md)
  - `preferences`: the stored `Preferences`
  - `listings`: `[{provider, name, baseUrl, models: [{id, contextWindow, resolved}], error}]`,
    local first, from a fresh `/models` call per provider (in parallel); the
    local one falls back to the startup listing; `error` is why a call failed.
    `resolved` is what a turn of that model runs with, display-ready
    (`{maxTokens, contextWindow, compactAt, reasoningEffort}`, each
    `{value, source: perModel|listed|default, wire}`): `wire` marks what a
    call sends to the provider (`maxTokens` always; `reasoningEffort` when
    set; `contextWindow`/`compactAt` are local).
  - `defaults`: `{provider, model, maxTokens, contextWindow, compactAt, reasoningEffort}`
    from the environment/built-in defaults
  - `optInWorkflows`: `[{id, name, description}]`, the embedded workflows that stay off
    until their id is in `preferences.enabledWorkflows`
- `POST /api/settings` with a full `Preferences` body → 200 with the same body
  as `GET /api/settings`; 400 with the validation message if invalid
- `POST /api/settings/providers/list` with `{provider, baseUrl, apiKey}` → a
  single listing in the `listings` shape (with `resolved` per model). Re-lists
  one provider's `/models` now, using the request's `baseUrl`/`apiKey` when
  they are not (yet) stored, so a new provider can be probed before saving;
  empty parts fall back to the stored/env endpoint; the stored key is reused
  only for the stored `baseUrl`, never sent to another one
- `POST /api/settings/models/probe` with `{provider, baseUrl, apiKey, model}`
  (endpoint resolved as for `providers/list`) → `{contextWindow, contextSource,
  maxTokens, reasoningEfforts, effortsSource, defaultReasoningEffort}`
  (openfox auto-config). One-token calls: a baseline, one per effort
  (accepted, deduped by `prompt_tokens`), one with an oversized `max_tokens`
  whose refusal may name the limits. A source is `endpoint` or `catalog` (the
  model family's known values, `llm-providers/src/catalog.rs`). Indicative;
  502 with the endpoint's message when the model cannot be called
- `GET /api/models/choices` → `{default, favorites}`, each a
  `{provider, model, providerName}`: what the composer's model picker offers;
  read from the stored preferences, no provider is called
- `GET /api/projects` → `[{id, name, workdir, branch, remote, editorUrl, sessions: [{id, title, state, createdAt, updatedAt}]}]`
  (projects most recently active first, sessions newest first; `createdAt`/`updatedAt` are Unix milliseconds; `branch` and `remote` may be `null`; `editorUrl` opens the workdir in VS Code, through the WSL remote under WSL)
- `POST /api/projects` `{workdir}` → the project (400 if not a directory)
- `POST /api/projects/{id}/delete` `{deleteFolder?}` → `{deleted: true, folderDeleted, folderError}`: deletes the project with every top-level session as `POST /api/sessions/delete` does (logs, children, worktrees, branches, session rules), then its project rules, its ideas backlog and the project. With `deleteFolder: true` the source folder is removed from disk afterwards; a failure there comes back as `folderError` (the project is already gone). 404 for an unknown project; 409 `{error}` while its queue runs, `{error, running}` while one of its sessions runs a turn, `{error, sessionId}` when a worktree cannot be removed (the project and the sessions not yet removed stay); 400 when the folder is the filesystem root, the home directory or an ancestor of it, or holds fennec's store (nothing is deleted then). The usage ledger keeps the calls.
- `GET /api/backlog` → `{total, projects}`: the cross-project Backlog page, one row per project (the idea lists
  live in each project's Tasks tab):
  - `total` `{yours, running, ready, drafts, done, projects}`: counts across all projects; `ready` sums each
    project's `queue.ready`, `projects` counts every project
  - `projects` `[{project, yours, running, ready, drafts, done, queue}]`: the projects with at least one idea or a
    running queue; `project` `{id, name}`, the counts are its board's stages, `queue` `{running, autonomous, next}`
    (`next` `{id, text, title} | null`); a running queue first, then most `yours`, most `ready`, then by name
- `GET /api/projects/{id}/commands` → `[{name, description}]`: the slash commands the composer offers (`compact`, then the project's skills); 404 if no project
- `GET /api/projects/{id}/agents` → `[{id, name, description, color, subagent, phase}]`: the
  project's agents ordered by `phase` (`start`, `frame`, `build`, `understand`, `check`; null last, for
  sub-agents and agents without one), discovery order within a phase (`color` null when unset); the modes are the
  ones with `subagent` false, the others only run in a workflow node's fresh context;
  404 if no project
- `GET /api/projects/{id}/overview` → `{map, mapFile, drift}`, the project page (404 if no project):
  - `map` `{modules: [{id, files, tests, entry, layer, row}], dependencies: [{from, to, imports, violation}]}`:
    the project map, read from the import lines of the files git tracks (Rust `crate::` and workspace
    crates, TS/JS relative imports; test files and modules skipped); a module is a directory (`.` the root),
    `imports` counts the importing files; `layer`/`row` place it (layers by dependency depth): the UI
    computes nothing. Deterministic (verified).
  - `mapFile` `missing | stale | current`: the project's `docs/map.md` against a fresh render of `map`.
  - `drift` `{rules, violations: [{from, to, imports, rule}], sinceMap}`: the produced architecture against the
    intended one (verified). `rules` counts the module rules of `.agents/architecture` (`<A> never imports <B>`, a
    module matching `A` or below it); `violations` are the map's dependencies breaking one (`rule` its display text,
    the dependency's `violation` set), in the map's order. `sinceMap` `{added: [{from, to}], removed: [{from, to}]}`
    compares the dependencies with those of the committed `docs/map.md` (sorted; null when the file is absent).
- `GET /api/projects/{id}/dashboard` → `{tiles, recent}`, the project page's overview tab (404 if no project):
  - `tiles` `[{id, icon, label, figure, detail, tone, active, href, title}]`, in display order (`sessions`,
    `ideas`, `tokens`, `time`, `cost`, `disk`, `map`, `doc`); every string is final display text. Usage over
    the last 30 days from the ledger; `disk` only reads the cached worktree scan (`—` until one ran), never scans.
  - `recent` `[{id, title, updatedAt, running}]`: the 5 top-level sessions updated last, newest first.
- `GET /api/projects/{id}/docs` → `{files, adrs}`, the project's documentation, read from disk (verified):
  - `files` `[{path, name, content}]`: the project's markdown (`AGENTS.md`, `README.md` and `docs/`, its
    ADRs excluded), sorted by path; `path` forward slashes, `name` the file name.
  - `adrs` `[{number, file, title, status, date, body}]`: the MADR files of `docs/adr/` (`NNNN-slug.md`,
    sorted by number; `status` and `date` from the frontmatter, null when absent; the title from the
    first `# ` heading).
- `GET /api/rules` → `{global, projects: [{id, name, rules}], sessions: [{id, title, projectId, rules}]}`, every rule of
  the rules crate's own store (`rules.db`, `FENNEC_RULES_DB`): global ones, per project, and the session grants;
  `POST /api/rules` `{scope: "global" | {project: id}, kind, pattern, access?, action, note?}` adds one → 201 `{rule}`
  (400 with `{error}` when the pattern does not parse: empty, flags only, `..`, over 500 characters; 404 if no project);
  `POST /api/rules/{id}/delete` → `{ok}` (404 if none). A rule is never edited: delete it and add another. No agent tool
  reaches these routes.
  - a rule `{id, scope, kind, pattern, access, action, note, origin, understood}`: `scope` `global`/`project`/`session`;
    `kind` `command` (matched on parsed tokens: program, words in order, flags as a set, through `sudo`, `env`, `sh -c`,
    `&&`, `|`, `find -exec`; block and ask rules match a superset, allow rules only the exact command), `tool` (a tool
    name) or `path` (gitignore-like, relative to the project or absolute / `~/…`; `access` `read`/`write`/`readWrite`,
    null otherwise); `action` `allow`, `ask` (refuses too, until the approval flow) or `block`; `origin` `"human"` or
    `{session}` (made by an answer in that session); `understood` how the server read the pattern, for display.
  - Precedence: block > allow > ask, then the narrowest scope. A file tool outside the workdir needs an allow rule; block
    write rules relative to the project are also mounted read-only in the sandbox.
  - Rules load once per turn: a change applies from the next turn. They are checked before every agent tool call and
    every workflow `shell`/`criteria` command.
- `GET /api/projects/{id}/rules` → `{rules, global}`, the project's rules and the global ones that apply to it too
  (read-only: the Settings page's "Règles" tab, `#/settings/rules`, edits them).
- `GET /api/projects/{id}/risks` → `{files, unreviewedPercent, untested, churning, unreviewed, churnDays, modules, remedyPrompt}`, the
  project's risk report, read from git (verified; 404 if no project). `files` counts the tracked source files, tests
  excluded; `unreviewedPercent` the share of their lines (git blame) from AI commits never reviewed: a commit with an
  AI `Co-Authored-By` trailer and no `Fennec-Session` trailer (written only by an integration, the human's review gate).
  - `modules` `[{path, unreviewed, untested, churning, zones}]`: risk zones grouped by the zone file's parent directory
    (`.` for a file at the root), riskiest module first (most total signals, then path); the counts are the number of
    its zones carrying that signal, and `zones` keeps the report's zone order (`{path, signals, churn, lines,
    unreviewedPercent}`, most signals first, then churn; `signals` among `unreviewed`, `untested`, `churn` as before).
  - `remedyPrompt`: the brief the "Remédier" button sends to the `remedy` agent; empty without zones.
- `GET /api/projects/{id}/worktrees[?fresh=1]` → `{scannedAt, totalBytes, generatedBytes, entries}`: the disk the
  project's session worktrees take, measured from disk (verified; allocated bytes, like `du`). `fresh=1` measures again,
  otherwise a scan younger than 10 minutes is reused; 404 if no project.
  - `entries` `[{name, sessionId, sessionTitle, running, updatedAt, bytes, generatedBytes, orphan, cache, repoMissing,
    foreignOwned, sudoCommand}]`, largest first: `name` the directory under the worktrees root (the root session's id);
    `generatedBytes` the part in `target`, `node_modules` and `.venv` (rebuilt by a build); `orphan` when no session
    owns it or git no longer registers it; `cache` for `.cargo-target`, the Cargo target directories every
    project's worktrees share (all of it `generatedBytes`, shown on every project page, emptied by `clean`, never an orphan); `repoMissing` when its repository is gone (such orphans show on every
    project page); `sudoCommand` (`sudo rm -rf <path>`) for an orphan holding files of another user (root).
- `GET /api/worktrees` → `{totalBytes, generatedBytes, alert, largestProjectId}`: every worktree, for the home page;
  `alert` from 5 GiB; `largestProjectId` the project whose worktrees take the most, null when none.
- `POST /api/worktrees/clean` `{name}` → `{freedBytes}`: the human's click empties a worktree's generated directories
  (`target` removed, `node_modules`/`.venv` emptied, they are the sandbox's mount points); 400 invalid name, 404 no such
  worktree, 409 while a turn of its session runs or when removing fails (the message names the `sudo` command).
- `POST /api/worktrees/remove-orphan` `{name}` → `{}`: removes an orphan directory and what git keeps of it (its
  registration, `fennec/<name>` branch, base and origin refs); 409 for a session's live worktree (use
  `POST /api/sessions/{id}/discard`) or when removing fails.
- `POST /api/projects/{id}/map-file` `{}` → `{mapFile}`: the human's click writes `docs/map.md` (a Mermaid
  diagram and a table) in the project's tree, not committed; 404 if no project
- `GET /api/projects/{id}/ideas` → `{queue, groups, nightReport}`: the project's ideas backlog as a board the UI renders
  as is, folded (crate `backlog`) from its append-only `idea.*` events (table `project_events`), dropped ideas left out.
  - `queue` `{running, autonomous, ready, next: {id, text} | null, caps: {maxMinutes, maxTokens}}`: whether the queue
    runs (and in night mode), how many ideas it holds, the idea it starts next, the Settings caps (0 no cap).
  - `groups` `[{stage, ideas: [idea]}]`, all five groups always sent, in this order: `yours` (waits for the human: review, question,
    halted, failed), `running`, `queue` (in queue order), `drafts` (inbox, framing), `done` (integrated); outside the
    queue, the most recently changed first.
  - `idea` `{id, text, attachments, status, stage, action, menu, sessionId, plan, halted, autonomous, position, createdAt, startedAt,
    durationMs, tokens, criteria, decisions, commits, files}`: `status` `inbox | framing | queued | running | question |
    review | halted | failed | integrated | completed` (from the idea and its session's state; a started idea whose
    session is gone is `failed`); `attachments` `[{filename, mimeType, size, data}]` the files and images pinned to the
    idea (a base64 data URL in `data`; handed to the agent on framing and build: images as vision, text inlined,
    binaries in the worktree); `action` the one next step `frame | resumeFraming | start | open | answer | review`;
    `menu` the secondary actions `restart | reframe | startUnframed | complete | drop` (empty for running, integrated
    and completed); `plan` the approved plan verbatim; `halted` why the night mode stopped or skipped it, cleared by a
    new plan or start; `position` its 1-based place in the queue; `durationMs` from its start to now (running) or its
    session's last change; `tokens` uncached prompt plus completion of the project's calls in that window (exact while
    one idea runs at a time); `criteria` `{passed, failed, pending}` and `decisions` `[{title, choice}]` from its
    session's run; `commits` and `files` of its worktree branch past its base, for `review`, `halted` and `failed` only.
    Null when not applicable.
  - `nightReport` the last queue run once finished and until read, if it started an idea: `{startedAt, finishedAt,
    reason, autonomous, summary: {started, running, review, question, halted, failed, integrated}, ideas: [idea]}`;
    `reason` `no idea ready | stopped by the human | project gone | server restarted | error: …`.
  404 if no project. Every write below answers the same fresh board; 404 if no project or no such idea:
  - `POST /api/projects/{id}/ideas` `{text, sessionId?, plan?, attachments?}` (`idea.added`; 400 if empty or over 4000
    characters; `attachments` is `[{filename, mimeType, size, data}]`, at most 10, 5 MB each);
    with `sessionId` (a session of the project, else 404) and `plan` (400 if empty), also `idea.framing` and `idea.ready`:
    the plan card's "Mettre au backlog" for a plan written outside the backlog
  - `POST /api/projects/{id}/ideas/{ideaId}/framing` `{sessionId}` (`idea.framing`; 404 if the session is not the project's)
  - `POST /api/projects/{id}/ideas/{ideaId}/ready` `{plan}` (`idea.ready`; 400 if empty)
  - `POST /api/projects/{id}/ideas/{ideaId}/edit` `{text}` (`idea.edited`; 400 if empty or over 4000 characters): the human rewrites the idea's text; a dropped or completed idea keeps its text
  - `POST /api/projects/{id}/ideas/{ideaId}/drop` `{}` (`idea.dropped`): abandons an idea, or removes a done one from the board (menu `remove`)
  - `POST /api/projects/{id}/ideas/{ideaId}/complete` `{}` (`idea.completed`)
  - `POST /api/projects/{id}/ideas/{ideaId}/start` `{}`: builds this idea now, outside the queue: from its plan
    when ready, or when restarting a halted/failed idea that has a plan, else from its text (the workflow plans first;
    an idea without a session gets one, `idea.framing`), then `idea.started {ideaId, autonomous: false}`; 409 if
    already started and not halted/failed, or its session is busy
  - `POST /api/projects/{id}/ideas/reorder` `{ids}` (`idea.reordered`): the human's queue order; ids left out follow,
    oldest first; 400 unless a list of queued idea ids
  - `POST /api/projects/{id}/ideas/report/read` `{}` (`idea.report_read`): hides the night report
  - `POST /api/projects/{id}/ideas/queue/start` `{autonomous?}`: starts the queue (no-op if it runs), recorded as
    `idea.queue_started {autonomous}` … `idea.queue_finished {reason}`. It takes the first queued idea not halted,
    starts the `plan-build-verify` workflow from its plan in its session's worktree (`idea.started {ideaId,
    autonomous}`), waits for the turn to end (done, waiting for the human, or failed), then the next. The queue lives in
    the server: a restart closes an open run (`server restarted`). `autonomous` (night mode): `ask_user` answers itself
    (the agent decides and records it with `record_decision`), `propose_criteria` is refused (the criteria stay the
    human's), in the session and its child sessions; an idea over the Settings caps (`autonomy.maxMinutesPerIdea`,
    `autonomy.maxTokensPerIdea`, checked every 30 s) is stopped, and an idea that fails to start is skipped, each with
    `idea.halted {ideaId, reason}`
  - `POST /api/projects/{id}/ideas/queue/stop` `{}`: the queue starts no further idea; the running turn finishes
- `GET /api/folders?path=<absolute path>` → `{path, parent, folders: [{name, path, hidden, repo}]}`,
  the sub-folders of `path` (the home directory when omitted), sorted by name;
  `repo` marks a git repository; 400 if `path` cannot be read
- `POST /api/sessions/{id}/audit/tasks` `{findings: [number]}` → `{created}`: adds one idea per finding of the
  session's audit report to its project's backlog, the text written by the server; 400 when a number is unknown;
  409 when a finding is already a task; records `audit.tasks_created` in the session's log;
- `POST /api/sessions/{id}/proposals/{callId}/accept` `{}` → `{ideaId}`: adds the task proposed by the call
  `callId` to the session's project backlog (title, blank line, text); 404 when no such proposal; 409 when already
  accepted; records `task.accepted` in the session's log;
- `POST /api/sessions/{id}/agents-md` `{}` → `{replaced}`: writes the last answer's AGENTS.md draft to the project's root; 400 when the last answer has no draft; 409 when AGENTS.md is a symlink; records `agents_md.written`;
- `GET /api/sessions/{id}` → `{id, title, state, mode, model, project: {id, name, workdir}, worktree, idea, request, audit, proposedTasks, agentsMdDraft, agentSuggestion}`;
  `idea` is `{id, state}` when the session frames one of the project's ideas, else null;
  `audit` is the session's last answer read as an audit report, `{scope, summary, findings: [{number, title,
  severity: high|medium|low, principle, locations: [string], problem, fix}], taken: [number]}`, or null when it is
  not one; `taken` lists findings already turned into tasks since this report;
  `proposedTasks` is `[{callId, title, text, ideaId}]`, the tasks proposed in the session in order, `ideaId` null
  until the human accepts one;
  `agentsMdDraft` is `{exists, written}` when the last answer holds an AGENTS.md draft (`## AGENTS.md draft` then a ```md block), else null;
  `agentSuggestion` is `{callId, agent, name, color, reason, brief}`, the last `agent.suggested` since the human last wrote, while that agent exists as a mode, else null;
  `request` is the first message the human typed in the session, verbatim, or null;
  `model` is `{provider, model, providerName, contextWindow, compactAt, source}`,
  what the next turn runs on; `source` is `session`, `parent` or `default`;
  `worktree` is `{path, branch, base, editorUrl, commits}` (`commits`: the agent's commits past the base, 0 when there is nothing to integrate; `editorUrl` opens the copy in VS Code, through the WSL remote under WSL) when the session (or its root session) runs a workflow in
  its own git worktree, else null; the human takes the agent's work with
  `git cherry-pick <base>..<branch>`;
  `mode` is the id of the agent the session resolves to from its event log
  (null when there is no top-level agent)
- `GET /api/sessions/{id}/repo` → `{dir, branch, originBranch, branchChanged, dirty}`: the human's repository now, for any session: its top directory, checked-out branch (null when detached), the branch the session's worktree started from (null without a worktree), whether they differ, and the count of uncommitted entries (`git status --porcelain`); 409 outside a git repository.
- `GET /api/sessions/{id}/integration` → `{branch, base, originBranch, humanBranch, defaultMessage, commits: [{sha, subject}], files: [{path, protected, carried, added, removed, signals}], testFiles, criteria, decisions, blockers, idea}`:
  what integrating the session's worktree branch would do (409 without a
  worktree). `files` come riskiest first (the review order): `signals` ⊂
  protected, sensitive, bypass, architecture, conflictMarker, testsWeakened (the gate's
  own checks, per file), hot (5+ commits in 90 days), untested (no inline
  test, no test file of the same stem on the branch), large (200+ lines
  changed); deterministic. `originBranch` is the branch the run started from (null for
  older worktrees or a detached HEAD). `decisions` are the session's
  `decision.recorded` events, `{callId, title, choice, reason, alternatives}`:
  informational (written by the model), never a blocker. `defaultMessage` is fennec's starting
  commit message (`feat: <title>`, an empty title gives `feat: integrate the
  agent's work`); the human edits it. `idea` is the idea tied to the session that the human may finish here (state started), null otherwise. `blockers` is the review gate, each
  `{kind, …}`: `noCommits`, `conflict {files}` (the merge onto the human's HEAD would conflict on these files; never acknowledged: the branch must be brought up to date first), `branchChanged {from, to}`, `localChanges {paths}` (uncommitted changes in the human's tree the agent's work would overwrite: content equal to neither the base's nor the branch's; the common case, the same dirty work copied into the worktree at the run's start, is not a blocker but a file flagged `carried`, whose uncommitted work goes into the integration commit; never acknowledged: the pick refuses to overwrite it, commit, stash or remove it first), `criteriaNotPassed
  {criteria}`, `noCriteria`, `protectedTouched {paths}`, `sensitiveTouched {paths}` (changed files inside the sensitive zones of `.agents/architecture`, one `sensitive <pattern>` per line, gitignore-like: the human's review is mandatory there; deterministic), `architectureViolated {violations: [{path, line, prefix, forbidden}]}` (added lines that break the project's `.agents/architecture` rules, one `<path prefix> forbids <text>` per line; deterministic, checked on added lines only), `suppressionsAdded {suppressions: [{path, line, marker}]}` (added lines that silence a lint, a type check or a test: `#[allow(`, `@ts-ignore`, `noqa`, `it.skip(`, `pytest.mark.skip`…; text tripwires, comments included), `testsWeakened {testsRemoved, assertionsRemoved, paths}` (the branch removes more test declarations or assertions than it adds, summed over all files so moved tests do not count; `paths` are the files that lost some); `branchChanged.to`
  is null for a detached HEAD; `testFiles` are the integrated test files (paths
  relative to the project) that the protection does not cover yet; protection is read from the project's tree, not
  the worktree's.
- `POST /api/sessions/{id}/integrate` `{acknowledged: [kind], message, protectTests?, removeWorktree?}` → `{head, commits, protectedTests, protectError, worktreeRemoved, removeError}`:
  the human's decision. The server recomputes the gate and answers 400 on a
  blank `message`, 409 while a turn runs, while the human has staged changes,
  while a blocker is not acknowledged (`noCommits` and `conflict` never are), or when git
  fails; then it merges `<base>..<branch>` onto the human's HEAD (plus the
  `carried` files' uncommitted content) with `git merge-tree`, makes one
  commit with `message` plus a `Fennec-Session: <session id>` trailer, under
  the human's git identity, and moves the human's branch and tree to it with
  `git reset --keep` (other uncommitted work stays). A conflict leaves the
  repository untouched and answers 409
  `{error: "conflict", conflict: {kind: "conflict", files, command}}`. On
  success it moves the base ref to the branch head and appends
  `integration.applied`. With `protectTests: true` (default false, the human's
  choice) the `testFiles` are then appended to the project's `.agents/protected`
  under `# tests validated by integration`; `protectedTests` lists them, and a
  write failure leaves the integration done and comes back in `protectError`.
  With `removeWorktree: true` (default false) the session's worktree, branch and
  refs are then removed as `discard` does (`worktree.discarded`, no commits); a
  failure comes back in `removeError`.
  Nothing else in fennec integrates: no agent, workflow
  or schedule calls it.
- `POST /api/sessions/{id}/discard` `{}` → `{commits}`:
  the human's decision; 409 while a turn runs or without a worktree; removes the
  worktree, its branch and base ref in the human's repository and appends
  `worktree.discarded`; the session's idea, still started, is dropped (`idea.dropped`). A later workflow in the
  session opens a fresh worktree.
- `POST /api/sessions/{id}/resolve-conflict` `{}` → `{conflicts, started}`:
  brings the session's branch up to date with the human's HEAD as one commit
  (the base ref moves to that HEAD, conflict markers are left in the files);
  409 while a turn runs or a workflow waits. When conflicts remain the session
  switches to `builder` (`mode.changed`, reason `conflict`) and a turn starts
  with a prompt naming the conflicted files; `started` is true then.
- `POST /api/sessions/{id}/terminal` → 202 `{path}`: opens the human's own
  terminal in the session's folder (its worktree, else the project): Windows
  Terminal under WSL, Terminal.app on macOS, else `$TERMINAL` or
  `x-terminal-emulator`. Outside the sandbox: the human's action. 500 if it
  cannot start.
- `POST /api/sessions/{id}/files` → 202 `{path}`: opens the human's own file
  manager in the session's project folder (their repository, not the worktree):
  Explorer under WSL, `open` on macOS, else `xdg-open`. 500 if it cannot start.
- `GET /api/sessions/{id}/servers` → `[{name, command, running, url}]`: the project's
  servers (`.agents/servers.json`) and whether each runs in this session; `url` is
  `http://127.0.0.1:{port}` when it runs in this session, `null` otherwise. 404 if no session.
- `POST /api/sessions/{id}/servers` `{servers: [{name, command}]}` → the same list:
  saves the project's servers to `.agents/servers.json`. 400 on an empty name/command or a
  duplicate name, 404 if no session.
- `POST /api/sessions/{id}/servers/start` `{name}` → 202 `{pid, port}`: starts the named server
  (from the project's `.agents/servers.json`) in the session's worktree when it has one, after
  linking its missing dependency directories to the main checkout's copies; otherwise in the
  project's directory. It passes a free `PORT` and runs as `setsid sh -c <command>`, so a stop
  frees the port by killing the whole process group. Outside the sandbox: the human's action.
  404 for an unknown server, 409 if it already runs, 500 if it cannot start. A server that exits
  on its own is dropped from the running state.
- `POST /api/sessions/{id}/servers/stop` `{name}` → `{}`: stops the server by killing its
  process group. 404 if it is not running.
- `POST /api/sessions/delete` `{ids}` → `{deleted: [id], failed: [{id, error}]}`: deletes top-level sessions with their child sessions and logs, and removes each one's worktree, branch and refs (work not integrated is lost; the UI warns). 400 for an empty list or a child session, 404 for an unknown id, 409 `{error, running}` when one of them runs a turn (nothing is deleted then). A session whose worktree cannot be removed is kept and listed in `failed`. The usage ledger keeps the calls.
- `GET /api/sessions/{id}/skills` → `{loaded: [ids], active: [ids]}`: the
  skills the session loaded (successful `load_skill` calls and `/<skill>`
  messages it typed, first-seen order) and those still in the model's view
  (not dropped by a compaction, not cut by a replay); 404 if no session
- `GET /api/sessions/{id}/usage` → `{calls, promptTokens, cachedTokens, completionTokens, cachePercent, cost, costDetail, inputCost, cachedCost, outputCost, lastGenerationSpeed}`:
  the session's model calls from the usage ledger (`usage.db`, crate `token-usage`), which outlives the log;
  `cachePercent` is 0 to 100, `lastGenerationSpeed` (tokens/s) is null before any call;
  `cost` is the estimate at each model's price (Settings, models), `—` when none has one, indicative;
  `costDetail` says how or why not; `inputCost` (all prompt tokens, as `promptTokens`), `cachedCost` and
  `outputCost` are the parts' costs, null when no model has a price; 404 if no session
- `GET /api/sessions/{id}/context` → `{percent, tone, compactMark, line, title, composition}`: the header's gauge, from
  the last `context.state` the model still sees (`tone` `ok`/`warn`/`err`, `compactMark` null when compaction is off);
  events without `compactAt` and sessions without a measure use the current settings. `composition` (null while
  nothing is measured) is what the context holds: `kinds` `[{kind, label, tokens, percent}]` for the stacked bar
  (`kind` `prompt`/`conversation`/`tools`, fixed order), `parts` `[{kind, label, tokens, percent}]` largest first
  (the instruction files, the tool definitions, each tool's results, the reasoning…; past 12, one row with `kind`
  null sums the rest), `images` a note or null. The exact token count is spread over the parts by their share of
  the text: indicative, and said so
- `GET /api/sessions/{id}/vision` → `[{label, anchorId, src, inContext}]`: the images the model received over the
  whole log, in log order; `inContext` false once a compaction, a replay or the image window (4 to 6 latest images)
  took it out of the model's view; deterministic; 404 if no session.
- `GET /api/sessions/{id}/usage/calls` → `[{messageId, line, detail, turnRecap}]`, oldest first: what sits under
  each answer, written by the server from the ledger (`line` `↑ 12,3k · cache 82 % · ↓ 345 · 4,2 s`, `detail` the
  tooltip, `turnRecap` on a turn's last call only, null otherwise: the turn's wall clock, `chat.done.durationMs`
  with its tools, over the tokens its calls generated); ledger rows stored before the `messageId` label
  existed are labelled at server start
- `GET /api/sessions/{id}/usage/workflow` → `{nodes: [{itemId, line, detail}], runs: [{runId, calls, generated, read, cache}]}`: the
  tokens of each workflow node run (`itemId` is `wf-{runId}-{nodeId}-{pass}`, its child session's calls or the
  session's own between its start and end) and of each finished run, from the ledger, ready to display.
- `GET /api/insight?days=<7|30|90|all>&project=<id>` → `{cards: [{label, value, detail}], days: [{day, label,
  cachedPercent, freshPercent, lostPercent, title}], projects: [row], models: [row], consumers, projectOptions: [{id, name}]}`,
  `row` being `{key, name, calls, input, cache, output, lost, cost, time}`: the Insight page from the ledger, so deleted
  sessions still count; `cost` and the last card are an estimate at each model's own price (Settings, per model), "—"
  for a model without a price; the card lists the unpriced models
  (`{inputPerM, cachedPerM, outputPerM}`, dollars per million tokens, default 3 / 0.30 / 15), indicative; `days` defaults to 30 (400 otherwise), `project` is optional (`projects` is then empty); bar
  percentages are against the busiest day's input. A lost cache is a call that re-read more than 1,000 tokens of
  its session's previous prompt without that prompt having shrunk (a compaction or a replay is not a loss).
  `consumers` (null before any call was broken down) has the shape of `/context`'s `composition`: each call's input
  tokens spread pro rata of its prompt's text over its parts (ledger breakdown keys `<kind>/<label>`), summed over
  the period; indicative.
- `POST /api/sessions` `{projectId, prompt, mode?, model?, attachments?}` → `{id}`;
  `model` is `{provider, model}` (400 if empty or the provider is unknown),
  recorded as `model.changed` before the first turn;
  the turn starts at once; `mode` is optional, validated against the
  project's agents before the session is created (400 if unknown) and applied
  before the first turn; `attachments` is `[{filename, data}]` (data a
  base64 data URL; images only, 5 MB each, at most 10); `prompt` may be empty
  when attachments are present (400 `empty prompt` only if both are empty); a
  new session's title is the first attachment's filename when the prompt is
  empty; `/compact` takes no attachments (400)
- `POST /api/sessions/{id}/messages` `{content, attachments?}` → 202; 409 if a
  turn is running; when the session waits for the user, `content` is the
  answer to the pending question; `attachments` is `[{filename, data}]` (data
  a base64 data URL; images only, 5 MB each, at most 10); `content` may be
  empty when attachments are present (400 `empty message` only if both are
  empty); when answering a pending question, attachments ride a user message
  after the answer
  A prompt or message starting with `/compact` alone compacts the session
  instead; `/<skill> [text]` asks the model to load that skill; any other text
  is sent as typed. `/compact` takes no attachments (400).
- `POST /api/sessions/{id}/rule-approval` `{callId, choice, pattern?}` → 202; 404 if no
  session; 409 if a turn is running or `no pending approval for this call`; 400
  for an unknown choice, a `pattern` not among the request's `folders`, or a pattern the rules store refuses. `session` stores an
  allow rule for the session, `always` an allow rule and `never` a block rule for
  the project (origin: the answer), then a turn records `rule.approval_answered`,
  runs or refuses the recorded call and resumes the loop.
- `POST /api/sessions/{id}/waiver` `{callId, grant, hours?}` → 202; 404 if no
  session; 409 if a turn is running or `no pending waiver for this call`; 400 when
  a grant has no `hours` between 1 and 168. A grant writes the waiver's ADR into the
  session's workdir (refused through a symlinked `docs/` or `docs/adr`); the answer is
  recorded as `waiver.answered`, then a turn records the call's result and resumes the loop.
- `POST /api/sessions/{id}/queue/{queueId}/cancel` `{}` → 200; 404 `no such
  queued message` when the id is not in the session's pending queue; appends
  `queue.cancelled`.
- `POST /api/sessions/{id}/messages` while a turn runs: 202 `{"queued": queueId}`
  when the message is queued; 409 `images wait for the end of the turn` with
  attachments. During a workflow run, the run's same-session agent nodes drop it
  in between tool rounds; what is left waits for the run to end. If the turn
  ended in between, the request falls through to the normal start.
- `POST /api/sessions/{id}/replay` `{messageId}` → 202; 404 if no session;
  409 if a turn is running; 400 `only a message you typed in the current
  window can be replayed` when the id is not a user-typed message in the
  model's current view; appends `session.rewound` and re-sends that message
  with its images. The model's view drops the message and everything after
  it, the log keeps them; files the agent changed meanwhile are not reverted
- `POST /api/sessions/{id}/compact` → 202; 404 if no session; 409 if a turn is
  running; summarises the history and opens a new context window
- `POST /api/sessions/{id}/mode` `{mode}` → 200 `{mode}`; 404 if no session;
  400 with `unknown mode: <mode>` if the id is not a top-level agent; appends
  `mode.changed` when the mode changes; allowed while a turn runs
- `POST /api/sessions/{id}/model` `{provider, model}` → 200 `{model}` (the
  session's `model` as in `GET /api/sessions/{id}`); 404 if no session; 400 if
  the model is empty or the provider unknown; 409 while a workflow runs;
  appends `model.changed`
- `POST /api/sessions/{id}/stop` → 202; the turn ends with `chat.done` reason `"stopped"`
- `GET /api/notices` → Server-Sent Events. One event when a top-level session's turn ends, so the UI can
  notify the human wherever they are in fennec; child sessions stay silent. Each event is
  `data: {"sessionId": ..., "title": ..., "projectId": ...}`; no backlog is sent on connect.
- `GET /api/sessions/{id}/events` → Server-Sent Events. Each stored event is sent as

  ```
  id: <seq>
  data: <event JSON>

  ```

  from `?after=<seq>` (the start of the log when omitted), then live. On
  reconnect the browser sends `Last-Event-ID` and the stream resumes after
  it.
- `GET /api/sessions/{id}/events?limit=<n>&before=<seq>` → JSON
  `[{seq, event}]`, ascending: at least the last `n` events before `before`
  (the log's end when omitted), extended back to the `running.changed` that
  started their turn, so a page never starts mid-turn and can exceed `n`.
  Older history exists while the first `seq` is above 1. `limit` must be in
  `1..=10000` (400 otherwise); 404 if no session; an empty array when there
  is nothing before `before`.
- `GET /api/sessions/{id}/workflow/run?before=<seq>` → JSON `[{seq, event}]`,
  ascending: the `workflow.*` events of the latest run started before
  `before` (the log's end when omitted), from its `workflow.started`; empty
  when none. The UI loads it once with the first page's first `seq`, so a run
  that started before the loaded page keeps its strip.
- `GET /api/sessions/{id}/preview/<path>` → the file at `<path>` relative to
  the session's project, for the preview; 404 outside the project or for a
  missing file, 413 above 10 MiB; always sent with
  `Content-Security-Policy: sandbox allow-scripts`, so a page runs in an
  opaque origin whose requests the guard refuses.

## Workflows

A session summary carries `awaitingIntegration`: true when a workflow ended `done` and no
`integration.applied` or `worktree.discarded` came after it (the home page badges it).

A session's `parentId` is the id of its parent session (null for a
top-level session); child sessions are hidden from the lists and opened
from the parent's thread.

- `GET /api/projects/{id}/workflows` → `[{id, name, description, origin}]`:
  the workflows found for the project, `origin` is `project`, `global` or
  `default`; 404 if no project
- `GET /api/projects/{id}/workflows/{wid}` → `{origin, workflow}`: one
  definition (`workflow` is the file as read, unknown fields included);
  404 if no project or no such workflow
- `PUT /api/projects/{id}/workflows/{wid}` `{scope: "project"|"global",
  workflow}` → saves after validation and answers `{origin, workflow}`;
  400 with `{error, errors: [{nodeId?, edge?, message}]}` on issues; 404 if
  no project
- `POST /api/workflows/validate` `{projectId, workflow}` → 200
  `{errors: [...]}` (empty when valid); 404 if no project
- `POST /api/sessions/{id}/workflow` `{workflowId, task, stepByStep?, worktree?, attachments?, plan?}` →
  202 `{}`; 404 if no session or no such workflow; 400 `empty task` when
  there are no attachments; 409
  while a turn or a run is active; `worktree` defaults to true (the session
  gets its own worktree; false makes the agent work in the project's folder;
  an existing worktree is kept); `attachments` is `[{filename, data}]` (data a
  base64 data URL; images only, 5 MB each, at most 10): the images are
  appended to the session as a user message before `workflow.started`, so
  session-context nodes see them, fresh ones do not; `plan` is the approved
  plan to start from:
  fennec records it as the output of the plan node (the one the workflow's
  criteria node names) and answers the human node right after it with its
  first choice, then runs on
- `POST /api/sessions` accepts `workflowId`, `stepByStep` and `worktree` too:
  when `workflowId` is set the prompt is the task and a run starts instead of
  a turn; an unknown workflow is a 404 before the session is created
- `POST /api/sessions/{id}/workflow/answer` `{runId, choice, comment}` → a
  human node; 202 `{}`; 409 while a turn runs or when `runId` is not the
  session's current run
- `POST /api/sessions/{id}/workflow/continue` `{runId, comment}` → a pause;
  same statuses
- `POST /api/sessions/{id}/workflow/resume` `{runId}` → an interrupted run;
  same statuses
- `POST /api/sessions/{id}/workflow/stop` `{runId}` → ends a run that is
  waiting or interrupted; same statuses
- `POST /api/sessions/{id}/workflow/pause` `{runId}` → while the run works:
  it finishes its current node and pauses at the next edge (inside parallel
  branches, after the join); 202 `{}`, again 202; 409 when the run is not
  running or `runId` is not the current run

While a run is active, a typed message is refused with a 409 that names
where the run waits: `the workflow waits for a choice` at a human node,
`the workflow is paused: continue or stop it` at a pause, `the workflow was
interrupted: resume or stop it` when interrupted. Only when the run waits
on an agent's `ask_user` is the message accepted as the answer to that
question. `/compact` and a mode change are refused with `a workflow is
running in this session` while a run is active.
