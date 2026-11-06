# fennec HTTP API (web UI ↔ server)

The server listens on `127.0.0.1:7777`. JSON in, JSON out. Errors are
`{"error": "<message>"}` with a 4xx/5xx status. Request bodies are limited to
32 MiB.

## Events

Every session is an append-only event log (`crates/fennec/src/domain/event.rs`),
one wire format, frozen: `{"type": "...", "data": {...}}`
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
| `criteria.preflight` | `{messageId, results: [{criterionId, command, verdict, exitCode?, output, existenceOnly}]}`: the plan's checks, run by fennec before any change at the end of the planner's turn (verified); `verdict` is `fails` (expected), `passesAlready` or `cannotRun`; `existenceOnly` is true when the check only searches the code (rg, grep, test -f...): it proves the code exists, not that it works; the last two send the plan back to the planner once |
| `criteria.proposed` | `{callId, criteria: [{id, description, check?}], reason}`: an agent's corrected criteria (`propose_criteria`), shown with a `chat.ask_user` of the same `callId` whose question lists only what changes against the criteria in force (modified, added, removed, then the unchanged ids); the human's `Accept` answer records them as `criteria.set` |
| `decision.recorded` | `{callId, title, choice, reason, alternatives}`: a design decision an agent recorded with `record_decision` (indicative: produced by a model); the turn goes on |
| `criterion.updated` | `{criterionId, status}`; `status` is `{type: "pending"}` or `{type: "passed" \| "failed", provenance}`, `provenance` `{kind: "verified", command, exitCode?, output}` (fennec ran the check) or `{kind: "indicative", agent, reason}` (an agent's verdict) |
| `integration.applied` | `{branch, base, commits, head, message, acknowledged, protectedTests}`: the human integrated the session's branch; `message` is the integration commit message, `acknowledged` lists the blockers they accepted, `protectedTests` the test files they protected with it (absent in older logs) |
| `tests.approved` | `{runId, files}`: the human validated the tests written before the code at the workflow's test gate; those files are protected for the rest of the session |
| `audit.tasks_created` | `{findings, ideaIds}`: the human turned audit findings into backlog ideas; `findings` are the report finding numbers, `ideaIds` the created ideas |
| `task.proposed` | `{callId, title, text}`: a backlog task an agent proposed with `propose_task` (indicative: produced by a model); the turn goes on |
| `plan.proposed` | `{callId, plan: {title, goal, steps: [{title, files, change}], doneWhen: [{text, check?}], risks}, markdown}`: the plan a plan agent recorded with `propose_plan` (indicative: produced by a model); `markdown` is the canonical text fennec rendered from it when it was recorded (`## Plan: …`, `## Done when`, `## Risks and open questions`): the workflow node's output, what the UI sends back as the approved plan, and the only plan text downstream readers parse; the turn ends `complete` and the human reviews it |
| `report.submitted` | `{callId, report: {title, summary, steps: [{title, done, note?}], doneWhen: [{text, met, how?}], changed: [{path, what}], test, todo}, markdown}`: the report a builder recorded with `submit_report` (indicative: produced by a model); `markdown` is the text fennec rendered from it when it was recorded (`## Report: …`, `### Steps`, `### Done when`, `### Changed`, `### Next`): the workflow node's output, what the UI shows as the report card; the turn ends `complete` and the human reviews it |
| `task.accepted` | `{callId, ideaId}`: the human added the proposed task `callId` to the project's backlog as `ideaId` |
| `agents_md.written` | `{replaced}`: the human wrote the AGENTS.md draft of the last answer to the project's root; `replaced` when a file was there |
| `architecture.rules_added` | `{count}`: the human appended the rules proposed in the last answer to `.agents/architecture`; `count` rule lines, comments aside |
| `understanding.started` | `{sessionId, head}`: the human opened a comprehension dialogue (the session `sessionId`, on the `understand` agent) on the branch's tip commit `head`; recorded in the reviewed session's log |
| `agent.suggested` | `{callId, agent, reason, brief}`: the `suggest_agent` tool: an agent suggests the human go on in a new session with another agent; nothing switches until the human opens it |
| `rule.blocked` | `{callId, ruleId, kind, pattern, scope, subject}`: fennec refused a tool call by one of the human's rules (fennec's own check, verified), before the call's failed `tool.result`; `kind` `command`/`tool`/`path`, `scope` `global`/`project`/`session`, `subject` the command, tool name or path refused (workdir-relative inside it, absolute outside); a negative `ruleId` is one of the kernel's own rules (`GET /api/rules` `fennec`) |
| `rule.approval_requested` | `{callId, subject, ruleId, kind, pattern, access, folders}`:; `folders` (a path only) the folders the human may grant instead, narrowest first, never `/` or `~/` fennec paused a tool call for the human (an "ask" rule, `ruleId` its id, or a file tool outside the workdir, `ruleId` null); the call stays open, no `tool.result`, and the turn ends `waiting_for_user`; `kind`/`pattern`/`access` are the rule the `session`, `always` and `never` answers store (`access` `read`/`write`/`readWrite` for a path, else null) |
| `rule.approval_answered` | `{callId, choice, ruleId}`: the human's answer (`once`/`session`/`always`/`never`), `ruleId` the rule it stored (null for `once`); fennec then appends the call's `tool.result` (run, or refused for `never`) and the turn goes on |
| `waiver.requested` | `{callId, path, reason}`: the agent's `request_waiver`: it asks the human to waive the protection of one project file; the call stays open, no `tool.result`, and the turn ends `waiting_for_user` (refused at once while the human is away) |
| `waiver.answered` | `{callId, granted, expiresAt, adr}`: the human's answer, recorded at the click; a grant exempts exactly `path` from protection (file tools and sandbox) in this session until `expiresAt` (ms), and `adr` is the MADR file written for it (`docs/adr/NNNN-waiver-<slug>.md` in the session's workdir); fennec then appends the call's `tool.result` and the turn goes on |
| `queue.added` | `{queueId, mode, content, queuedAt}`: a message typed while a turn runs; `mode` is always `asap` (dropped in between tool rounds); `queuedAt` is epoch ms |
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

`globalInstructions` and the AGENTS.md files from the
filesystem root down to the session's working directory go into the system
prompt of a new session; an existing session keeps the prompt of its first
turn.

`search` (SearXNG only) is the instance the `web_search`
tool queries (`GET {searxngUrl}/search?format=json&q=…`, with
`Authorization: Bearer {searxngApiKey}` when set). The tool runs in fennec,
not in a command: the model only chooses the query. `webFetch` (fennec's own, default true) allows the `web_fetch` tool, which also runs in fennec; the model chooses the URL, loopback and private addresses are refused.

`sandbox` (fennec's own) is how commands run: the `run_command` tool and the
workflow shell nodes. Enabled (the default), each command runs in bwrap: the
system read-only, the working directory writable, `/tmp` private, the secrets
under `$HOME` (`.ssh`, `.aws`, `.netrc`…) and fennec's data directory hidden,
the environment's credential-looking variables removed, every process ended
with the command, and no network unless `network` is true (npm and cargo are
then set offline). Disabled, commands run on the host.

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
- `GET /api/version` → `{version, build, changelogUrl}`: the binary's version, the same string `fennec --version` prints; `build` is the local build's number (`<commits>.<sha>[-dirty]`, set by `just install`), `null` for a release; `changelogUrl` is the changelog on GitHub
- `GET /api/update[?refresh=1]` → `{current, latest, newer, releaseUrl}`: the latest GitHub release against this
  binary, cached 6 h (`refresh=1` asks again); 502 `{error}` when no release can be read
- `POST /api/update` `{}` → 202 `{restarting, latest}`: installs the latest release over the binary (`install.sh`,
  checksum verified) and restarts the server on it; 409 while a turn runs, when up to date or on a development or local build
- `GET /api/server` → `{port, runningPort, fromFlag, path}`: the port `settings.yaml` sets (default 7777), the one this
  process listens on, whether `--port` was given (it wins over the file) and the file's path
- `POST /api/server` `{port}` → the same view: writes the port to `settings.yaml` (1024..=65535, else 400), applied at the
  next start
- `POST /api/server/restart` `{}` → 202 `{restarting}`: the server re-executes itself, same arguments; 409 while a turn runs
- `GET /api/setup` → `{checks: [{id: model|sandbox|git|project|scanners, status: ok|todo|warn, detail}], complete}`: the first-run checklist, probed on each call (verified); `todo` is required and missing, `warn` works but deserves a look (sandbox off, scanners missing); `complete` when no check is `todo`. `fennec doctor` prints the same checks.
- `GET /api/changelog` → `{current, releases: [{version, date, sections: [{title, items}]}], unseen}`: the
  release-please `CHANGELOG.md` embedded in the binary, newest first (verified); `unseen` counts the releases newer
  than the version the human last looked at, up to `current`. The first call records `current`, so a fresh install
  has nothing new.
- `POST /api/changelog/seen` `{}` → `{unseen: 0}`: the human looked at the changelog of the running version.
- `GET /api/settings` → `{preferences, listings, defaults, skills, optInWorkflows, scanners, sandboxHealth, mcpTools}`
  - `mcpTools`: `{<server name>: [{name, access: read|write|destructive|undeclared, tokens}]}`, each cached
    MCP tool's access as its server declares it in its annotations (indicative, not verified;
    `undeclared` without annotations) and its definition's estimated token cost
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
  One-token calls: a baseline, one per effort
  (accepted, deduped by `prompt_tokens`), one with an oversized `max_tokens`
  whose refusal may name the limits. A source is `endpoint` or `catalog` (the
  model family's known values, `catalog.rs` in llm-providers-connector). Indicative;
  502 with the endpoint's message when the model cannot be called
- `POST /api/settings/mcp/connect` with `{name}` → 200 `{server}` (the stored
  server, its `cachedTools` replaced by a live `tools/list`); 400 with the
  config's problems; 404 for an unknown server; 502 when the server cannot be
  reached or `tools/list` fails. The cache is what the model's tool definitions
  come from, so a frozen prompt stays byte-stable
- `POST /api/settings/mcp/authorize` with `{name}` → 200 `{url}`: starts the
  OAuth authorization (MCP authorization spec) of a stored http server:
  discovery from its 401, dynamic client registration, PKCE S256; `url` is the
  server's consent page, opened by the UI in a new tab. 400 for a stdio server
  or a config problem; 404 for an unknown server; 502 when discovery or
  registration fails
- `GET /api/mcp/oauth/callback?code&state` → an HTML page: the redirect target
  of the consent page. Exchanges the code, stores the tokens sealed with
  `secret.key` (`settings` row `mcp_oauth:{name}`), then connects as
  `mcp/connect` does. Later calls send the access token as a Bearer header and
  refresh it when it expires
- `GET /api/models/choices` → `{default, favorites}`, each a
  `{provider, model, providerName}`: what the composer's model picker offers;
  read from the stored preferences, no provider is called
- `GET /api/projects` → `[{id, name, workdir, favorite, recentRank, branch, remote, editorUrl, sessionWorktree, sessions: [{id, title, state, createdAt, updatedAt}]}]`
  (favourites first, then most recently active, sessions newest first; `createdAt`/`updatedAt` are Unix milliseconds; `favorite` is `true` when the project is pinned; `recentRank` is the project's place by recent activity alone, favourites ignored (0 = most recent), the order the top bar's "+" menu uses; `branch` and `remote` may be `null`; `editorUrl` opens the workdir in VS Code, through the WSL remote under WSL; `sessionWorktree` marks a workdir inside fennec's own store (a session's worktree opened as a project before that was refused))
- `POST /api/projects` `{workdir}` → the project (400 if not a directory, or inside fennec's store)
- `POST /api/projects/{id}/delete` `{deleteFolder?}` → `{deleted: true, folderDeleted, folderError}`: deletes the project with every top-level session as `POST /api/sessions/delete` does (logs, children, worktrees, branches, session rules), then its project rules, its ideas backlog and the project. With `deleteFolder: true` the source folder is removed from disk afterwards; a failure there comes back as `folderError` (the project is already gone). 404 for an unknown project; 409 `{error}` while its queue runs, `{error, running}` while one of its sessions runs a turn, `{error, sessionId}` when a worktree cannot be removed (the project and the sessions not yet removed stay); 400 when the folder is the filesystem root, the home directory or an ancestor of it, or holds fennec's store (nothing is deleted then). The usage ledger keeps the calls.
- `POST /api/projects/{id}/favorite` `{favorite}` → `{ok: true}`: pins or unpins the project (400 if `favorite` is missing or not a boolean; 404 for an unknown project)
- `GET /api/projects/{id}/budget` → `{limitUsd, label, figure, detail, tone, active}`: the project's monthly budget and what it spent this month, display-ready; 404 if no project. `limitUsd` is null when none is set; `tone` is `ok`, `attention`, `err` or `neutral`; `active` is true when a budget is set
- `POST /api/projects/{id}/budget` `{limitUsd: number | null}` → the same as `GET`; 400 when `limitUsd` is not a finite number > 0 (null removes the budget); 404 if no project
- `GET /api/projects/{id}/contributions` → `[{branch, ahead, subject}]`: the local and remote branches holding commits the human's HEAD lacks (fennec's own session branches left out), newest first, at most 50; `ahead` is that commit count, `subject` the tip's subject; `[]` outside a git repository; 404 if no project
- `POST /api/projects/{id}/contributions` `{branch}` → `{id}`: a new session titled `Contribution: <branch>` whose worktree branch `fennec/<id>` starts at the branch's tip, its base ref at `merge-base HEAD <branch>`, so the integration gate reviews exactly the contribution's commits; nothing is merged. 400 when `branch` is missing or not in the `GET` list; 404 if no project
- `GET /api/projects/{id}/indicators` → `{indicators: [{id, label, figure, detail, tone}]}`: the project's indicators, display-ready and verified (computed from the session logs, git and the usage ledger, never from a model); 404 if no project. `id` is one of `integrity`, `sensitiveReview`, `understanding`, `architecture`, `security`, `cost`, `transmission`, `traceability`, `reviewLoad`; `tone` as the dashboard tiles'
- `GET /api/backlog` → `{total, projects}`: the cross-project Backlog page, one row per project (the idea lists
  live in each project's Tasks tab):
  - `total` `{yours, running, ready, drafts, done, projects}`: counts across all projects; `ready` sums each
    project's `queue.ready`, `projects` counts the projects with a row
  - `projects` `[{project, yours, running, ready, drafts, done, queue}]`: the projects with at least one idea or a
    running queue; `project` `{id, name}`, the counts are its board's stages, `queue` `{running, autonomous, next}`
    (`next` `{id, text, title} | null`); a running queue first, then most `yours`, most `ready`, then by name
- `GET /api/projects/{id}/commands` → `[{name, description, kind}]`: the slash commands the composer offers (`compact`, then the project's skills); `kind` is `command` or `skill`; 404 if no project
- `GET /api/projects/{id}/agents` → `[{id, name, description, color, subagent, composer, phase, workflow, origin}]`: the
  project's agents ordered by `phase` (`start`, `frame`, `build`, `understand`, `check`; null last, for
  sub-agents and agents without one), discovery order within a phase (`color` null when unset; `workflow` the
  id of the workflow a message to this agent starts by default in the composer, null when the frontmatter
  names none); the composer's
  menu is the ones with `composer` true: `subagent` true only runs in a workflow node's fresh context, and
  `composer: false` in the frontmatter marks an agent fennec launches itself with a brief (a tile, the gate, a
  workflow node); `origin` is
  `default` (embedded in fennec), `user` (`~/.agents/agents`) or `project` (`.agents/agents`);
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
- `GET /api/agents[?projectId=…]` → `{agents, tools, broken}`, the human's agent library (the Settings page's
  Agents tab). With `projectId` the named project's `.agents/agents` is included, read-only. No agent tool reaches
  these routes: `.agents` is protected from the file tools, and the CRUD below acts for the human only.
  - an agent `{id, name, description, color, subagent, phase, workflow, origin, path, editable, readOnly, allowedTools}`,
    ordered by `phase` as `GET /api/projects/{id}/agents` is; `origin` `default`/`user`/`project`, `path` the file it
    was read from (null for a default), `editable` true only for `user`, `readOnly` whether its commands are
    read-only by construction.
  - `tools` the tool names an agent may list in `allowedTools`; `broken` `[{id, error, field}]` the human's
    `*.agent.md` files discovery had to skip, so a typo does not silently remove an agent from the menu.
- `GET /api/agents/{id}` → `{id, content, fields}`: the human's `~/.agents/agents/{id}.agent.md`, `content`
  verbatim and `fields` `{id, name, description, phase, workflow, color, subagent, readOnly, reasoningEffort, allowedTools,
  prompt}` for the edit form (null when the file does not parse, so the human can still see and delete it).
  400 for an id that is not kebab-case, 404 if no such file.
- `POST /api/agents[?projectId=…]` with the `fields` shape → `{agent}`: renders the `*.agent.md` from the fields
  (`domain/agent_file.rs` owns the frontmatter format, so no client writes YAML) and writes
  `~/.agents/agents/{id}.agent.md`, new or replaced. Refusals carry `{error, field}` (`field` the input at fault:
  `id`, `prompt`, `allowedTools`, empty when none): 400 for a bad id, an empty `id` or `prompt`, or a tool outside
  `tools`; 409 when the id belongs to a default or to the project (duplicate it under another id instead); 500 when
  `HOME` is unset. A default the human already overrode is theirs to edit.
- `POST /api/agents/{id}/delete[?projectId=…]` → `{ok}`: removes the human's file (404 if none, 409 for a default
  or a project agent). Agents are files, not rows: they are versionable and editable outside fennec, and a change
  applies to the next session that lists them.
- `GET /api/rules` → `{global, projects: [{id, name, rules}], sessions: [{id, title, projectId, rules}], fennec}`, every rule of
  the rules crate's own store (`rules.db`, `FENNEC_RULES_DB`): global ones, per project, and the session grants; `fennec` lists
  the kernel's own block rules (`git push`, `git update-ref`, `git branch -D`, `git worktree`, `gh pr merge`…: fennec integrates,
  the model never pushes nor moves a branch), built in code with negative ids, origin `"fennec"`, applied to every turn before
  the human's and never deleted;
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
  (read-only: the Settings page's Security tab, `#/settings/security`, edits them).
- `GET /api/projects/{id}/risks` → `{files, commits, aiCommits, unreviewedPercent, untested, churning, unreviewed, churnDays, modules, remedyPrompt}`, the
  project's risk report, read from git (verified; 404 if no project). `files` counts the tracked source files, tests
  excluded; `commits` and `aiCommits` report the commits in history and those declaring an AI co-author; the
  unreviewed signal sees only declared ones; `unreviewedPercent` the share of their lines (git blame) from AI commits
  never integrated through fennec, where a file is unreviewed when more than half of its lines come from such commits:
  a commit with an AI `Co-Authored-By` trailer and no `Fennec-Session` trailer (written only by an integration, the
  human's review gate).
  - `modules` `[{path, unreviewed, untested, churning, zones, remedyPrompt}]`: risk zones grouped by the zone file's
    parent directory (`.` for a file at the root), riskiest module first (most total signals, then path); the counts
    are the number of its zones carrying that signal, `remedyPrompt` is the same brief for this module only, and
    `zones` keeps the report's zone order (`{path, signals, churn, lines, unreviewedPercent}`, most signals first,
    then churn; `signals` among `unreviewed`, `untested`, `churn` as before; `untested` means no test of its own,
    and skips the files that need none: declaration-only modules, test support, build tooling).
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
  `repo` marks a git repository; 400 if `path` cannot be read or is inside fennec's store (its database, the sessions' worktrees)
- `POST /api/sessions/{id}/audit/tasks` `{findings: [number]}` → `{created}`: adds one idea per finding of the
  session's audit report to its project's backlog, the text written by the server; 400 when a number is unknown;
  409 when a finding is already a task; records `audit.tasks_created` in the session's log;
- `POST /api/sessions/{id}/proposals/{callId}/accept` `{}` → `{ideaId}`: adds the task proposed by the call
  `callId` to the session's project backlog (title, blank line, text); 404 when no such proposal; 409 when already
  accepted; records `task.accepted` in the session's log;
- `POST /api/sessions/{id}/agents-md` `{}` → `{replaced}`: writes the last answer's AGENTS.md draft to the project's root; 400 when the last answer has no draft; 409 when AGENTS.md is a symlink; records `agents_md.written`;
- `POST /api/sessions/{id}/architecture-rules` `{}` → `{count}`: appends the last answer's proposed rules not yet in `.agents/architecture` (each with its comment), creating the file if needed; 400 when none is new; 409 when `.agents` or the file is a symlink; records `architecture.rules_added`;
- `GET /api/sessions/{id}` → `{id, title, state, mode, model, project: {id, name, workdir}, worktree, idea, request, audit, proposedTasks, agentsMdDraft, ruleProposal, agentSuggestion}`;
  `idea` is `{id, state}` when the session frames one of the project's ideas, else null;
  `audit` is the session's last answer read as an audit report, `{scope, summary, findings: [{number, title,
  severity: high|medium|low, principle, locations: [string], problem, fix}], taken: [number]}`, or null when it is
  not one; `taken` lists findings already turned into tasks since this report;
  `proposedTasks` is `[{callId, title, text, ideaId}]`, the tasks proposed in the session in order, `ideaId` null
  until the human accepts one;
  `agentsMdDraft` is `{exists, written}` when the last answer holds an AGENTS.md draft (`## AGENTS.md draft` then a ```md block), else null;
  `ruleProposal` is `{lines, added}` when the last answer proposes rules (`## Executable rules` then a fenced block of rule lines and `#` comments): `lines` those not yet in `.agents/architecture`, `added` once the human appended them; else null;
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
- `GET /api/sessions/{id}/integration` → `{branch, base, originBranch, humanBranch, defaultMessage, commits: [{sha, subject}], files: [{path, protected, carried, added, removed, signals}], commonSignals, testFiles, criteria, decisions, blockers, idea}`:
  what integrating the session's worktree branch would do (409 without a
  worktree). `files` come riskiest first (the review order): `signals` ⊂
  protected, sensitive, outsidePlan, bypass, architecture, conflictMarker, testsWeakened (the gate's
  own checks, per file), hot (5+ commits in 90 days), untested (no test of its own: no inline
  test, no test file of the same stem on the branch; declaration-only modules, test support and build tooling
  need none), large (200+ lines
  changed); deterministic. `commonSignals` are the signals every file carries (empty
  under two files), so the UI says them once. `originBranch` is the branch the run started from (null for
  older worktrees or a detached HEAD). `decisions` are the session's
  `decision.recorded` events, `{callId, title, choice, reason, alternatives}`:
  informational (written by the model), never a blocker. `defaultMessage` is fennec's starting
  commit message (`feat: <title>`, an empty title gives `feat: integrate the
  agent's work`); the human edits it. `idea` is the idea tied to the session that the human may finish here (state started), null otherwise. `securityScans` is one `{scanner, state}` per scanner, `state` one of `ran {findings}`, `notInstalled`, `notApplicable`, `failed {reason}`, `running {seconds}`: a scanner missing, not needed or failing is shown, never a blocker. The scanners run in the background, started by the first read of the gate and cached per (base, branch); while one runs, the gate carries `securityScanRunning` and the UI reads it again each second. `blockers` is the review gate, each
  `{kind, …}`: `noCommits`, `conflict {files}` (the merge onto the human's HEAD would conflict on these files; never acknowledged: the branch must be brought up to date first), `branchChanged {from, to}`, `localChanges {paths}` (uncommitted changes in the human's tree the agent's work would overwrite: content equal to neither the base's nor the branch's; the common case, the same dirty work copied into the worktree at the run's start, is not a blocker but a file flagged `carried`, whose uncommitted work goes into the integration commit; never acknowledged: the pick refuses to overwrite it, commit, stash or remove it first), `criteriaNotPassed
  {criteria}`, `noCriteria`, `protectedTouched {paths}`, `sensitiveTouched {paths}` (changed files inside the sensitive zones of `.agents/architecture`, one `sensitive <pattern>` per line, gitignore-like: the human's review is mandatory there; deterministic), `outsidePlan {paths}` (changed files that no `Files:` line of the run's approved plan lists, the plan read from the plan node named by the workflow's criteria node; no plan, no check; deterministic), `understandingMissing` (the branch changes a sensitive zone and no comprehension dialogue was held on its current head; deterministic), `securityFindings {findings: [{scanner, rule, path, line, detail}]}` (what the security scanners found on PATH report on the branch, run by fennec in the read-only sandbox without network: gitleaks on the branch's commits, secrets redacted; cargo-audit when a Cargo.lock changed, offline; semgrep on the changed files only with the human's own `.semgrep.yml`; deterministic, zero model tokens), `securityScanRunning` (a scanner still runs, its findings unknown; never acknowledged, it clears when the scanners end), `architectureViolated {violations: [{path, line, prefix, forbidden}]}` (added lines that break the project's `.agents/architecture` rules, one `<path prefix> forbids <text>` per line; deterministic, checked on added lines only), `suppressionsAdded {suppressions: [{path, line, marker}]}` (added lines that silence a lint, a type check or a test: `#[allow(`, `@ts-ignore`, `noqa`, `it.skip(`, `pytest.mark.skip`…; text tripwires, comments included), `testsWeakened {testsRemoved, assertionsRemoved, paths}` (the branch removes more test declarations or assertions than it adds, summed over all files so moved tests do not count; `paths` are the files that lost some); `branchChanged.to`
  is null for a detached HEAD; `testFiles` are the integrated test files (paths
  relative to the project) that the protection does not cover yet; protection is read from the project's tree, not
  the worktree's.
- `GET /api/sessions/{id}/diff?path=<path>` → `{path, binary, truncated, hunks: [{header, lines: [{kind, old, new, text}]}], editorUrl}`: one file the session's branch changed, over the range the integration takes; `path` as in the integration's `files[].path`, `kind` is `context`, `added` or `removed`, `old`/`new` the line numbers (null on the side the line is absent from), `truncated` past 5000 lines, `editorUrl` opens the worktree's file in VS Code (verified: read from git). 400 without `path`, 404 when the session does not exist or the branch did not change the path, 409 without a worktree or a git repository.
- `POST /api/sessions/{id}/understanding` `{dialogueId}` → `{understanding}`: records that the session `dialogueId`, created just before by the UI, is the comprehension dialogue of the session `id` on its branch's current head (`understanding.started`); 400 when `dialogueId` is missing or belongs to another project, 404 when it does not exist, 409 when the branch has no commit. `understanding` in the integration preview is `{state: "none"}`, `{state: "started", sessionId}` (opened, the human has not answered yet) or `{state: "held", sessionId}` (the human answered at least once after the opening brief): verified, folded from the two logs; a dialogue on an older head does not count.
- `POST /api/sessions/{id}/integration/message` → `{message}`: an English Conventional Commit subject the model proposes for the integration (indicative; `null` when the model fails or answers no valid subject; `defaultMessage` stays the fallback). The body is `{}`.
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
- `GET /api/plugins` → `{plugins: [{name, command, args, env, blocks, enabled, network, active}], problems, blocks}`: `problems` lists why definitions were left out (the file's parse error, a definition's problems);
  the human's plugin definitions (`~/.agents/plugins.json`, a definition with problems left out; a plugin's code
  lives in `~/.agents/plugins/<name>/`; nothing is read from a project) and the human's global activation of
  each, stored in fennec's store; `active` when enabled and the SHA-256 of its definition and code still matches
  the one taken at activation (a changed file deactivates it). `blocks` the global-scope blocks placed in the
  Plugins tab (`placement` `settings`), the served-block shape below.
- `POST /api/plugins` `{name, enabled, network}` → `{name, enabled, network, active}`: the human activates or
  deactivates a plugin, hashing its definition and code now, and restarts the plugin hosts. A plugin always runs
  in the bwrap world (never with the sandbox off), its code directory read-only (and its working directory), a
  private scratch writable, no project mounted, the network only when `network`. 400 on an empty name or an
  invalid definition; 404 if no such plugin.
- `GET /api/sessions/{id}/plugins` → `{blocks}`: the session-scope blocks placed in the session's right-hand
  context panel (`placement` `panel` or `own_panel`). 404 if no session. The event feed a plugin receives
  carries the `session` and `project` ids; a session seen for the first time by the pump is not replayed.
- `GET /api/projects/{id}/plugins` → `{blocks}`: the project-scope blocks placed in the project page's
  dashboard (`placement` `dashboard`). 404 if no project.
- `GET /api/plugins/status-bar?sessionId=&projectId=` → `{blocks}`: every block placed in the status bar
  (`placement` `status_bar`), in the order the surfaces stack: the global ones, then the project's, then the
  session's. Both query params optional; a missing one simply omits that surface's blocks.
- The served-block shape of the four routes above:
  `[{plugin, title, scope: "session"|"project"|"global", placement: "panel"|"own_panel"|"status_bar"|"dashboard"|"settings",
  content: {elements: [...]}}]`. Elements (data only, rendered by the UI, indicative):
  `{"kind":"text","text"}`, `{"kind":"figure","label","value","tone"?}`, `{"kind":"badge","label","tone"?}`,
  `{"kind":"list","items":[{"text","tone"?}]}`, `{"kind":"progress","label","value","max","tone"?}` (finite
  numbers, `max > 0`), `{"kind":"link","label","url"}` (`url` starts with `http://` or `https://`),
  `{"kind":"table","columns":[…],"rows":[[…]]}` (at most 100 rows, no row longer than `columns`); `tone` is
  `none`, `info`, `success`, `warning` or `danger`. An update with a non-http link, a non-finite progress or an
  oversized table is dropped with the rest of its content, as one beyond the caps (200 elements, 32 KiB).
- `GET /api/sessions/{id}/servers` → `[{name, command, suggested, state, running, url, exitCode, output}]`:
  the project's servers (`.agents/servers.json`) and their state in this session. When the file
  lists none, one `dev` server guessed from the project (a Justfile `dev` recipe, a package.json
  `dev` script, a Makefile `dev` or `run` target, else `cargo run` with a Cargo.toml) comes back
  with `suggested: true`; the UI saves it on the human's confirmation. `state` is `stopped`,
  `starting` (the process lives, its port does not answer yet), `ready` (the port answers) or
  `exited` (it ended on its own; `exitCode`, `null` when killed by a signal). `url` is the last
  loopback URL the server printed, else `http://127.0.0.1:{PORT}`, `null` unless it runs;
  `running` is `starting` or `ready`. `output` is the last 40 lines of its stdout and stderr,
  without ANSI escapes (fennec keeps 200). 404 if no session.
- `POST /api/sessions/{id}/servers` `{servers: [{name, command}]}` → the same list:
  saves the project's servers to `.agents/servers.json`. 400 on an empty name/command or a
  duplicate name, 404 if no session.
- `POST /api/sessions/{id}/servers/start` `{name}` → 202 `{pid, port}`: starts the named server
  (from the project's `.agents/servers.json`) in the session's worktree when it has one, after
  linking its missing dependency directories and its gitignored `.env*` files to the main
  checkout's (links, never copies); otherwise in the project's directory. It passes a free `PORT`,
  the project's shared `CARGO_TARGET_DIR` (so a worktree does not rebuild from scratch), and runs
  as `setsid sh -c <command>`, so a stop frees the port by killing the whole process group.
  Outside the sandbox: the human's action. 404 for an unknown server, 409 if it already runs (an
  exited one restarts), 500 if it cannot start.
- `POST /api/sessions/{id}/servers/stop` `{name}` → `{}`: stops the server by killing its
  process group, or forgets an exited one. 404 if it is not running.
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
- `GET /api/sessions/{id}/thread` → `{pendingApproval, pendingWaiver, queued, criteria, decisions}`: the session's
  blocks, folded by the server (the UI computes none): `pendingApproval` `{callId, subject, kind, pattern, access,
  folders}` or null, the tool call waiting for the human's answer; `pendingWaiver` `{callId, path, reason}` or null;
  `queued` `[{queueId, content}]` the messages queued for the next rounds, in order; `criteria`
  `[{id, description, check?, status, passedBefore, existenceOnly}]` the criteria in force (the last `criteria.set` and the
  `criterion.updated` since; `status` as in the integration preview, `passedBefore` from the last preflight, `existenceOnly` true when the check only searches the code: it proves the code exists, not that it works);
  `decisions` `[{callId, title, choice, reason, alternatives}]` those recorded, oldest first; `replayable` the ids
  of the messages the human may replay (typed, in the model's current view: the UI offers the action on those only);
  deterministic (the decisions and the indicative statuses record what a model said); 404 if no session.
- `GET /api/sessions/{id}/usage/calls` → `[{messageId, line, detail, turnRecap}]`, oldest first: what sits under
  each answer, written by the server from the ledger (`line` `↑ 12,3k · cache 82 % · ↓ 345 · 4,2 s`, `detail` the
  tooltip, `turnRecap` on a turn's last call only, null otherwise: the turn's wall clock, `chat.done.durationMs`
  with its tools, over the tokens its calls generated); ledger rows stored before the `messageId` label
  existed are labelled at server start
- `GET /api/sessions/{id}/usage/workflow` → `{nodes: [{itemId, line, detail}], runs: [{runId, calls, generated, read, cache}]}`: the
  tokens of each workflow node run (`itemId` is `wf-{runId}-{nodeId}-{pass}`, its child session's calls or the
  session's own between its start and end) and of each finished run, from the ledger, ready to display.
- `GET /api/insight?days=<7|30|90|all>&project=<id>` → `{cards: [{kind, label, value, detail}], days: [{day, label,
  cachedPercent, freshPercent, lostPercent, title}], projects: [row], models: [row], consumers, projectOptions: [{id, name}]}`,
  `row` being `{key, name, calls, input, cache, output, lost, cost, time}`: the Insight page from the ledger, so deleted
  sessions still count; `kind` is the card's stable id (`calls`, `input`, `cache`, `lostCache`, `output`, `cost`), the
  texts follow the stored `display.language`; `cost` and the last card are an estimate at each model's own price (Settings, per model), "—"
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
- `POST /api/sessions/{id}/retry` → 202; 404 if no session; 409 if a turn is
  running or a workflow waits in the session; 400 `the last turn did not end
  in an error` when the last `chat.done` is not reason `error`. Re-runs the
  turn with a system-generated continuation prompt (the last `chat.error`
  included when there is one)
- `POST /api/sessions/{id}/compact` → 202; 404 if no session; 409 if a turn is
  running; summarises the history and opens a new context window
- `POST /api/sessions/{id}/mode` `{mode}` → 200 `{mode}`; 404 if no session;
  400 with `unknown mode: <mode>` if the id is not a top-level agent; appends
  `mode.changed` when the mode changes; allowed while a turn runs
- `POST /api/sessions/{id}/model` `{provider, model}` → 200 `{model}` (the
  session's `model` as in `GET /api/sessions/{id}`); 404 if no session; 400 if
  the model is empty or the provider unknown; 409 while a workflow runs;
  appends `model.changed`
- `GET /api/sessions/{id}/mcp` → `{servers, totalTokens}`: each server with
  `name, visible, disabled, active, tokens, enabledToolTokens, toolCount, tools` (each tool with
  `name, enabled, tokens`); `active` is the session's effective state (the
  global flag, or the session's last `mcp.disabled`); `totalTokens` is the cost
  of the active tools; `enabledToolTokens` the enabled tools' cost, active or not
- `POST /api/sessions/{id}/mcp` `{disabled: [name…]}` → 200 with the same body
  as `GET`; appends `mcp.disabled` (the full disabled set, last wins); 404 if
  no session
- `POST /api/sessions/{id}/stop` → 202; the turn ends with `chat.done` reason `"stopped"`
- `GET /api/notices` → Server-Sent Events. A snapshot of what needs the human, sent on connect and
  re-sent when it changes; child sessions stay silent. Each event is
  `data: {"items": [...], "budgets": [...], "pluginsVersion": <n>}`; `items` are the attention items (`sessionId`, `title`,
  `projectId`, `kind`, `detail`), `budgets` one entry per project whose monthly budget is at alert
  or exceeded (`projectId`, `project`, `status` `alert`|`exceeded`, `detail`), `pluginsVersion` a counter that
  moves on every accepted plugin block.update, so a tab refreshes on new plugin data without a session log
  changing; no backlog is sent on connect.
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

## Canvas

Artifacts the human and the agent iterate on, each version kept (the `canvas` crate, its own `canvas.db`). Deleting a session or a project deletes its artifacts. The agent's `canvas` tool creates a message artifact titled as it asked, and adds a version to it when called again with the same title.

- `GET /api/sessions/{id}/canvas` → the canvas artifacts pinned in the session; 404 when the session is missing.
- `POST /api/sessions/{id}/canvas` → pin a project-relative file with `{"path": "<path>"}` or promote a message block with `{"language": "html"|"md"|"markdown"|"svg"|"mermaid"|"json", "content": "<text>"}`; 400 for an unsupported file/language, empty content, or invalid body, 404 for a missing session/file, 413 above 10 MiB; 200 with `{"artifact": Artifact}` when the file was already pinned, 201 with `{"artifact": Artifact}` when a new artifact is created.
- `GET /api/projects/{id}/canvas` → the canvas artifacts in the project; 404 when the project is missing.
- `GET /api/canvas/{id}` → `{"artifact": Artifact, "versions": [Version], "annotations": [Annotation]}`; 404 when the artifact is missing. Versions of a file are taken when this route reads it, those of a pinned message block from the blocks of its format the agent wrote after the latest version, and identical content makes no new version.
- `GET /api/canvas/{id}/versions/{n}` → the raw bytes of version `n`, with the artifact's `Content-Type` and `Content-Security-Policy: sandbox allow-scripts` (omitted for a PDF whose bytes start with `%PDF-`: browsers' PDF viewers refuse a sandboxed document); 404 when the artifact or version is missing.
- `POST /api/canvas/{id}/delete` → delete the artifact from the library; 404 when it is missing. Deleting never touches the file in the session's working directory.
- `POST /api/canvas/{id}/annotations` → add an annotation with `{"version": <u32>, "anchor": Anchor, "note": "<text>"}`; 400 for a missing/invalid field, 404 when the artifact is missing; 201 with `{"annotation": Annotation, "message": <string>}`.
- `Artifact` is `{id, projectId, sessionId, title, format: "html"|"markdown"|"svg"|"mermaid"|"json"|"pdf"|"png"|"jpeg"|"gif"|"webp", source: {kind: "file", path} | {kind: "message"}, createdAt, updatedAt, latestVersion}`.
- `Version` is `{number, size, createdAt}`.
- `Annotation` is `{id, artifactId, version, anchor, note, createdAt}`.
- `Anchor` is `{kind: "whole"} | {kind: "quote", text} | {kind: "region", x, y, width, height}`.

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
  same statuses. Also relaunches a run that **ended** in `error`: it re-runs
  the node that was open when the run ended (a new pass), then continues the
  walk; 409 `the failed node is in a branch, it cannot be relaunched alone`
  when that node sits in a parallel branch
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
