# Audit index

Every report under `audit/` is listed here. Reports are never rewritten after
the fact; decisions are appended to them. `status.json` is written by
`vibecheck` and feeds the README badge.

`audit/evidence/` holds the tool outputs listed in `VIBECODING.md` under
`evidence`; `just evidence` writes them and CI regenerates them on `main`,
never edited by hand.

| Date | Commit | Kind | Tool / model | Read by | Verdict | Open findings |
|---|---|---|---|---|---|---|
| 2026-11-10 | 6a32d7e9 | architecture | Claude Code / claude-opus-5-5, claude-sonnet-5-5 | bikarys | usable with listed gaps | 0 |
| 2026-11-10 | 6a32d7e9 | security | Claude Code / claude-sonnet-5-5 | bikarys | fail | 0 |

Kinds: `convention`, `architecture`, `security`, `blind-<campaign>`.
File names: `<date>-<kind>.md`, for example `2026-11-08-architecture.md`.
