# Next steps

State on 2026-11-03, `develop` at `c28ed23`. Delete each item when done, and this file when empty.

## 1. Integration gate after a manual merge

`merge_start` (`crates/fennec/src/integration.rs`) fixed the false conflicts of a branch merged by
hand, for the merge only.

- `preview` still lists commits and files from the base ref, so the dialog also shows work already
  merged.
- If everything is already merged, the button stays active and `apply` creates an empty commit.
- Proposed test: after an integration, the base ref stays the merge's start (the squash leaves the
  fork point older). Tests need the human's approval first.

## 2. Misleading commit subject

After a conflict resolution, the integration dialog's model proposed
`fix: resolve merge conflicts with develop branch` for what was i18n work: it reads the branch's
commit subjects. See `domain/commit_subject.rs`.

## 3. Housekeeping

- Delete the remote branches whose PRs are merged: `feat/prompt-queue`, `feat/rules`, `feat/waivers`.
- Delete the local `feat/agent-crud` (nothing in it is missing from `develop`).
- `main` is behind `develop`: `just promote` when a release is wanted.

## 4. Then

The next capability is still to be chosen among the remaining C* of the Notion definition (see
"Next" in `AGENTS.md`).
