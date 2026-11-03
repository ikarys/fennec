# Next steps

State on 2026-11-03, `develop` at `fec1d2c`. Delete each item when done, and this file when empty.

## 1. Finish the i18n migration

English is the default, French the second language, but part of the UI stays in French with
English selected. The last run stopped before its criteria ever ran, and it was integrated with
criterion C1 failing.

- 265 lines with accented French remain in 48 files outside `web/src/i18n/`, 261 of them not
  plain comments. Most in `components/project` (81) and `components/workflow` (61). Largest files:
  `MapSection`, `RulesSection`, `RunEndCard`, `RiskSection`, `DiskSection`.
- This count only sees accents: French without any accent is also left to find.
- Measure, from `web/src`:

  ```bash
  grep -rnE '[éèêàâçùûœÉÈÀÇŒ]' --include='*.ts' --include='*.tsx' --exclude='*.test.ts' \
    --exclude='*.test.tsx' --exclude-dir=i18n . | grep -v "label: 'Français'"
  ```

- Done when that prints nothing, and `npx tsc --noEmit` and `npx vitest run i18n/i18n.test.ts` pass.
- Split it by folder: both runs on this hit `stopped after 200 rounds`.
- `AGENTS.md` does not mention i18n yet: add it to the done list and to the Layout section.

## 2. Integration gate after a manual merge

`merge_start` (`crates/fennec/src/integration.rs`) fixed the false conflicts of a branch merged by
hand, for the merge only.

- `preview` still lists commits and files from the base ref, so the dialog also shows work already
  merged.
- If everything is already merged, the button stays active and `apply` creates an empty commit.
- Proposed test: after an integration, the base ref stays the merge's start (the squash leaves the
  fork point older). Tests need the human's approval first.

## 3. Misleading commit subject

After a conflict resolution, the integration dialog's model proposed
`fix: resolve merge conflicts with develop branch` for what was i18n work: it reads the branch's
commit subjects. See `domain/commit_subject.rs`.

## 4. Housekeeping

- Delete the remote branches whose PRs are merged: `feat/prompt-queue`, `feat/rules`, `feat/waivers`.
- Delete the local `feat/agent-crud` (nothing in it is missing from `develop`).
- `main` is behind `develop`: `just promote` when a release is wanted.

## 5. Then

The next capability is still to be chosen among the remaining C* of the Notion definition (see
"Next" in `AGENTS.md`).
