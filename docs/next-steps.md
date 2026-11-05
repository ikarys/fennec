# Next steps

State on 2026-11-05. Delete each item when done, and this file when empty.

## 1. Integration gate: base ref after an integration

A branch merged by hand is handled (preview, apply and the gate start from `merge_start`). Still
proposed: a test that after an integration the base ref stays the merge's start (the squash leaves
the fork point older). Tests need the human's approval first.

## 2. Housekeeping

- `main` is behind `develop`: `just promote` when a release is wanted.

## 3. Then

The next capability is still to be chosen among the remaining C* of the Notion definition (see
"Next" in `AGENTS.md`).
