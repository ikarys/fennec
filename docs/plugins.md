# Writing a plugin

A plugin is the human's own extension: a process that watches session events and shows typed
blocks (text, figures, badges) in the session's left bar or on Settings' Plugins tab. It never
touches the prompt, the tools or a log. Plugins are global, never read from a project.

## Files

`~/.agents/plugins.json`, a list of definitions:

```json
[{"name": "turnstats", "command": "python3", "args": ["turnstats.py"], "env": {},
  "blocks": [{"id": "status", "title": "Turnstats", "scope": "global"},
             {"id": "tools", "title": "Tool calls", "scope": "session"}]}]
```

- `name`: letters, digits, `-`, `_`; the code lives in `~/.agents/plugins/<name>/`.
- `command`, `args`: run with that directory as the working directory.
- `blocks`: what the plugin may show. `global` blocks go to the Plugins tab, `session` blocks
  to each session's left bar. An update for a block not listed here is never shown.

Then enable it on Settings → Plugins. Editing the definition or any file in its directory
deactivates it until the human enables it again (SHA-256 of definition and code).

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
3. The plugin sends blocks whenever it likes:
   `{"jsonrpc":"2.0","method":"block.update","params":{"block":"tools","session":"<sessionId>","content":{"elements":[...]}}}`
   (`session` only for a `session` block). Each update replaces the block's content. Elements:
   `{"kind":"text","text"}`, `{"kind":"figure","label","value","tone"?}`,
   `{"kind":"badge","label","tone"?}`; `tone` is `none`, `info`, `success`, `warning` or `danger`.
   Caps: 200 elements, 32 KiB per block, 16 blocks; an update beyond them is dropped.

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
