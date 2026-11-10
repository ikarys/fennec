---
status: accepted
date: "2026-11-10"
---

# The agent loop knows the outside world only through ports

## Context and problem statement

Recorded after the fact, 2026-11-10. The agent loop runs a session's steps: it calls the model,
reads and appends the session log, runs tools, starts child sessions and runs workflow shell
nodes. Each of these touches HTTP, SQL or processes, which the loop should not depend on.

## Decision

- The loop depends only on the traits in `agent/ports.rs`: `Model`, `EventLog`, `Toolbox`,
  `ChildSessions` and `Commands`. Adapters implement them (`llm::OpenAiClient`,
  `store::SqliteStore`, `tools::LocalTools`, the web layer).
- Dependencies point inward: adapters → agent → domain; the domain does no I/O.
- A port exists because it decouples a layer or has a test double, not for show.

## Consequences

- A new outside need enters the loop as a method on an existing port or, when it decouples a
  new layer, as a new port; a concrete system stays in its adapter.
- Tests drive the loop with doubles (a scripted model, an in-memory store).
- `.agents/architecture` checks the direction on fennec's own imports
  (`architecture.rs::tests::fennec_keeps_its_own_architecture_rules`).
