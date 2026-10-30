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
| `criteria.proposed` | `{callId, criteria: [{id, description, check?}], reason}`: an agent's corrected criteria (`propose_criteria`), shown with a `chat.ask_user` of the same `callId` whose question lists only what changes against the criteria in force (modified, added, removed, then the unchanged ids); the human's `Accept` answer records them as `criteria.set` |
| `decision.recorded` | `{callId, title, choice, reason, alternatives}`: a design decision an agent recorded with `record_decision` (indicative: produced by a model); the turn goes on |
| `criterion.updated` | `{criterionId, status}`; `status` is `{type: "pending"}` or `{type: "passed" \| "failed", provenance}`, `provenance` `{kind: "verified", command, exitCode?, output}` (fennec ran the check) or `{kind: "indicative", agent, reason}` (an agent's verdict) |
| `integration.applied` | `{branch, base, commits, head, message, acknowledged, protectedTests}`: the human integrated the session's branch; `message` is the integration commit message, `acknowledged` lists the blockers they accepted, `protectedTests` the test files they protected with it (absent in older logs) |
| `worktree.discarded` | `{branch, commits}`: the human discarded the session's worktree and branch; `commits` are the agent's commits that were dropped |
| `workflow.paused` | `{runId, from, port, to}`: waiting before `to` |
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
`.gitlab-ci.yml`, and the accepted ADRs `docs/adr`) plus the lines of the project's `.agents/protected`
(gitignore-like: `#` comments, `*`, `?`, `**`, a leading `/` anchors, a
pattern without `/` matches at any depth, a directory protects its content).
`write_file` and `edit_file` refuse them; in the sandbox they are mounted
read-only. The human edits them as usual.

## Endpoints

- `GET /api/config` → `{model, contextWindow, compactAt}`: the resolved parameters of the
  stored preferences; `compactAt` is the fraction at which the session compacts (0 disables)
- `GET /api/settings` → `{preferences, listings, defaults}`
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
- `GET /api/projects` → `[{id, name, workdir, branch, sessions: [{id, title, state, createdAt, updatedAt}]}]`
  (projects most recently active first, sessions newest first; `createdAt`/`updatedAt` are Unix milliseconds; `branch` may be `null`)
- `POST /api/projects` `{workdir}` → the project (400 if not a directory)
- `GET /api/projects/{id}/commands` → `[{name, description}]`: the slash commands the composer offers (`compact`, then the project's skills); 404 if no project
- `GET /api/projects/{id}/agents` → `[{id, name, description, color, subagent}]`: the
  project's agents in discovery order (`color` null when unset); the modes are the
  ones with `subagent` false, the others only run in a workflow node's fresh context;
  404 if no project
- `GET /api/projects/{id}/overview` → `{map, mapFile, adrs}`, the project page (404 if no project):
  - `map` `{modules: [{id, files, tests, entry, layer, row}], dependencies: [{from, to, imports}]}`:
    the project map, read from the import lines of the files git tracks (Rust `crate::` and workspace
    crates, TS/JS relative imports; test modules skipped); a module is a directory (`.` the root),
    `imports` counts the importing files; `layer`/`row` place it (layers by dependency depth): the UI
    computes nothing. Deterministic (verified).
  - `mapFile` `missing | stale | current`: the project's `docs/map.md` against a fresh render of `map`.
  - `adrs` `[{number, file, title, status, date, body}]`: the MADR files of `docs/adr/` (`NNNN-slug.md`,
    sorted by number; `status` and `date` from the frontmatter, null when absent; the title from the
    first `# ` heading).
- `POST /api/projects/{id}/map-file` `{}` → `{mapFile}`: the human's click writes `docs/map.md` (a Mermaid
  diagram and a table) in the project's tree, not committed; 404 if no project
- `GET /api/folders?path=<absolute path>` → `{path, parent, folders: [{name, path, hidden, repo}]}`,
  the sub-folders of `path` (the home directory when omitted), sorted by name;
  `repo` marks a git repository; 400 if `path` cannot be read
- `GET /api/sessions/{id}` → `{id, title, state, mode, model, project: {id, name, workdir}, worktree}`;
  `model` is `{provider, model, providerName, contextWindow, compactAt, source}`,
  what the next turn runs on; `source` is `session`, `parent` or `default`;
  `worktree` is `{path, branch, base, editorUrl}` (`editorUrl` opens the copy in VS Code, through the WSL remote under WSL) when the session (or its root session) runs a workflow in
  its own git worktree, else null; the human takes the agent's work with
  `git cherry-pick <base>..<branch>`;
  `mode` is the id of the agent the session resolves to from its event log
  (null when there is no top-level agent)
- `GET /api/sessions/{id}/integration` → `{branch, base, originBranch, humanBranch, defaultMessage, commits: [{sha, subject}], files: [{path, protected, added, removed}], testFiles, criteria, decisions, blockers}`:
  what integrating the session's worktree branch would do (409 without a
  worktree). `originBranch` is the branch the run started from (null for
  older worktrees or a detached HEAD). `decisions` are the session's
  `decision.recorded` events, `{callId, title, choice, reason, alternatives}`:
  informational (written by the model), never a blocker. `defaultMessage` is fennec's starting
  commit message (`feat: <title>`, an empty title gives `feat: integrate the
  agent's work`); the human edits it. `blockers` is the review gate, each
  `{kind, …}`: `noCommits`, `branchChanged {from, to}`, `localChanges {paths}` (uncommitted changes in the human's tree the cherry-pick would overwrite — unstaged edits whose content differs from both the base's and the branch's, and untracked files the pick would create; the common case, the same dirty work copied into the worktree at the run's start, is not a blocker), `criteriaNotPassed
  {criteria}`, `noCriteria`, `protectedTouched {paths}`, `architectureViolated {violations: [{path, line, prefix, forbidden}]}` (added lines that break the project's `.agents/architecture` rules, one `<path prefix> forbids <text>` per line; deterministic, checked on added lines only), `suppressionsAdded {suppressions: [{path, line, marker}]}` (added lines that silence a lint, a type check or a test: `#[allow(`, `@ts-ignore`, `noqa`, `it.skip(`, `pytest.mark.skip`…; text tripwires, comments included), `testsWeakened {testsRemoved, assertionsRemoved, paths}` (the branch removes more test declarations or assertions than it adds, summed over all files so moved tests do not count; `paths` are the files that lost some); `branchChanged.to`
  is null for a detached HEAD; `testFiles` are the integrated test files (paths
  relative to the project) that the protection does not cover yet; protection is read from the project's tree, not
  the worktree's.
- `POST /api/sessions/{id}/integrate` `{acknowledged: [kind], message, protectTests?, removeWorktree?}` → `{head, commits, protectedTests, protectError, worktreeRemoved, removeError}`:
  the human's decision. The server recomputes the gate and answers 400 on a
  blank `message`, 409 while a turn runs, while the human has staged changes,
  while a blocker is not acknowledged (`noCommits` never is), or when git
  fails; then it runs `git cherry-pick --no-commit <base>..<branch>` in the
  human's repository and one `git commit` with `message` plus a
  `Fennec-Session: <session id>` trailer, under the human's git identity. A
  conflict is aborted, the tree left as it was, and answers 409
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
  `worktree.discarded`. A later workflow in the session opens a fresh worktree.
- `GET /api/sessions/{id}/skills` → `{loaded: [ids], active: [ids]}`: the
  skills the session loaded (successful `load_skill` calls and `/<skill>`
  messages it typed, first-seen order) and those still in the model's view
  (not dropped by a compaction, not cut by a replay); 404 if no session
- `GET /api/sessions/{id}/usage` → `{calls, promptTokens, cachedTokens, completionTokens, cachePercent, lastGenerationSpeed}`:
  the session's model calls from the usage ledger (`usage.db`, crate `token-usage`), which outlives the log;
  `cachePercent` is 0 to 100, `lastGenerationSpeed` (tokens/s) is null before any call; 404 if no session
- `GET /api/sessions/{id}/context` → `{percent, tone, compactMark, line, title, composition}`: the header's gauge, from
  the last `context.state` the model still sees (`tone` `ok`/`warn`/`err`, `compactMark` null when compaction is off);
  events without `compactAt` and sessions without a measure use the current settings. `composition` (null while
  nothing is measured) is what the context holds: `kinds` `[{kind, label, tokens, percent}]` for the stacked bar
  (`kind` `prompt`/`conversation`/`tools`, fixed order), `parts` `[{kind, label, tokens, percent}]` largest first
  (the instruction files, the tool definitions, each tool's results, the reasoning…; past 12, one row with `kind`
  null sums the rest), `images` a note or null. The exact token count is spread over the parts by their share of
  the text: indicative, and said so
- `GET /api/sessions/{id}/usage/calls` → `[{messageId, line, detail, turnRecap}]`, oldest first: what sits under
  each answer, written by the server from the ledger (`line` `↑ 12,3k · cache 82 % · ↓ 345 · 4,2 s`, `detail` the
  tooltip, `turnRecap` on a turn's last call only, null otherwise: the turn's wall clock, `chat.done.durationMs`
  with its tools, over the tokens its calls generated); ledger rows stored before the `messageId` label
  existed are labelled at server start
- `GET /api/sessions/{id}/usage/workflow` → `{nodes: [{itemId, line, detail}], runs: [{runId, calls, generated, read, cache}]}`: the
  tokens of each workflow node run (`itemId` is `wf-{runId}-{nodeId}-{pass}`, its child session's calls or the
  session's own between its start and end) and of each finished run, from the ledger, ready to display.
- `GET /api/insight?days=<7|30|90|all>&project=<id>` → `{scope, cards: [{label, value, detail}], days: [{day, label,
  cachedPercent, freshPercent, lostPercent, title}], projects: [row], models: [row], consumers, projectOptions: [{id, name}]}`,
  `row` being `{key, name, calls, input, cache, output, lost, time}`: the Insight page from the ledger, so deleted
  sessions still count; `days` defaults to 30 (400 otherwise), `project` is optional (`projects` is then empty); bar
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

While a run is active, a typed message is refused with a 409 that names
where the run waits: `the workflow waits for a choice` at a human node,
`the workflow is paused: continue or stop it` at a pause, `the workflow was
interrupted: resume or stop it` when interrupted. Only when the run waits
on an agent's `ask_user` is the message accepted as the answer to that
question. `/compact` and a mode change are refused with `a workflow is
running in this session` while a run is active.
