---
status: accepted
date: "2026-11-10"
---

# Reusable, fennec-free concerns live in their own crates

## Context and problem statement

Recorded after the fact, 2026-11-10. Some of fennec's concerns know nothing of sessions, agents
or the web server, and another product could use them as they are.

## Decision

A reusable concern that needs no fennec type gets its own crate:

- `crates/backlog`: ideas jotted, framed into a plan and run as a queue; an append-only log of
  `BacklogEvent` folded into a `Backlog`, with no I/O.
- `crates/rules`: the human's rules (model, matching, decision) in their own SQLite file; the host
  asks what to do and enforces the answer.
- `crates/canvas`: versioned canvas artifacts owned by a project and session, knowing only string
  ids and their own SQLite file.
- `crates/token-usage`: an append-only SQLite ledger of LLM calls answering aggregate queries.
- llm-providers-connector, on crates.io: talking to models.

## Consequences

- Each crate states in its header what it holds, and imports nothing from `crates/fennec`.
- A concern that needs fennec types stays in `crates/fennec`.
