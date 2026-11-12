---
date: 2026-11-10
commit: 6a32d7e9
kind: architecture
tool: "Claude Code"
model: "claude-opus-5-5 (consolidation), claude-sonnet-5-5 (vibecoding-architecture and frontend-architecture agents)"
read_by: bikarys
scope: whole repository (crates/ and web/ at develop 6a32d7e9)
verdict: usable with listed gaps
---

# Architecture audit

First architecture audit. The layering declared in AGENTS.md holds in the code: production
code in `domain/` does no I/O (no `std::fs`, `rusqlite`, `reqwest`, `tokio`, `std::process`; only
the clock and id leaves, Finding 11), `agent/` imports only `domain` and `agent`, and each of the
five traits in `agent/ports.rs` has a real adapter and a test double. No backend file is a god
file once its inline test module is set aside (production parts are about 470 to 590 lines; all
under `file_max` 2900). What matters most: two architecture statements in AGENTS.md no longer
describe the code (the client-side fold, the "no file loose at a source root" rule), the
architecture rules are only checked by hand although `.agents/architecture` already holds them in
checkable form, and the web side repeats card and button markup that belongs in
`components/ui`. Verdict `usable with listed gaps`: (1) the dependency direction and domain purity
are real and tested by 1647 Rust and 309 UI tests, (2) the gaps are doc accuracy, enforcement and
local duplication, none needs a redesign, (3) the migration scheme will cost on every schema change.

The security report is [2026-11-10-security.md](2026-11-10-security.md), verdict **fail**: one
DOC-WRONG, a sandboxed command with network on reaches fennec's own API as a local caller and can
turn the sandbox off, which the docs understate and AGENTS.md rules 1 and 2 contradict.

## Scores (0-5)
| Axis | Score | Evidence |
|---|---|---|
| Architecture (layers, SOLID, clean arch) | 2 | Layers hold; Finding 1 (DOC-WRONG), 3, 11, 12, 18, 21, 22 |
| Modularity (one responsibility, no god file) | 2 | No god file; Finding 2 (DOC-WRONG), 13, 14 |
| Duplication / reuse | 3 | jscpd: 3.06 % (max 4 %); Findings 7, 8, 9, 15, 16, 17 |
| Simplicity (KISS, no speculative layer) | 5 | No single-use trait, layer or wrapper found; every port has a double |
| Frontend components | 3 | Findings 5, 6, 19 |
| Styling system (shared classes, tokens) | 3 | Tailwind + one 315-line `app.css`; Finding 10 |
| Security | 2 | verdict `fail` of 2026-11-10-security.md (one DOC-WRONG, three PAINFUL) |
| Tests and gate | 3 | `just check` green in 34 s, line coverage 83.3 % (min 80 %); Findings 4, 20 |

## Metrics
- Largest files (lines, tests included): `web/src/i18n/messages.ts` 3124 (translation table, over
  `file_max` 2900), `agent/workflow/tests.rs` 2146 (test-only), `agent/turn.rs` 1975 (~530
  production), `domain/workflow/run.rs` 1540 (~590), `agent/tool_calls.rs` 1305,
  `domain/workflow/validate.rs` 1244, `store.rs` 1225, `sandbox.rs` 1196 (~470), `integration.rs`
  1180 (~585), `web/src/pages/SessionPage.tsx` 1075, `web/src/App.tsx` 689. Full list:
  `/tmp/code-audit-fennec/sizes.txt` at audit time.
- Duplication: `audit/evidence/jscpd-report.json` (2026-11-10), 3.06 %, 265 clones. The two
  largest (Icons.tsx, 276 and 241 lines) are the 65 icon functions sharing one `Svg` frame, not
  real duplication. Most large backend clones sit inside `#[cfg(test)]` modules.
- Gate: `just check` passed (fmt, clippy `-D warnings`, 1647 Rust tests, UI lint, 309 UI tests, UI
  build, licences). Coverage: `audit/evidence/coverage.txt`, 83.31 % lines.
- Catch-all names (`utils`, `helpers`, `misc`, `common`): none.

## Findings

### Finding 1: Settle the client-side fold against "the server folds and computes"
- Severity: DOC-WRONG
- Axis: Architecture (layers, SOLID, clean arch)
- Requirement: A1; AGENTS.md "The UI holds no logic"
- Where: web/src/lib/fold.ts:1 (643 lines, with lib/workflowRun.ts 394 and lib/plan.ts 412); AGENTS.md:33; docs/api.md:660
- Problem: AGENTS.md and docs/api.md say the server folds and the UI computes none, yet `fold.ts` folds the raw event stream into the thread view client-side (1.4k lines with its helpers). The exception is real and undocumented.
- Fix: Either state in AGENTS.md and docs/ui.md that the event-to-thread fold is the one deliberate client exception (and keep derived numbers server-side), or move the fold to the server.

### Finding 2: Reword "no file loose at a source root" to the rule actually followed
- Severity: DOC-WRONG
- Axis: Modularity (one responsibility, no god file)
- Requirement: A1; AGENTS.md:32
- Where: AGENTS.md:32; crates/fennec/src/ (40 root `.rs` files)
- Problem: Besides the adapter pairs and composition roots, root files such as `chat.rs`, `locks.rs`, `notifier.rs`, `metering.rs`, `update.rs`, `protected.rs`, `scanners.rs`, `project_files.rs` have no domain peer. The rule as written is broken; the convention in use is unwritten.
- Fix: Reword to "a root file is an adapter or a composition root", or group the adapters under `adapters/`.

### Finding 3: Replace the column-probe migrations with a schema version
- Severity: PAINFUL
- Axis: Architecture (layers, SOLID, clean arch)
- Requirement: A6 (persistence, migrations)
- Where: crates/fennec/src/store.rs:120 (`init`), :141 (`has_column`)
- Problem: Schema changes are `ALTER TABLE` guarded by `PRAGMA table_info`; no `user_version`, no order, no record of which step a database reached. `store/ideas.rs` and `store/plugins.rs` carry their own `SCHEMA`. Every new column adds a probe.
- Fix: `PRAGMA user_version` with an ordered list of numbered steps, and a short ADR.

### Finding 4: Run the architecture rules in `just check`
- Severity: PAINFUL
- Axis: Tests and gate
- Requirement: A1 (architecture claims verifiable)
- Where: VERIFICATION.md:19; .agents/architecture; crates/fennec/src/domain/architecture.rs
- Problem: VERIFICATION.md says the architecture rules have "no automated test", yet `.agents/architecture` holds them in checkable form (`domain forbids std::fs`, `agent never imports store/tools`) and fennec ships the checker. Nothing runs it on fennec itself.
- Fix: One `cargo test` that applies `.agents/architecture` to the real imports; name it in VERIFICATION.md. Finding 12 must be settled first.

### Finding 5: Split SessionPage.tsx
- Severity: PAINFUL
- Axis: Frontend components
- Requirement: AGENTS.md "one responsibility per file"
- Where: web/src/pages/SessionPage.tsx (1075 lines, 59 imports, 26 `useState`/`useEffect`)
- Problem: Data loading, selection, layout and rendering in one file.
- Fix: Move effects into hooks beside `useSessionThread`, `useSessionActions`, `useSessionMcp`; move panes into `components/session/*`.

### Finding 6: Split App.tsx
- Severity: PAINFUL
- Axis: Frontend components
- Requirement: AGENTS.md "one responsibility per file"
- Where: web/src/App.tsx (689 lines; internal clones at 235/286 and 410/446)
- Problem: Composition root plus routing, tabs and shortcut handling.
- Fix: Extract tab/route handling into a hook or `components/layout/*`; merge the cloned blocks.

### Finding 7: Give the decision cards one shared shell
- Severity: PAINFUL
- Axis: Duplication / reuse
- Requirement: T8; AGENTS.md "uses the primitives of web/src/components/ui"
- Where: web/src/components/session/RuleProposalCard.tsx:57, ProposedTasksCard.tsx:79, AuditCard.tsx:90, RuleApprovalCard.tsx:51, WaiverCard.tsx:23
- Problem: The same header, provenance and action layout written five times (8 to 12 line clones).
- Fix: A `DecisionCard` in `components/ui` taking title, provenance badge and an actions slot; provenance then comes from one place.

### Finding 8: Collapse the three variants in RunActionCard
- Severity: PAINFUL
- Axis: Duplication / reuse
- Requirement: T8
- Where: web/src/components/workflow/session/RunActionCard.tsx:59, :101, :146
- Problem: The same markup three times in one file (12, 17, 19 lines).
- Fix: One parametrized sub-component or a map over variants.

### Finding 9: Extract the repeated form blocks in panels and dialogs
- Severity: PAINFUL
- Axis: Duplication / reuse
- Requirement: T8
- Where: web/src/components/workflow/EdgePanel.tsx:62 / NodePanel.tsx:102 (23 lines); components/settings/ModelPrefsSection.tsx:268/279 (24 lines); components/project/ideas/NewIdeaDialog.tsx:117/134 (30 lines)
- Problem: Field groups copied within and across files.
- Fix: A shared field-group component, or a map over a field list.

### Finding 10: Bring hand-rolled row and icon buttons into components/ui
- Severity: PAINFUL
- Axis: Styling system (shared classes, tokens)
- Requirement: AGENTS.md "uses the primitives of web/src/components/ui"
- Where: web/src/pages/HomePage.tsx:129, :159; components/session/Composer.tsx:193; components/session/ToolCard.tsx:87; 20+ other files with raw `<button className="flex w-full … hover:bg-accent-soft …">`
- Problem: Row and icon buttons are styled per file instead of from a primitive, so a style change touches every copy and the icon-plus-tooltip rule is applied by hand.
- Fix: Add a `RowButton` and an `IconButton` (aria-label and tooltip built in) to `components/ui` and use them.

### Finding 11: Make the domain's clock and id leaves explicit
- Severity: MINOR
- Axis: Architecture (layers, SOLID, clean arch)
- Requirement: AGENTS.md "The domain does no I/O"
- Where: crates/fennec/src/domain/clock.rs:3, crates/fennec/src/domain/id.rs:4
- Problem: Both read the system clock (`id.rs` also the process id) inside `domain/`.
- Fix: Pass time in, or state in AGENTS.md that time and id generation are the domain's only impure leaves.

### Finding 12: Remove the agent → tools imports from test modules
- Severity: MINOR
- Axis: Architecture (layers, SOLID, clean arch)
- Requirement: `.agents/architecture` "agent never imports tools"
- Where: crates/fennec/src/agent/turn.rs:537, agent/compaction.rs:136, agent/waiver.rs:59, agent/rule_approval.rs:56, agent/workflow/tests.rs:13
- Problem: Five test modules import `crate::tools::LocalTools`; the rule would fail on them once checked (Finding 4).
- Fix: A `Toolbox` double in `agent/test_support.rs`, or scope the rule to non-test code and say so.

### Finding 13: Split store.rs by aggregate
- Severity: MINOR
- Axis: Modularity (one responsibility, no god file)
- Requirement: AGENTS.md "one responsibility per file"
- Where: crates/fennec/src/store.rs:58-620
- Problem: Connection, migrations, projects, sessions, event reads, the `EventLog` impl and delete cascades in one file, while newer aggregates already live in `store/*.rs`.
- Fix: `store/projects.rs`, `store/sessions.rs`, `store/events.rs`; keep `store.rs` for open and init.

### Finding 14: Move the large inline test modules to sibling files
- Severity: MINOR
- Axis: Modularity (one responsibility, no god file)
- Requirement: T8 (file size measured fairly)
- Where: crates/fennec/src/agent/turn.rs:529 (~1450 test lines), domain/event.rs:686, domain/preferences.rs:377, agent/tool_calls.rs:225
- Problem: Tests are 55 to 75 % of these files, hiding the production size from the reviewer and from `file_max`.
- Fix: `#[cfg(test)] mod tests;` with `x/tests.rs` for files over 1000 lines, as `agent/workflow/tests.rs` already does.

### Finding 15: Share the Turn test harness
- Severity: MINOR
- Axis: Duplication / reuse
- Requirement: T8
- Where: crates/fennec/src/agent/rule_approval.rs:70, agent/waiver.rs:70, agent/turn.rs:593
- Problem: Each module rebuilds the same `Harness`, `scratch_dir()` and 20-field `Turn` literal; every new field is edited in each copy.
- Fix: `Turn::for_test(...)` or a `Harness` in `agent/test_support.rs`.

### Finding 16: Build web test events from shared builders
- Severity: MINOR
- Axis: Duplication / reuse
- Requirement: T8
- Where: web/src/lib/workflowRun.test.ts (10 clones), lib/integration.test.ts (30 lines), lib/fold.test.ts / lib/plan.test.ts
- Problem: Event fixtures copied; a change to an event shape is edited in many places.
- Fix: A `testEvents.ts` with builder functions.

### Finding 17: Merge the small repeated UI fragments
- Severity: MINOR
- Axis: Duplication / reuse
- Requirement: T8
- Where: web/src/components/layout/TopBar.tsx:13, :33, :53; ProjectSection.tsx:51 / ProjectTile.tsx:33; DashboardSection:47 / IndicatorsSection:39; lib/fold.ts:223/238, :327/342
- Problem: 8 to 16 line clones (headers, empty states, local helpers).
- Fix: Map over item arrays, use `ui/Section` and `ui/Stat`; local helpers in `lib`. Skip anything under ~10 lines.

### Finding 18: Serve the criteria percentage from the server
- Severity: MINOR
- Axis: Architecture (layers, SOLID, clean arch)
- Requirement: AGENTS.md "The UI holds no logic"
- Where: web/src/components/session/CriteriaSection.tsx:78
- Problem: `(passed / total) * 100` computed in the view, while other views receive `percent` from the API.
- Fix: Add `percent` to the criteria tally in the API.

### Finding 19: Centralise localStorage access and the raw fetches
- Severity: MINOR
- Axis: Frontend components
- Requirement: AGENTS.md "one responsibility per file"
- Where: web/src/pages/ProjectsPage.tsx:41, :112; pages/SplitPage.tsx:22; ui/Panel.tsx; ui/SideRail.tsx; lib/theme.ts; lib/route.ts; components/settings/NetworkSection.tsx:105; hooks/useFetchedText.ts:19
- Problem: The same try/catch and key building for UI preferences in six places; two `fetch()` calls bypass `lib/api/request.ts`.
- Fix: One `lib/storage.ts`; route the fetches through `request.ts` or comment why they cannot (NetworkSection probes another port).

### Finding 20: Check that docs/map.md is current
- Severity: MINOR
- Axis: Tests and gate
- Requirement: A1 (generated documents stay true)
- Where: docs/map.md (606 changed lines uncommitted at audit time)
- Problem: Nothing fails when the committed map drifts from the imports, so it can be stale for many commits.
- Fix: Regenerate in `just check` and fail on a diff.

### Finding 21: Record the design-shaping decisions as ADRs
- Severity: MINOR
- Axis: Architecture (layers, SOLID, clean arch)
- Requirement: A6 (decision records)
- Where: docs/adr/ (only 0001-targeted-log-reads.md)
- Problem: The five-port set, the split into `backlog`, `rules`, `canvas`, `token-usage`, and the frozen wire names and schema have no record of why.
- Fix: Short ADRs in the 0001 format.

### Finding 22: Type the turn errors
- Severity: MINOR
- Axis: Architecture (layers, SOLID, clean arch)
- Requirement: AGENTS.md "prefer types (enums, newtypes) to strings and flags"
- Where: crates/fennec/src/agent/turn.rs (23 `Result<_, String>` signatures)
- Problem: Errors travel as strings, so callers cannot tell failure kinds apart without parsing.
- Fix: A `TurnError` enum.

Not retained after verification: `crates/fennec/usage.db` is not tracked (only ignored); the
Icons.tsx clones are the shared `Svg` frame, not duplication; an ADR for rusqlite is not needed,
AGENTS.md rule 6 records it. Unchecked: an `unwrap`/`expect` audit (1670 occurrences, mostly tests).

## Since last audit
First audit.

## Decisions
| # | Decision | Commit or reason |
|---|---|---|
| 1 | Fixed: AGENTS.md names the live-stream fold as the one exception | #338 |
| 2 | Fixed: the rule now says a root file is an adapter or a composition root | #338 |
| 3 | Fixed: schema changes numbered with `user_version` | #343 |
| 4 | Fixed: a cargo test checks `.agents/architecture` over fennec's production sources, so `just check` runs it | #339 |
| 5 | Fixed: SessionPage split into hooks and session components | #350 |
| 6 | Fixed: App.tsx split into routing, tab, split and shortcut hooks | #349 |
| 7 | Dropped: stale, the decision bar had already replaced those cards with DecisionRow | #345 (commit body) |
| 8 | Dropped: stale, same reason as 7 | #345 (commit body) |
| 9 | Fixed: PanelFooter shared, price fields mapped | #345 |
| 10 | Fixed: `listRowClass` in ListCard; ProjectMap's eye button kept, the shared size would enlarge every graph node | #354 |
| 11 | Fixed: ADR on the ports and the domain's impure leaves | #346 |
| 12 | Fixed: the architecture check reads production code only, test wiring allowed | #351 |
| 13 | Fixed: store.rs split by aggregate | #347 |
| 14 | Fixed: large test modules moved to sibling files | #348 |
| 15 | Fixed: one shared Turn test harness | #348 |
| 16 | Fixed: shared contract fixtures and file builders in the web tests | #353 |
| 17 | Won't fix: UI duplication at 0.35 %, remaining pairs 11-16 lines; merging costs more than it saves | |
| 18 | Fixed: the criteria tally carries `percent` from the server | #351 |
| 19 | Fixed: `lib/storage.ts`; the two raw fetches commented with why | #352 |
| 20 | Fixed: a cargo test fails when `docs/map.md` differs from a fresh render | #355 |
| 21 | Fixed: ADRs for the ports, the crates and the frozen wire | #346 |
| 22 | Won't fix: no caller tells turn errors apart, they are only logged (`web/turns.rs:158`); an enum would be speculative | |
