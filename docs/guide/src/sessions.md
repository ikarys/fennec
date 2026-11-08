# Sessions

A session is where you work with an agent on a project. This page covers projects, sessions, agents, the composer, context, and the context panel.

## Projects and sessions

The home page lists your projects and recent sessions. Use **New chat** to start a conversation without a project, folder, or git.

Open a project from the home page. The project page shows **Recent sessions** and **All sessions**, and offers **New session in this project**. The session list shows **Title**, **Status**, **Modified**, and **Agent**. A session with unintegrated work shows **to integrate**.

Session titles appear in the top bar tabs and the project session list. The top bar keeps open session tabs. You can close a tab, use split view, and open the command palette to search a session, project, page, command, or skill. Themes and display options are on **Settings** > **Display**.

Session states include **Running**, **Complete**, **Stopped**, **Error**, **Idle**, **Waiting for you**, **Awaiting your decision**, **Paused**, **Interrupted**, **Workflow running**, and **Awaiting your answer**.

## Agents and modes

An agent is a session mode: its method and the tools it may use. fennec provides embedded agents, you can add yours in `~/.agents/agents`, and a project can define agents in `.agents/agents`.

The composer menu groups agents by phase: **Start**, **Frame**, **Build**, **Understand**, and **Check**.

| Agent | Use | Access |
| --- | --- | --- |
| **Plan** | Explore the codebase and frame a plan | Read-only |
| **Builder** | Implement the task and verify it | Write allowed |
| **Question** | Answer a project question with sources | Read-only |

Switch the session agent from the composer's **Session agent** picker. The **Mode** picker shows the current mode. You can also change the model for a session from the composer. Agents marked **Subagent** run only inside workflow nodes and are not offered as free session modes.

See [extending](extending.md) for writing your own agents.

## The composer

The composer is the **Message** input at the bottom of the session. Its placeholder is **Ask something… (@ for a file, / for a command)**.

- `Enter` sends; `Shift+Enter` adds a new line.
- `@` references a file.
- `/` opens commands and skills.
- **Attach an image** adds an image; you can also paste with `Ctrl+V` or drop a file. fennec accepts up to 10 files and 10 images, each at most 5 MB.

While a turn runs, a typed message is queued. Queued messages appear above the composer with **Queued** and **Remove from the queue**. fennec drops queued messages into the running turn between tool rounds. A slash command waits. The first queued message left starts the next turn when the current one ends. **Stop** clears the queue. A pending approval, waiver, or unfinished workflow run holds the queue until it is resolved.

**Stop** stops the turn and clears the queue. **Pause** asks a running workflow to stop after the current step. Open tool calls are closed after a stop or crash.

Slash commands include:

```text
/compact
/<skill>
```

`/compact` summarizes the conversation to free context. `/<skill>` loads a skill. Skills are `SKILL.md` packages from `~/.agents/skills` and the project's `.agents/skills`. Always-active skills are set on **Settings** > **Agents & skills**.

## Questions and approvals

When the agent's `ask_user` question appears, the turn ends and the answer is the next input. The question card shows **Question**, **Yes**, **No**, and **Or answer freely in the input area.**

Rule approvals and waiver requests also pause the turn. You answer on a card. A waiver grants a protected file for 1, 8, or 24 hours in that session and writes an ADR to the session's `docs/adr/`. See [rules](rules.md) and [the review gate](review-gate.md#waivers).

## Delegated child sessions

With **Allow delegation** on in **Settings** > **Behaviour**, the **Plan**, **Builder**, and **Question** agents can hand a self-contained task to a read-only agent in a hidden child session. The parent waits and receives only the child's final answer. The tool card shows the agent and task, marks the answer **indicative**, and shows the time the child ran. A child cannot delegate further. Only top-level web turns delegate.

## Compaction and context

Automatic compaction runs when the context reaches the threshold set by **Compact at (%)** in **Settings** > **Models**, or by `FENNEC_COMPACT_AT`. The default is `0.85`; `0` disables automatic compaction. You can also type `/compact`.

Compaction opens a new window from the model's summary plus facts taken from the log, such as files changed and the last request. The log stays append-only.

The context panel's **Model context** page shows **Session tokens**, **Calls**, **Estimated cost**, **Last call**, and **Composition**. Use **Compact** to compact manually.

## Replaying a message

You can replay a typed message. fennec sends it again and keeps the log append-only.

## The context panel

The right-hand context panel shows what the model sees and what fennec computed. It has pages such as **Request**, **Criteria**, **Modified files**, **Agent's workspace**, **Servers**, **Skills**, **Decisions**, **Tokens**, **Vision**, **Plugins**, and **Status bar**.

Use **Panel page** to switch pages. Drag the edge to change **Panel width**. You can add, remove, and reorder pages on the **Context panel** section of **Settings**.

fennec marks items **verified** when it computed them deterministically and **indicative** when a model produced them.

- **Criteria** shows the plan's **Done when** list, check results, and agent verdicts.
- **Modified files** links to the **Changes** page.
- **Agent's workspace** shows the session's worktree and buttons to open a terminal, editor, or file manager there.
- **Servers** starts the project's servers in the worktree with **Try the change** or **Try it**.
- **Skills** lists skills loaded into the model's context.
- **Decisions** lists choices the agent recorded.
- **Vision** shows images the model has seen; images out of context are dimmed.

## Session workspace and changes

When a session uses a worktree, it works in a git worktree on branch `fennec/<session-id>` under `~/.local/share/fennec/worktrees/`. Your checkout is not written until you integrate.

The **Changes** page lists the branch's files in review order, shows the selected file's diff against the session's base, and can open the whole file in VS Code. Diffs are cut at 5,000 lines.

The session panel's buttons open your terminal, editor, or file manager in the worktree. These tools run outside the sandbox with your environment. See [the sandbox](sandbox.md).

## Deleting sessions

From the project page, use **Delete the session**. A running session cannot be deleted. If a session has unintegrated work, fennec warns that its commits will be lost. Deletion removes the thread, workflow steps, agent workspace, and git branch. Usage figures remain in **Insight**. The action is permanent.

## Where to go next

- [Plans](plans.md)
- [Workflows](workflows.md)
- [The review gate](review-gate.md)
- [Tasks](tasks.md)
- [Understanding](understanding.md)
- [Settings](settings.md)
- [CLI](cli.md)
