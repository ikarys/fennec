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
| `message.delta` | `{messageId, content}` (text chunk) |
| `message.thinking` | `{messageId, content}` (reasoning chunk) |
| `message.done` | `{messageId, partial?, stats?}`; `stats` (assistant messages): `{model, totalTime (s), prefillTokens, cachedPromptTokens, generationTokens, prefillSpeed, generationSpeed}`, speeds in tokens/s, prefill speed over new (non-cached) tokens |
| `tool.call` | `{messageId, toolCall: {id, name, arguments, parseError?}}` |
| `tool.output` | `{messageId, toolCallId, stream: "stdout"\|"stderr", content}` |
| `tool.result` | `{messageId, toolCallId, result: {success, output?, error?, durationMs, truncated, metadata?: {mimeType, dataUrl, path, size}}}` (`metadata` when `read_file` returned an image) |
| `context.state` | `{currentTokens, maxTokens}` |
| `context.compacted` | `{beforeTokens, afterTokens, summary}`: the cut where a new context window started from `summary`; `afterTokens` is an estimate until the next model call reports the real count |
| `running.changed` | `{isRunning}` |
| `chat.done` | `{messageId, reason: "complete"\|"stopped"\|"error"\|"truncated"\|"waiting_for_user"}` |
| `chat.error` | `{error, recoverable}` |
| `chat.ask_user` | `{callId, question, type?: "text"\|"confirm"\|"choice", options?: [{value, label, description?}]}`: the model asked the human a question; the turn ends and the answer arrives as the next turn's input |
| `turn.snapshot` | `{cachedSystemPrompt}`: the session's system prompt, frozen at its first turn |
| `mode.changed` | `{mode, auto, reason?}`: the human switched the session's agent; it applies from the next turn |
| `session.rewound` | `{messageId}`: fennec's replay cut; the model's view drops that message and everything after it, the log keeps them |
| `workflow.started` | `{runId, workflow, task, stepByStep}`: `workflow` is the whole definition, frozen for the run |
| `workflow.node.started` | `{runId, nodeId, pass, via?, childSessionId?}`: `via` is `{from, port}`, the edge taken |
| `workflow.node.finished` | `{runId, nodeId, port, output, exitCode?, durationMs?}`: `durationMs` is the node's working time measured by fennec (none for a human node) |
| `criteria.set` | `{criteria: [{id, description, check?}]}`: the approved plan's criteria, all pending |
| `criterion.updated` | `{criterionId, status}`; `status` is `{type: "pending"}` or `{type: "passed" \| "failed", provenance}`, `provenance` `{kind: "verified", command, exitCode?, output}` (fennec ran the check) or `{kind: "indicative", agent, reason}` (an agent's verdict) |
| `integration.applied` | `{branch, base, commits, head, acknowledged}`: the human integrated the session's branch; `acknowledged` lists the blockers they accepted |
| `worktree.discarded` | `{branch, commits}`: the human discarded the session's worktree and branch; `commits` are the agent's commits that were dropped |
| `workflow.paused` | `{runId, from, port, to}`: waiting before `to` |
| `workflow.resumed` | `{runId, comment}` |
| `workflow.finished` | `{runId, outcome, reason?, stats?}`: `outcome` is `done`, `blocked`, `stopped` or `error`; `stats` are facts fennec computed from the log: `{workingMs, nodes: [{nodeId, label, runs, workingMs}], reruns, calls, prefillTokens, cachedPromptTokens, generationTokens, filesChanged, criteriaTotal, criteriaPassed, criteriaVerified}` (child sessions' model calls included, the time waiting for the human excluded) |

Tools: `read_file {path, offset?, limit?}`, `write_file {path, content}`,
`edit_file {path, old_string, new_string, replace_all?}`,
`run_command {command, cwd?, timeout?}`,
`ask_user {question, type?, options?}` (handled by the loop, the turn ends
waiting for the human).

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

Precedence for a turn:

1. stored per-model value in `models[model]`
2. for `contextWindow` only, the endpoint's current `/models` listing
3. the environment/built-in default (`FENNEC_*`, or 128 000 tokens)

`Preferences` JSON (camelCase, all fields optional on read):

```json
{
  "model": "string | null",
  "models": {
    "<model id>": {
      "maxTokens": "number | null",
      "contextWindow": "number | null",
      "compactionThreshold": "number | null",
      "reasoningEffort": "\"none\" | \"minimal\" | \"low\" | \"medium\" | \"high\" | \"xhigh\" | \"max\" | null"
    }
  },
  "display": {
    "reasoningOpen": true,
    "toolResultsOpen": false,
    "fontSize": 14
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
`fontSize` in `13..=16` (default 14), `searxngUrl` blank or starting with `http://` or `https://`. `reasoningEffort` is sent as the
`reasoning_effort` field of the model request when set; `null` omits it.

`globalInstructions` (openfox name) and the AGENTS.md files from the
filesystem root down to the session's working directory go into the system
prompt of a new session; an existing session keeps the prompt of its first
turn.

`search` (openfox `search.*`, SearXNG only) is the instance the `web_search`
tool queries (`GET {searxngUrl}/search?format=json&q=…`, with
`Authorization: Bearer {searxngApiKey}` when set). The tool runs in fennec,
not in a command: the model only chooses the query.

`sandbox` (fennec's own) is how commands run: the `run_command` tool and the
workflow shell nodes. Enabled (the default), each command runs in bwrap: the
system read-only, the working directory writable, `/tmp` private, the secrets
under `$HOME` (`.ssh`, `.aws`, `.netrc`…) and fennec's data directory hidden,
the environment's credential-looking variables removed, every process ended
with the command, and no network unless `network` is true (npm and cargo are
then set offline). Disabled, commands run on the host, as in openfox.

Protected files: a model may read them, never write them. They are fennec's
defaults (`.agents`, `AGENTS.md`, `CLAUDE.md`, the lockfiles `Cargo.lock`,
`package-lock.json`, `yarn.lock`, `pnpm-lock.yaml`, and the CI `.github`,
`.gitlab-ci.yml`) plus the lines of the project's `.agents/protected`
(gitignore-like: `#` comments, `*`, `?`, `**`, a leading `/` anchors, a
pattern without `/` matches at any depth, a directory protects its content).
`write_file` and `edit_file` refuse them; in the sandbox they are mounted
read-only. The human edits them as usual.

## Endpoints

- `GET /api/config` → `{model, contextWindow}`: the resolved parameters of the
  stored preferences
- `GET /api/settings` → `{preferences, models, defaults}`
  - `preferences`: the stored `Preferences`
  - `models`: `[{id, contextWindow}]` from a fresh `/models` call; on failure
    the startup listing
  - `defaults`: `{model, maxTokens, contextWindow, compactAt}` from the
    environment/built-in defaults
- `POST /api/settings` with a full `Preferences` body → 200 with the same body
  as `GET /api/settings`; 400 with the validation message if invalid
- `GET /api/projects` → `[{id, name, workdir, branch, sessions: [{id, title, state, createdAt, updatedAt}]}]`
  (sessions newest first; `createdAt`/`updatedAt` are Unix milliseconds; `branch` may be `null`)
- `POST /api/projects` `{workdir}` → the project (400 if not a directory)
- `GET /api/projects/{id}/commands` → `[{name, description}]`: the slash commands the composer offers (`compact`, then the project's skills); 404 if no project
- `GET /api/projects/{id}/agents` → `[{id, name, description, color, subagent}]`: the
  project's agents in discovery order (`color` null when unset); the modes are the
  ones with `subagent` false, the others only run in a workflow node's fresh context;
  404 if no project
- `GET /api/folders?path=<absolute path>` → `{path, parent, folders: [{name, path, hidden, repo}]}`,
  the sub-folders of `path` (the home directory when omitted), sorted by name;
  `repo` marks a git repository; 400 if `path` cannot be read
- `GET /api/sessions/{id}` → `{id, title, state, mode, project: {id, name, workdir}, worktree}`;
  `worktree` is `{path, branch, base, editorUrl}` (`editorUrl` opens the copy in VS Code, through the WSL remote under WSL) when the session (or its root session) runs a workflow in
  its own git worktree, else null; the human takes the agent's work with
  `git cherry-pick <base>..<branch>`;
  `mode` is the id of the agent the session resolves to from its event log
  (null when there is no top-level agent)
- `GET /api/sessions/{id}/integration` → `{branch, base, humanBranch, commits: [{sha, subject}], files: [{path, protected}], criteria, blockers}`:
  what integrating the session's worktree branch would do (409 without a
  worktree). `blockers` is the review gate, each `{kind, …}`: `noCommits`,
  `criteriaNotPassed {criteria}`, `noCriteria`, `protectedTouched {paths}`;
  protection is read from the project's tree, not the worktree's.
- `POST /api/sessions/{id}/integrate` `{acknowledged: [kind]}` → `{head, commits}`:
  the human's decision. The server recomputes the gate and answers 409 while a
  turn runs, while a blocker is not acknowledged (`noCommits` never is), or when
  git fails; then it runs `git cherry-pick <base>..<branch>` in the human's
  repository. A conflict is aborted, the tree left as it was, and answers 409
  `{error: "conflict", conflict: {kind: "conflict", files, command}}`. On
  success it moves the base ref to the branch head and appends `integration.applied`. Nothing else in
  fennec integrates: no agent, workflow or schedule calls it.
- `POST /api/sessions/{id}/discard` `{}` → `{commits}`:
  the human's decision; 409 while a turn runs or without a worktree; removes the
  worktree, its branch and base ref in the human's repository and appends
  `worktree.discarded`. A later workflow in the session opens a fresh worktree.
- `GET /api/sessions/{id}/skills` → `{loaded: [ids], active: [ids]}`: the
  skills the session loaded (successful `load_skill` calls and `/<skill>`
  messages it typed, first-seen order) and those still in the model's view
  (not dropped by a compaction, not cut by a replay); 404 if no session
- `POST /api/sessions` `{projectId, prompt, mode?, attachments?}` → `{id}`;
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
- `POST /api/sessions/{id}/workflow` `{workflowId, task, stepByStep?, worktree?}` →
  202 `{}`; 404 if no session or no such workflow; 400 `empty task`; 409
  while a turn or a run is active; `worktree` defaults to true (the session
  gets its own worktree; false makes the agent work in the project's folder;
  an existing worktree is kept)
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
