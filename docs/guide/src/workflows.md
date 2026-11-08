# Workflows

A workflow is a graph you draw to run a task in steps. fennec walks the graph deterministically; models run only inside agent nodes and do not choose the path on their own.

## What a workflow gives you

- Nodes do one thing each: run an agent, run a shell command, ask you, test a condition, run criteria, branch in parallel, or end.
- Edges are wired by named output ports, not by an ordered list.
- Loops have caps. Parallel branches have one join.
- The run's state is folded from the session log, so a reload shows where it is.
- Integration stays a human decision: a workflow cannot commit to your branch.

## Embedded workflows

| Workflow | Use it for |
| --- | --- |
| **Plan, build, verify** | A full change. It plans, waits for approval, writes tests from the **Done when** criteria, builds, runs criteria checks, verifies other criteria in a fresh context, reviews the diff, and sends findings back to the builder. Findings left after two rounds go to you. |
| **Quick build** | A small direct build. It builds the task in the session's tree and stops; no plan, no criteria, no review. |
| **Write documentation** | Project documentation in the session's tree: README, `docs/`, guides, API references, true to the code. |
| **Write a training course** | Research a subject, write course content, then generate self-contained HTML slides in the session's tree. |

## Running a workflow

In a session, choose a workflow in the **Workflow** picker next to the mode picker. The message you type is the task.

Tick **Step by step** to pause before each node. A strip above the thread shows the run's nodes and their state. Each node run appears as a divider.

A paused edge stops before its target. The card shows **Continue** and **Stop**, with an optional comment. A comment is appended to the next agent node's prompt.

A human node pauses the run and shows one button per choice plus a comment field. While a run waits, a typed message is refused; answer the node or stop the run.

If a run is interrupted, you can **Resume** it, which re-runs the node that was running, or stop it. Workflow runs need the UI for their pauses; the CLI does not run them.

## Node kinds

| Node | What it does | Output ports |
| --- | --- | --- |
| **Start** | Exactly one per workflow; carries the task. | `next` |
| **Agent** | Runs an agent with a prompt. `Context` can be **Session** or **New session**. **Verdicts on criteria** lets it judge criteria without a command. | Its **Outputs**, default `done` |
| **Shell** | Runs a command with a timeout and success codes. | `success`, `failure` |
| **Human** | Pauses for your choice. | One port per choice |
| **Condition** | Tests a template value with `equals`, `contains`, or `empty`. | `true`, `false` |
| **Criteria** | Reads the **Done when** list from a plan node, records it as the contract, and runs each `check:` command itself. | `pass`, `fail` |
| **Parallel** | Starts several branches at once. | `branch` |
| **Join** | Waits for all branches, then continues. | `next` |
| **End** | Ends the run with an outcome. | — |

An agent node with one output port ends when the agent ends its turn. With several output ports, the agent must end its last message with one line `Outcome: <port>`. If it does not, it is asked once more; if it still gives none, the run ends **blocked**.

## Human nodes and pauses

A human node has a question and at least one choice. The run pauses and the session waits for you. The UI shows the question with one button per choice and a comment field.

A paused edge stops the run before its target. A run started step by step pauses on every edge without changing the workflow file. A comment typed at a pause or at a human node is appended to the next agent node's prompt.

## Loops and caps

An edge may go back to any node. **Max passes** on an edge caps how many times the run takes it. When the cap is reached, the run follows the source node's `exhausted` port. When that port is not wired, the run ends **blocked**.

A workflow also has a step cap, default 50, as a guard against a graph that validation could not prove finite.

## Parallel branches

A **Parallel** node starts every edge of its `branch` port at once. Each branch runs in a child session up to the **Join** node all branches lead to. The **Join** waits for all of them, then continues in the parent.

Branches share the working directory, so at most one branch may contain a node that writes: an agent whose tools include file writes or command execution, or any shell node.

A branch never pauses: no human node and no paused edge inside a branch, and step by step does not pause inside one. A branch's agent nodes always run in new sessions. An `ask_user` inside a branch ends that branch **blocked**. Nested parallel nodes are not supported yet.

## Templates

Prompts, commands, condition values, and questions may use:

- `{{task}}`: the message the workflow was started with
- `{{nodes.<id>.output}}`, `{{nodes.<id>.port}}`: the latest run of a node
- `{{nodes.<id>.exitCode}}`: for a shell node
- `{{filesChanged}}`: the files changed since the run started, one per line
- `{{pass}}`: how many times the current node has run, from 1
- `{{comment}}`: the latest comment typed at a pause or a human node
- `{{criteria}}`: the run's criteria, one per line, with their check and status

A reference to a node that has not run yet expands to an empty string. An unknown name is a validation error. Shell commands receive the values shell-quoted.

## The editor

Open **Workflows** from the top bar or command palette. A workflow is found per project.

- Drag node shapes from the palette onto the canvas.
- Draw wires from a node's named ports.
- The side panel edits the selected node: its fields, the prompt with the template names it can use, and the output ports.
- The side panel edits the selected edge: **Max passes** and **Pause before the target**.
- Edges are styled by role: forward edges grey, loops orange, `exhausted` edges red and dotted, paused edges dashed.
- Validation errors are marked on the graph.
- **Test step by step** opens a new session with this workflow, step by step.

## Where workflow files live

A workflow file is `<id>.workflow.json`. fennec looks in this order for an id:

1. the project's `.agents/workflows/`
2. `~/.agents/workflows/`
3. the defaults embedded in fennec

The first file found wins. A project workflow therefore overrides a global one, and a global one overrides an embedded default.

The id must match `^[a-z0-9][a-z0-9-]*$` and equals the file name. The editor saves into the project or global folder. An embedded default is copied on its first save.

## Validation

Validation is checked on save and before a run, and shown in the editor next to the faulty node or edge:

- one **Start**, at least one **End**, unique ids, known agent ids
- every output port wired at most once, except `parallel.branch`
- every node reachable from **Start**, and an **End** reachable from every node
- every **Parallel** has a matching **Join** that all its branches reach, disjoint branches entered only from the **Parallel**, no pause inside, at most one writing branch
- templates use known names
- a cycle without any **Max passes** edge, paused edge, or **Human** node on it is an error
