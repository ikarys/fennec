---
status: accepted
date: "2026-11-09"
---

# Read the session log by window and by kind where reads repeat

## Context and problem statement

A session is an append-only event log in SQLite (`events`, JSON payloads); every view of a
session is a fold over it. Until now each reader called `EventLog::events`, which decodes the
whole session: 87 call sites do.

Measured on a real store (2026-11-09):

- the largest session holds 14.2 MB of events; its window (the events since the last
  compaction) holds 2.0 MB, its decisions 0.3 MB;
- the agent loop decoded the whole session 3 to 6 times per model round;
- the thread view, refetched on every tool call, decoded it once per fetch;
- `fennec.db-wal` had grown to 227 MB and never shrank.

The readers fall in two families: what the model sees (the window, by construction of
compaction) and the session-wide tallies (criteria, queue, decisions, modified files), which
read a few event kinds across the whole log.

## Decision

- The log stays the single source of truth. No derived state is stored.
- Reads that repeat (one model round, one thread fetch) use two targeted reads on the
  `EventLog` port:
  - `window(session)`: the events from the last `context.compacted` on, the cut included,
    so `model_view` over it equals `model_view` over the whole log;
  - `events_of(session, kinds)`: only the kinds asked, filtered in SQL
    (index `idx_events_session_type`).
  The port's default implementations filter `events()` in memory; they are the reference
  the SQL overrides are tested against.
- A whole-session fold read by kind declares its kinds next to its caller
  (`thread_view::SESSION_KINDS`), and a test checks the fold over the targeted reads equals
  the fold over the whole log. A fold that starts reading another kind fails that test until
  the kind is added.
- One-off actions (replay, retry, compact, a workflow node's start) and the review gate keep
  reading the whole log: there, being exactly right matters more than speed.
- Folds that may need events before the cut stay on full reads or read their own kind:
  the agent's mode (`mode.changed`), the stated working directory, and the per-call
  breakdown, which reads the window plus the `turn.snapshot` events before it.
- The WAL is capped back to 64 MB after a checkpoint (`journal_size_limit`).

## Considered options

- **A stored projection** (a table of derived state updated on append): a second truth to keep
  in sync, and a silent divergence when a fold changes. Rejected.
- **An in-memory cache of decoded events**: RAM grows with sessions, against fennec's
  resource rule. Rejected.
- **Converting all 87 call sites**: about 25 kind lists to maintain for gains nobody would
  notice, and a risk on the review gate. Rejected; convert a site when it measurably weighs.
- **Rewriting or pruning history**: stored logs are frozen. Rejected.

## Consequences

- Two ways to read a session; the rule above says which. A one-off reader that starts to
  weigh is converted on its own, with its equality test.
- A new session-wide fold in the thread view must declare its kinds in `SESSION_KINDS`.
- A round's cost follows the window's size, not the session's.
