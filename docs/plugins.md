# Writing a plugin

A plugin is the human's own extension: a process that watches session events and shows typed
blocks (text, figures, badges, lists, progress, links, tables) in the session's right-hand context
panel, a project's dashboard, the app's status bar, or on Settings' Plugins tab. It never touches
the prompt, the tools or a log. Plugins are global, never read from a project.

## Files

`~/.agents/plugins.json`, a list of definitions:

```json
[{"name": "turnstats", "command": "python3", "args": ["turnstats.py"], "env": {},
  "blocks": [{"id": "status", "title": "Turnstats", "scope": "global"},
             {"id": "tools", "title": "Tool calls", "scope": "session"}]}]
```

- `name`: letters, digits, `-`, `_`; the code lives in `~/.agents/plugins/<name>/`.
- `command`, `args`: run with that directory as the working directory.
- `icon` (optional): the icon of the plugin's own tab in the session's context rail, a name from
  fennec's set: `activity`, `alert`, `bell`, `branch`, `check`, `clock`, `cpu`, `eye`, `file`,
  `globe`, `layers`, `server`, `terminal`, `tokens`, `puzzle` (the default, and what an unknown
  name becomes). A plugin never ships an image.
- `blocks`: what the plugin may show. Each block has a `scope` — `session` (per session),
  `project` (one block per project), or `global` (the whole server, the default) — and an optional
  `placement`, which picks the surface within that scope (its default is the scope's, below). An
  update for a block not listed here is never shown.

| scope | placement (default first) | where it shows |
| --- | --- | --- |
| `session` | `panel` (default) | the "Plugins" tab of the session's context rail (shown while an active plugin declares such a block) |
| `session` | `own_panel` | a rail tab of the plugin's own, with its `icon`, one panel per block |
| `session` | `status_bar` | the app's bottom status bar while that session is on screen |
| `project` | `dashboard` (default) | a section of the project page's dashboard |
| `project` | `status_bar` | the status bar while a page or session of that project is on screen |
| `global` | `settings` (default) | Settings' Plugins tab |
| `global` | `status_bar` | the status bar, everywhere |

The status bar shows itself while at least one block is placed there and stays hidden otherwise.

Then enable it on Settings → Plugins. Editing the definition or any file in its directory
deactivates it until the human enables it again (SHA-256 of the command, arguments, environment
and code; the blocks and the icon only choose where data shows).

## The world it runs in

Always the bwrap sandbox (no sandbox, no plugin): its code directory read-only, a private
writable scratch, `/tmp`, no project mounted, no network unless the human ticks it. Its stderr
is discarded.

## Protocol

JSON-RPC 2.0, one JSON object per line, over stdin/stdout.

1. fennec sends `{"jsonrpc":"2.0","id":1,"method":"init","params":{"name","network","blocks"}}`.
   Answer within 5 s with `{"jsonrpc":"2.0","id":1,"result":{"wantedEvents":["tool.call"]}}`:
   the event types it wants (wire names, `docs/api.md`).
2. fennec sends each wanted event as a notification:
   `{"jsonrpc":"2.0","method":"event","params":{"sessionId","projectId","seq","type","event"}}`.
   `event` is a **string**: the stored event JSON, `{"type": "...", "data": {...}}`; for
   `tool.call`, the tool's name is at `data.toolCall.name`. A session's history before the
   plugin started is not replayed.
3. fennec's own resources: a plugin whose `init` result also holds `"resources": true` gets,
   every 5 s,
   `{"jsonrpc":"2.0","method":"resources","params":{"rssBytes":<resident memory in bytes>,"cpuPercent":<CPU % of one core since the previous message, or null on the first>}}`.
   fennec measures its own process only (not the commands and plugins it starts), on Linux;
   elsewhere no message is sent. A plugin that did not ask costs nothing.
4. The plugin sends blocks whenever it likes:
   `{"jsonrpc":"2.0","method":"block.update","params":{"block":"tools","session":"<sessionId>","project":"<projectId>","content":{"elements":[...]}}}`
   (`session` for a `session` block, `project` for a `project` block, neither for a `global` one).
   Each update replaces the block's content. Elements:
   `{"kind":"text","text"}`, `{"kind":"figure","label","value","tone"?}`,
   `{"kind":"badge","label","tone"?}`, `{"kind":"list","items":[{"text","tone"?}]}`,
   `{"kind":"progress","label","value","max","tone"?}`, `{"kind":"link","label","url"}`,
   `{"kind":"table","columns":[…],"rows":[[…]]}`; `tone` is `none`, `info`, `success`, `warning`
   or `danger`. Caps: 200 elements, 32 KiB per block, 16 blocks; an update beyond them is dropped.
   An update is dropped the same way when it holds a `javascript:` or `data:` link (a link's `url`
   must start with `http://` or `https://`), a non-finite progress or one with `max <= 0`, or a
   table with more than 100 rows or a row longer than its `columns`.

## Example

`~/.agents/plugins/turnstats/turnstats.py`, counting each session's tool calls by name:

```python
import collections, json, sys

counts = collections.defaultdict(collections.Counter)

def send(message):
    sys.stdout.write(json.dumps(message) + "\n")
    sys.stdout.flush()

for line in sys.stdin:
    msg = json.loads(line)
    if msg.get("method") == "init":
        send({"jsonrpc": "2.0", "id": msg["id"], "result": {"wantedEvents": ["tool.call"]}})
        send({"jsonrpc": "2.0", "method": "block.update", "params": {"block": "status",
              "content": {"elements": [{"kind": "badge", "label": "running", "tone": "success"}]}}})
    elif msg.get("method") == "event":
        params = msg["params"]
        name = json.loads(params["event"])["data"]["toolCall"]["name"]
        counts[params["sessionId"]][name] += 1
        elements = [{"kind": "figure", "label": tool, "value": str(n), "tone": "info"}
                    for tool, n in counts[params["sessionId"]].most_common()]
        send({"jsonrpc": "2.0", "method": "block.update", "params": {
              "block": "tools", "session": params["sessionId"], "content": {"elements": elements}}})
```

## A second example: fennec's resources

`examples/plugins/resources/resources.py` asks for fennec's own resources and places a global
block in the status bar. Copy the `examples/plugins/resources/` directory to
`~/.agents/plugins/resources/`, add the definition from its `plugin.json` to
`~/.agents/plugins.json`, and enable it on Settings → Plugins. It shows fennec's resident
memory and CPU, measured by fennec's own process on Linux.
