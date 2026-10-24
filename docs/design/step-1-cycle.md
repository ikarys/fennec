# Step 1 — the cycle end to end, simple version

Capabilities C1–C5, C8, C9, C26 of fennec's v1 definition, on top of the
protection that already exists (guard, protected paths, human landing,
Journal). The kernel is hardened later (step 3); until then the UI says the
guarantees are partial.

## Project files

Both are read at the project's `HEAD`, never from a world, so an agent cannot
change what judges it.

- `ARCHITECTURE.md` (root): the architecture vision, written by hand in this
  step (fennec proposes it from step 2). Given to the design and the review.
- `.fennec-os/zones.json`: the sensitive zones.

  ```json
  {"zones": [
    {"name": "kernel", "kind": "security", "paths": ["crates/fennec-os/src/kernel/**"]}
  ]}
  ```

  `kind` is `architecture`, `security` or `sensitive`. `ARCHITECTURE.md` and
  `.fennec-os/zones.json` are always an `architecture` zone.

## The cycle

| # | Step | Who | Provenance |
|---|------|-----|------------|
| 1 | Objective: the conversation ends on a brief (as before) | model + dev | model |
| 2 | Design: done when, how the change fits the vision, plan, files it expects to touch | model | model |
| 3 | Risk: the expected files matched against the zones | kernel | verified |
| 4 | Tests: sensitive → acceptance tests written first (`spec::compile`), protected once validated; otherwise the project's own suite | model, then dev | model / verified |
| 5 | Validation stop: the dev validates design (+ tests) | dev | — |
| 6 | Code in a world (as before) | model | — |
| 7 | AI review against vision, plan and done when; its report is always kept | model | model |
| 8 | Proof (as before), see and try (page, open the worktree) | model + kernel | verified |
| 9 | Human review, then integration by the dev | dev | — |

### Risk-proportional rule (deterministic)

- **Sensitive task** (an expected file in a zone): the design and the tests
  must be validated before code; the human review must be marked done before
  landing.
- **Other task**: the dev may start without validating ("not validated") and
  land without marking the review ("not reviewed"); both are recorded on the
  task and shown.
- A non-sensitive task cannot touch a zone: every zone's paths are protected
  paths of its solve (the existing `protect` rule fails an attempt that
  touches them). Needing one means the task was mis-framed: the dev corrects
  it and it is designed again.

### Stop points (the dev's loop)

Per project, in the project page:

- `design` (on by default): wait for the dev's validation after step 5. Off,
  a non-sensitive task goes straight to code, marked "not validated". A
  sensitive task always stops.
- `code` (off by default): wait after step 6, before review and proof, so the
  dev can look at the work first.
- Integration always stops.

## Where it lives

- `fennec-os` `kernel/zones.rs`: zones and the vision at `HEAD`, `classify`.
- `fennec-os` `engine/spec.rs`: `frame` becomes the design (prompt and parse);
  the vision goes in its context; `goal` carries architecture and plan, so
  solve and review read them.
- `fennec-os` `engine/review_stage.rs`: the vision in the review's ask; the
  report kept when the review passes too.
- `fennec-lead` `work.rs`: risk, tests-first when sensitive, the zones
  protected for other tasks, validation and review marks, stop points, the
  landing rule.
- `fennec-lead` UI: design, plan, risk and review on the task; provenance
  badges; "partial guarantees" line; stop points on the project page.

## Done when

A task on fennec itself goes through the cycle in the UI; a task in a
sensitive zone does not reach code without validated design and tests; a
task outside can skip them and is marked "not validated".
