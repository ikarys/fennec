---
status: accepted
date: "2026-11-10"
---

# Number the SQLite schema changes with `PRAGMA user_version`

## Context and problem statement

The store created its tables with `CREATE TABLE IF NOT EXISTS`, then probed for columns added
later (`parent_id`, `favorite`, `linked_project_id`) and added the missing ones. Each new column
meant one more probe on every start, and the probe could not express anything but "add this
column": no data change, no index rebuild. A database written by a newer fennec opened silently,
with tables the older code did not know.

## Decision

- `store/migrations.rs` holds an ordered list of steps. The database's `PRAGMA user_version`
  counts the steps already applied; on open, each missing step runs in its own
  `BEGIN IMMEDIATE` transaction, together with the version bump.
- Step 0 is the baseline: the old idempotent schema plus the three column probes. It brings both
  a new database and any database from before this change to version 1.
- A database whose version is above the last step is refused: it comes from a newer fennec.
- A released step is never edited or reordered; a schema change appends a step.

## Consequences

- The tables themselves do not change: stored logs read exactly as before.
- A downgrade to a fennec from before this change still opens the database (it ignores
  `user_version`); a downgrade across a later step is refused instead of misreading data.
- Tested by `store::migrations::tests` (new database, idempotence, a pre-versioned database,
  a newer database refused).
