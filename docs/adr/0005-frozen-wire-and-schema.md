---
status: accepted
date: "2026-11-10"
---

# The event wire names and the SQLite schema only grow

## Context and problem statement

Recorded after the fact, 2026-11-10. A session is an append-only event log stored in SQLite,
each event `{"type": "...", "data": {...}}`. Logs written by any past version are read and folded
by the current one.

## Decision

- An event type name, a field name or a table column, once released, keeps its name and meaning.
- New event types, fields and columns may be added. A schema change is an appended migration step
  (ADR 0002), never an edit of a released one.

## Consequences

- Old logs fold with the current code without conversion.
- A renaming is done in the code only, never on the wire or in the schema.
