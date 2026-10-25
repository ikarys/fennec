# fennec HTTP API (web UI ↔ server)

The server listens on `127.0.0.1:7777`. JSON in, JSON out. Errors are
`{"error": "<message>"}` with a 4xx/5xx status.

## Events

Every session is an append-only event log (`crates/fennec/src/domain/event.rs`),
the same wire format as openfox's `TurnEvent`: `{"type": "...", "data": {...}}`
with camelCase fields.

| type | data |
|---|---|
| `session.initialized` | `{workdir}` |
| `message.start` | `{messageId, role: "user"\|"assistant", content?, isSystemGenerated?}` |
| `message.delta` | `{messageId, content}` (text chunk) |
| `message.thinking` | `{messageId, content}` (reasoning chunk) |
| `message.done` | `{messageId, partial?, stats?}`; `stats` (assistant messages): `{model, totalTime (s), prefillTokens, cachedPromptTokens, generationTokens, prefillSpeed, generationSpeed}`, speeds in tokens/s, prefill speed over new (non-cached) tokens |
| `tool.call` | `{messageId, toolCall: {id, name, arguments, parseError?}}` |
| `tool.output` | `{messageId, toolCallId, stream: "stdout"\|"stderr", content}` |
| `tool.result` | `{messageId, toolCallId, result: {success, output?, error?, durationMs, truncated}}` |
| `context.state` | `{currentTokens, maxTokens}` |
| `running.changed` | `{isRunning}` |
| `chat.done` | `{messageId, reason: "complete"\|"stopped"\|"error"\|"truncated"}` |
| `chat.error` | `{error, recoverable}` |
| `turn.snapshot` | `{cachedSystemPrompt}`: the session's system prompt, frozen at its first turn |

Tools: `read_file {path, offset?, limit?}`, `write_file {path, content}`,
`edit_file {path, old_string, new_string, replace_all?}`,
`run_command {command, cwd?, timeout?}`.

## Session state

`"running"` while a turn runs, else the reason of the last `chat.done`
(`"complete"`, `"stopped"`, `"error"`, `"truncated"`), or `"idle"` if none.

## Endpoints

- `GET /api/config` → `{model, contextWindow}`
- `GET /api/projects` → `[{id, name, workdir, branch, sessions: [{id, title, state, createdAt, updatedAt}]}]`
  (sessions newest first; `createdAt`/`updatedAt` are Unix milliseconds; `branch` may be `null`)
- `POST /api/projects` `{workdir}` → the project (400 if not a directory)
- `GET /api/folders?path=<absolute path>` → `{path, parent, folders: [{name, path, hidden, repo}]}`,
  the sub-folders of `path` (the home directory when omitted), sorted by name;
  `repo` marks a git repository; 400 if `path` cannot be read
- `GET /api/sessions/{id}` → `{id, title, state, project: {id, name, workdir}}`
- `POST /api/sessions` `{projectId, prompt}` → `{id}`; the turn starts at once
- `POST /api/sessions/{id}/messages` `{content}` → 202; 409 if a turn is running
- `POST /api/sessions/{id}/stop` → 202; the turn ends with `chat.done` reason `"stopped"`
- `GET /api/sessions/{id}/events` → Server-Sent Events. Each stored event is sent as

  ```
  id: <seq>
  data: <event JSON>

  ```

  from the start of the log, then live. On reconnect the browser sends
  `Last-Event-ID` and the stream resumes after it.
