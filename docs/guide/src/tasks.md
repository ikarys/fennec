# Tasks

The **Tasks** board keeps your project's ideas and the work that comes from them. It shows what is ready to run, what is running, and what needs your decision.

## Ideas and tasks

The project page keeps your ideas. Add one with **New idea** and **Add the idea**. An idea is a short description of what should change, why, and what makes it done. You can paste an image or attach a file.

Agents can also propose tasks. A card in the thread asks **Which tasks to add to the backlog?** and offers **Add to the backlog** or **Add all**. Proposed tasks are **indicative**, in build order.

Audit findings can become tasks the same way: you choose the findings to keep, then **Create the tasks** adds them to the backlog.

A task in the backlog is not running. It must be framed before it can enter the queue.

## Framing an idea

The board's next action for an idea is usually **Frame**. This opens a plan session tied to the idea. When the plan is approved, the idea is marked **ready** and the plan is kept verbatim with the idea.

You can **Resume framing**, **Reframe**, or **Start without framing** from the idea's **Actions** menu. A framed idea is ready for the queue.

## Stages and statuses

The board groups work by stage:

| Stage | What it contains |
| --- | --- |
| **To decide** | Ideas and tasks that need your choice. |
| **Running** | Sessions that are actively working. |
| **Queue** | Framed ideas ready to run. |
| **Ideas** | Drafts awaiting framing. |
| **Done** | Finished or integrated work. |

Each idea also has a status:

| Status | Meaning |
| --- | --- |
| **idea** | A new idea in the backlog. |
| **framing** | A plan session is open for it. |
| **ready** | The plan is approved and it can run. |
| **running** | The queue is running it. |
| **question** | It needs your answer. |
| **to review** | It has work to review. |
| **halted** | It stopped at a night-mode cap. |
| **failed** | The run failed. |
| **integrated** | Its work was integrated. |
| **done** | The task is complete. |

## The queue

**Start the queue** runs ready ideas one at a time, oldest first. Each idea runs through the default workflow in its own session and git worktree.

The queue bar shows **queue running** or **queue running · night mode**, the next idea, and ready counts. You can **Stop the queue** or **Stop the queue after the current task**.

Integrating an idea's work stays your click, one idea at a time. See [review gate](review-gate.md).

## Night mode

Tick **Night mode** to run the queue while you are away.

In night mode, `ask_user` answers itself: the agent decides its own questions and records each decision with its reason. Changing acceptance criteria is refused.

Per-idea caps come from [settings](settings.md): **Max duration per idea (minutes)** and **Max tokens per idea**. `0` means no limit. When an idea exceeds a cap, it is **halted** and the queue moves to the next one.

## The night report

In the morning, the **Night report** lists each idea run with its duration, tokens, criteria passed, decisions taken, and files changed. It stays visible until you **Mark as read**. You can **Reread** it later.

The report shows failed and running counts. It does not integrate anything; integration remains your click.

## Cross-project view

The **Tasks** page shows a cross-project backlog with counts for **to decide**, **running**, **ready**, and **done**. The top bar shows a **to review** counter for work that needs your eye.

Use it to see which project has work waiting, halted, or ready.

## Managing tasks

Each idea has one next action computed by fennec: **Frame**, **Resume framing**, **Start**, **View the session**, **Answer**, or **Reread**.

The **Actions** menu offers **Reframe**, **Start without framing**, **Restart**, **Mark as done**, **Drop**, and **Remove**.

- **Drop** removes an idea from the queue.
- **Remove** removes a task from the board; its session and history remain.
- **Reorder** changes the order of items in the board.
