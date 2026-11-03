# Workflows

A workflow is a graph the human draws: nodes that do one thing each, wired
by their named outputs. fennec walks the graph deterministically; models
only run inside agent nodes and never choose the path on their own, except
through the output ports a node declares. Everything a run does is in the
session's event log, so its state is folded from the log like the rest of
fennec.

A workflow has steps of kind agent, shell and user; first-entry and re-entry
prompts; terminal outcomes; an iteration cap; fan-out to parallel lanes with a
synthesis, and a bounded verification loop (the last two borrowed from
Opencode-Workflows). They are nodes, not instructions to a model: outputs are
ports wired in the graph instead of an ordered list of conditions; loops have
their own cap; parallel branches; nodes keep their position for the editor.

## Moving from one node to the next

Three levels, chosen when drawing the graph:

| level | how |
|---|---|
| automatic | a plain edge (the default) |
| pause to review | `"pause": true` on an edge, or the run started step by step |
| a decision that changes the path | a `human` node and its choices |

A paused edge stops the run before its target: the UI shows Continue and
Stop, with an optional comment. A run started with `stepByStep` pauses on
every edge, without changing the file. A comment typed at a pause or at a
human node is appended to the next agent node's prompt
(`\n\nHuman comment: …`).

Integration stays a human decision whatever the graph: a workflow cannot
commit to the human's branch (rule 1), even when every edge is automatic.

## Files

`<id>.workflow.json` in, by priority (the first found wins for an id):

1. the project's `.agents/workflows/`
2. `~/.agents/workflows/`
3. the defaults embedded in the binary (`crates/fennec/workflows/`)

The embedded defaults are `plan-build-verify` (plan, build, verify), `quick` (small direct build), `write-docs` (project documentation), and `write-training` (a training course).

`id` matches `^[a-z0-9][a-z0-9-]*$` and equals the file name. The editor
saves into the project's or the global folder; an embedded default is
copied on its first save.

## Format

```json
{
  "id": "plan-build-check",
  "name": "Plan, build, check",
  "description": "Plan, wait for approval, build, run the checks, loop on failure.",
  "maxSteps": 50,
  "nodes": [
    { "id": "start", "kind": "start", "position": { "x": 0, "y": 0 } },
    { "id": "plan", "kind": "agent", "name": "Plan", "agent": "plan",
      "prompt": "{{task}}", "position": { "x": 0, "y": 120 } },
    { "id": "approve", "kind": "human", "name": "Approve the plan",
      "question": "Go with this plan?", "choices": ["Go", "Revise"],
      "position": { "x": 0, "y": 240 } },
    { "id": "build", "kind": "agent", "name": "Build", "agent": "builder",
      "prompt": "Execute the plan above.",
      "retryPrompt": "The checks failed:\n\n{{nodes.check.output}}\n\nFix it.",
      "position": { "x": 0, "y": 360 } },
    { "id": "check", "kind": "shell", "name": "Checks", "command": "just check",
      "timeoutSecs": 900, "position": { "x": 0, "y": 480 } },
    { "id": "done", "kind": "end", "outcome": "done", "position": { "x": 0, "y": 600 } },
    { "id": "stuck", "kind": "end", "outcome": "blocked", "position": { "x": 240, "y": 600 } }
  ],
  "edges": [
    { "from": "start", "port": "next", "to": "plan" },
    { "from": "plan", "port": "done", "to": "approve" },
    { "from": "approve", "port": "Go", "to": "build" },
    { "from": "approve", "port": "Revise", "to": "plan" },
    { "from": "build", "port": "done", "to": "check" },
    { "from": "check", "port": "success", "to": "done", "pause": true },
    { "from": "check", "port": "failure", "to": "build", "maxPasses": 3 },
    { "from": "check", "port": "exhausted", "to": "stuck" }
  ]
}
```

Unknown fields are kept on save (the editor round-trips them).

### Nodes and their ports

| kind | fields | output ports | output value |
|---|---|---|---|
| `start` | (exactly one per workflow) | `next` | the task |
| `agent` | `agent` (agent id), `prompt`, `retryPrompt?`, `outputs?` (default `["done"]`), `context?` (`"session"` default, or `"fresh"`), `verdicts?` (default false) | each of `outputs` | the agent's last text |
| `shell` | `command`, `timeoutSecs?` (default 600), `successCodes?` (default `[0]`) | `success`, `failure` | stdout and stderr, last 8 KiB |
| `criteria` | `plan` (the id of the agent node whose output holds the plan) | `pass`, `fail` | the failed checks: command, exit code, output |
| `human` | `question`, `choices` (at least one) | one per choice | the choice, plus the comment typed with it |
| `condition` | `value` (a template), `test` (`equals`, `contains`, `empty`), `against?` | `true`, `false` | the evaluated `value` |
| `parallel` | | `branch` (several edges allowed) | |
| `join` | | `next` | the outputs of the branches, each under a `### <node name>` heading |
| `end` | `outcome` (`done` or `blocked`) | | |

Every node has `id`, `name?` and `position`. A node with a capped edge
(see Loops) also has the `exhausted` port.

**Agent node.** `context: "session"` runs in the session itself: the
node's agent becomes the mode (`mode.changed` and its reminder, as when the
human switches), the prompt is appended as a user message marked as coming
from the workflow, and the agent loop runs. The conversation carries over
from node to node, so the cached prefix keeps growing. `context: "fresh"`
runs the node in a child session that starts from the prompt only (see
Child sessions). Images attached to a workflow start are appended as a user
message before `workflow.started`, so session-context nodes see them and
fresh-context nodes do not. A sub-agent (`subagent: true`, as `verify` and
`review`) is never a mode of the session: it runs only with `context: "fresh"`. With one output port, the node ends when the agent ends
its turn. With several ports, the prompt asks the agent to end its last
message with one line `Outcome: <port>`. A tool would change the tool list,
which is part of the cached prefix, and force a re-read of the session.
If the turn ends without a valid outcome line, the agent is asked once
more. If it still gives none, the run ends `blocked`. The node's output is
its last text, without the outcome line. The first entry sends `prompt`. A re-entry
sends `retryPrompt` when set, else `prompt` again.

**Human node.** The run pauses and the session waits for the human
(`waiting_for_user`, as with ask_user). The UI shows the question with one
button per choice and a comment field. While a run waits at a human node
or a pause, a typed message is refused: the human answers, or stops the
run. An ask_user inside an agent node is answered as usual, then the run
goes on.

**Criteria node.** No model. It reads the "Done when" list of the plan
node's latest output: each bullet is a criterion (`C1`, `C2`…), and one that
ends with `— check: \`<command>\`` carries a check. When the list differs
from the run's current criteria, it records it (`criteria.set`): the plan the
human approved is the contract, and no tool lets a model change it. A check
is part of that contract: it must hold for any correct implementation of the
work, not one implementation, and it must run in the sandbox (the main
repository is read-only). A builder that cannot pass a check for those
reasons proposes a corrected list (`propose_criteria`); the human decides.
The node then
runs each check itself, as a shell node does (in the sandbox), and records
the result (`criterion.updated`) as **verified**. The node takes `pass` when
every checked criterion passed; criteria without a check do not count here.
It runs in the main walk only, not in a parallel branch.

**Verdicts.** An agent node with `verdicts: true` judges the criteria
without a check: each line `C<n>: pass — <why>` or `C<n>: fail — <why>` of
its output is recorded as that criterion's status, marked **indicative**. A
verdict on a criterion that has a check is ignored: a model never overrides
a verified fact. Main walk only.

**Condition node.** No model. `value` is a template, `against` a literal.
`empty` is true for an empty or whitespace-only value.

### Templates

Prompts, commands, condition values and questions may use:

- `{{task}}`: the message the workflow was started with
- `{{nodes.<id>.output}}`, `{{nodes.<id>.port}}`: the latest run of a node
- `{{nodes.<id>.exitCode}}`: for a shell node
- `{{filesChanged}}`: the files changed since the run started, one per line
- `{{pass}}`: how many times the current node has run, from 1
- `{{comment}}`: the latest comment typed at a pause or a human node
- `{{criteria}}`: the run's criteria, one per line, with their check and status

A reference to a node that has not run yet expands to an empty string. An
unknown name is a validation error. Shell commands receive the values
shell-quoted.

### Loops

An edge may go back to any node. `maxPasses` on an edge caps how many times
the run takes it. When the cap is reached, the run follows the source
node's `exhausted` port. When that port is not wired, the run ends
`blocked`. `maxSteps` (default 50) caps the node runs of the whole run, as
a guard against a graph the validation could not prove finite.

### Parallel branches

A `parallel` node starts every edge of its `branch` port at once. Each
branch runs in its own child session, up to the `join` node all branches
lead to. The `join` waits for all of them, then continues in the parent.
The branches share the working directory. So at most one branch may hold
a node that writes: an agent whose tools include `write_file`, `edit_file`
or `run_command`, or any shell node. The validation rejects any other
graph. A branch never pauses: no human node and no paused edge inside a
branch, and step by step does not pause inside one. A branch's agent nodes
always run in child sessions (`fresh`), and an ask_user there ends the
branch `blocked`. Nested parallel nodes are not supported yet. Only the
coordinator writes to the parent log: branches send it their events.

### Validation

Checked on save and before a run, by the server (`domain/workflow`), and
shown in the editor next to the faulty node or edge:

- one `start`, at least one `end`, unique ids, known agent ids;
- every output port wired at most once, except `parallel.branch`;
- every node reachable from `start`, and an `end` reachable from every node;
- every `parallel` has a matching `join` that all its branches reach,
  disjoint branches entered only from the `parallel`, no pause inside,
  at most one writing branch;
- templates use known names;
- a cycle without any `maxPasses` edge, paused edge or `human` node on it
  is an error. A pause bounds a loop: the human is asked each time.

## Events

All events go to the parent session's log. Their names follow fennec's own
event style:

| event | data |
|---|---|
| `workflow.started` | `runId`, `workflow` (the whole definition, frozen for the run), `task`, `stepByStep` |
| `workflow.node.started` | `runId`, `nodeId`, `pass`, `via?` (`{from, port}`, the edge taken), `childSessionId?`, `startedAt?` (unix ms, so a running node's elapsed time survives a reload) |
| `workflow.node.finished` | `runId`, `nodeId`, `port`, `output`, `exitCode?` |
| `workflow.paused` | `runId`, `from`, `port`, `to`: waiting before `to` |
| `workflow.resumed` | `runId`, `comment` |
| `workflow.finished` | `runId`, `outcome` (`done`, `blocked`, `stopped`, `error`), `reason?` |

`via` is how the caps are counted: an edge's passes are the node starts
that came through it.

The run's state is folded from these events: the current node(s), the
passes per edge, and the latest output per node. The next step is a pure
function of the definition and that state. A run without
`workflow.finished` after a restart shows as interrupted: the human may
resume it, which re-runs the node that was running, or stop it. Stopping
ends the current node like a stopped turn, then appends
`workflow.finished {outcome: "stopped"}`.

## Child sessions

A child session is an ordinary session with a parent. `sessions` gains a
nullable `parent_id` column, added at startup when missing. Child sessions
are hidden from the lists, open from the parent's thread, and are
read-only to the human while their node runs.

## API

- `GET /api/projects/{id}/workflows`: the workflows found for the project,
  with their origin (`project`, `global`, `default`).
- `GET /api/projects/{id}/workflows/{wid}`: one definition.
- `PUT /api/projects/{id}/workflows/{wid}` `{scope: "project"|"global", workflow}`:
  saves after validation. 400 with `{errors: [{nodeId?, edge?, message}]}`.
- `POST /api/workflows/validate` `{projectId, workflow}`: the same errors, for the
  editor as you type.
- `POST /api/sessions/{id}/workflow` `{workflowId, task, stepByStep?}`: starts a run
  (409 while a turn or a run is active). `POST /api/sessions` accepts
  `workflowId` and `stepByStep` too.
- `POST /api/sessions/{id}/workflow/answer` `{runId, choice, comment}`: a human node.
- `POST /api/sessions/{id}/workflow/continue` `{runId, comment}`: a pause.
- `POST /api/sessions/{id}/workflow/resume` `{runId}`: an interrupted run.
- `POST /api/sessions/{id}/workflow/stop` `{runId}`: ends a run that is not
  running (waiting or interrupted); a running one is stopped by the
  existing `stop`.

Not in the CLI yet: a run needs the UI for its pauses.

## UI

- **Editor** (`#/workflows`, `#/workflows/<projectId>/<id>`: a workflow is found per project), on React Flow, loaded only on that page:
  - a palette of node shapes to drag onto the canvas: start and end as
    pills, agent as a rounded box with the agent's colour, shell as a
    terminal block, human as a hexagon, condition as a diamond, parallel
    and join as bars;
  - wires drawn from a node's named ports;
  - the side panel edits the selected node: its fields, the prompt with
    the template names it can use (the inputs) offered as chips, and the
    output ports;
  - the side panel edits the selected edge: `maxPasses` and `pause` (a
    paused edge is drawn dashed with a pause icon);
  - edges styled by role, with a legend: forward edges grey, loops (an
    edge back up the graph) orange and routed in a lane beside every node
    they pass, `exhausted` edges red and dotted, paused edges dashed;
  - zoom, pan, minimap, and validation errors marked on the graph.
- **Session**: a workflow picker next to the mode picker, with a step by
  step toggle; the message typed is the task. A strip above the thread shows the run's nodes with
  their state. Each node run appears in the thread as a divider. A human
  node shows as a choice card.
