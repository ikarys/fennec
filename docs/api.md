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
| `context.compacted` | `{beforeTokens, afterTokens, summary}`: the cut where a new context window started from `summary`; `afterTokens` is an estimate until the next model call reports the real count |
| `running.changed` | `{isRunning}` |
| `chat.done` | `{messageId, reason: "complete"\|"stopped"\|"error"\|"truncated"\|"waiting_for_user"}` |
| `chat.error` | `{error, recoverable}` |
| `chat.ask_user` | `{callId, question, type?: "text"\|"confirm"\|"choice", options?: [{value, label, description?}]}`: the model asked the human a question; the turn ends and the answer arrives as the next turn's input |
| `turn.snapshot` | `{cachedSystemPrompt}`: the session's system prompt, frozen at its first turn |

Tools: `read_file {path, offset?, limit?}`, `write_file {path, content}`,
`edit_file {path, old_string, new_string, replace_all?}`,
`run_command {command, cwd?, timeout?}`,
`ask_user {question, type?, options?}` (handled by the loop, the turn ends
waiting for the human).

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
      "compactionThreshold": "number | null"
    }
  },
  "display": {
    "reasoningOpen": true,
    "toolResultsOpen": false
  }
}
```

Validation: `maxTokens >= 1`, `contextWindow >= 1024`,
`compactionThreshold` in `0.0..=1.0` (NaN rejected), model ids non-empty.

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
- `GET /api/folders?path=<absolute path>` → `{path, parent, folders: [{name, path, hidden, repo}]}`,
  the sub-folders of `path` (the home directory when omitted), sorted by name;
  `repo` marks a git repository; 400 if `path` cannot be read
- `GET /api/sessions/{id}` → `{id, title, state, project: {id, name, workdir}}`
- `POST /api/sessions` `{projectId, prompt}` → `{id}`; the turn starts at once
- `POST /api/sessions/{id}/messages` `{content}` → 202; 409 if a turn is running;
  when the session waits for the user, `content` is the answer to the pending
  question
- `POST /api/sessions/{id}/compact` → 202; 404 if no session; 409 if a turn is
  running; summarises the history and opens a new context window
- `POST /api/sessions/{id}/stop` → 202; the turn ends with `chat.done` reason `"stopped"`
- `GET /api/sessions/{id}/events` → Server-Sent Events. Each stored event is sent as

  ```
  id: <seq>
  data: <event JSON>

  ```

  from the start of the log, then live. On reconnect the browser sends
  `Last-Event-ID` and the stream resumes after it.
