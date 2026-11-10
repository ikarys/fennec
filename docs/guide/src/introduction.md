# Introduction

fennec is a local-first coding agent for professional developers who want to stay the authors of their software. This page explains what fennec is, how a typical piece of work flows, and where each part of the product lives.

## What fennec is

fennec runs as one Rust binary with an embedded web UI and a local SQLite store. It works with any OpenAI-compatible model endpoint, including a local model. It supports Linux, macOS, and Windows through WSL2.

You frame the work. The agent loop runs the steps. fennec waits for your review before anything is integrated. Nothing reaches your branch without a human decision.

## The author model

AI-written code is abundant. Understanding, mastery, and responsibility are rare. fennec is built to keep you the author of your software.

- Provenance is shown. Every UI item is marked **verified** when fennec computed it deterministically, or **indicative** when a model produced it.
- The approved plan is a contract. Its **Done when** criteria are checked by commands fennec runs itself, then reviewed by fresh agents that did not write the code.
- Guards catch a model gaming the gate. Protected files, silenced lints or tests, removed assertions, architecture rules, and ADRs can block integration until you accept them.
- Commands run in a sandbox by default. The system is read-only, secrets are hidden, and network is off by default. A workflow runs in its own git worktree.
- Decisions are recorded. Agents record each choice with its reason and alternatives. Night mode can run ready ideas unattended within per-idea caps and leaves a report.

## A typical piece of work

1. You add an idea or open a session.
2. The plan agent frames the work in a read-only session and proposes a plan with **Done when** criteria.
3. You approve the plan and its criteria.
4. The builder works in a dedicated git worktree and sandbox.
5. fennec runs the criterion checks, verifies the result in a fresh context, and reviews the diff with fresh agents.
6. The review gate presents findings. You accept blockers one by one.
7. You click **Integrate**. fennec cherry-picks the session's worktree branch into your repository.

See [plans](plans.md) and [the review gate](review-gate.md) for the details.

## Where things live in the UI

The home page shows your projects, recent sessions, and a **Getting started** checklist. Use **New chat** to talk without a project.

The project page holds **Tasks**, the module map, documentation, sessions, analyses, risk zones, disk space, and indicators.

A session page has the thread, composer, agent and model pickers, workflow picker, context panel, changes, servers, and preview surfaces. The top bar shows open tabs, search, update notices, and a **To review** counter.

**Settings** has **Models**, **Behaviour**, **Security**, **Rules**, **Agents & skills**, **Extensions**, **Display**, and **About**. **Insight** shows model usage. **Workflows** opens the workflow editor.

## Documentation map

- [Install](install.md): platforms, install script, service, and updates.
- [Three ways to work](ways-to-work.md): chat, an agent, or a workflow, and which to pick.
- [First run](first-run.md): setup checklist, model, first project, first session.
- [Sessions](sessions.md): projects, sessions, agents, composer, context panel.
- [Plans](plans.md): framing work and approving criteria.
- [Review gate](review-gate.md): blockers, waivers, and integration.
- [Workflows](workflows.md): graphs, nodes, loops, and runs.
- [Tasks](tasks.md): ideas, queue, night mode, and board.
- [Understanding](understanding.md): questions, audits, and comprehension dialogues.
- [Insight](insight.md): usage and cost over time.
- [The sandbox](sandbox.md): what commands can and cannot do.
- [Rules](rules.md): allow, ask, and block rules.
- [Settings](settings.md): models, providers, security, display, and extensions.
- [Extending](extending.md): agents, skills, and project files.
- [Plugins](plugins.md): writing your own extensions.
- [CLI](cli.md): command-line commands.
- [Remote access](remote-access.md): password and network settings.
- [Configuration](configuration.md): environment variables and defaults.
- [Troubleshooting](troubleshooting.md): common failures and fixes.
