# Plans

A plan is the read-only framing of a task. It gives you the steps, the acceptance criteria, and the evidence fennec uses before and after the build.

## How a plan is made

You start a session on the **Plan** agent, or open a plan session from an idea in [tasks](tasks.md). The plan agent is read-only: it explores the codebase and proposes a plan, but it changes nothing.

The plan card in the thread shows **Steps**, **To test**, **To do**, and **Risks** when the agent includes them. Use **Revise** to keep working on the plan with the agent.

## The Done when list

The **Done when** list is the acceptance criteria for the run. Each bullet can carry a check command:

```text
— check: <command>
```

A check must hold for any correct implementation of the work, not for one implementation. It must also run in the sandbox, where the main repository is read-only. If a builder cannot pass a check for those reasons, it proposes a corrected list and you decide. In night mode, changing acceptance criteria is refused.

Criteria without a check are judged by an agent and marked **indicative**.

## Preflight

Before the build, fennec runs the plan's checks in the **Preflight** section. The section is marked **verified** because fennec ran the commands itself before any change.

Preflight results tell you what the baseline proves:

- **fails before the work**: expected, because the criterion is not met yet.
- **already passes**: proves nothing about the upcoming change, so it is sent back to the planner.
- **cannot run here**: an environment problem, also sent back to the planner.

## Launching the build

From the plan card, **Launch** starts the **Plan, build, verify** workflow: criteria are checked, the result is verified in a fresh context, and the diff is reviewed. See [workflows](workflows.md).

**Builder alone, without criteria or review** starts a build without the full gate. **Save to backlog** keeps the plan in the project backlog without launching it now.

The build normally works in a copy of the repository, a git worktree, so you integrate afterwards. If the session works directly in your folder, nothing separates the agent's work from yours.

## Builder report

After the build, the **Report** card lists the **Steps** the builder says it took. The report is **indicative**: its checks are declared by the model, not verified by fennec.

fennec's own criterion checks and fresh-context verification are separate. When fennec computes a result, it is marked **verified**.

## Verdicts and provenance

A criterion with a `check:` command is run by fennec and recorded as **verified**.

A criterion without a check can be judged by an agent node with verdicts. Lines such as `C1: pass — <why>` or `C2: fail — <why>` are recorded as that criterion's status and marked **indicative**. A model verdict never overrides a verified check.

Use **verified** for facts fennec computed deterministically. Use **indicative** for model judgments you still need to review.

## Plan scope

The approved plan lists the files or steps the work should touch. The integration gate compares the branch's files with that scope. Files no step lists become an **outside the plan** signal and a blocker in the [review gate](review-gate.md#plan-scope).

The plan is a contract: the agent can propose more, but you decide what enters your branch.
