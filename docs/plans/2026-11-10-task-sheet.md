# Task sheet Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** An idea can be turned into a task whose sheet (objective, context, constraints, out of scope, deliverable, approach, done when) is edited by hand in a drawer, helped by a model, and rendered to the existing plan markdown when marked ready.

**Architecture:** The sheet, its event, fold, provenance, completeness and markdown live in `crates/backlog` (no I/O). The server adds sheet routes (save, ready, suggest, apply), a pure suggest prompt and a plan → sheet mapping in `crates/fennec/src/domain`, and a "sheet changed" notice before a framing turn. The UI adds a `Drawer` primitive and a task drawer; it renders what the server folds.

**Tech Stack:** Rust (serde, rusqlite store, llm-providers-connector `OpenAiClient`), React + TypeScript + Tailwind, Vitest.

**Spec:** `docs/adr/0006-task-sheet.md`

## Global Constraints

- Wire is frozen and only grows (ADR 0005): one new event `idea.sheet_edited`; new fields `#[serde(default)]`; no existing name changes.
- Required to mark ready: non-empty trimmed `objective` and at least one `done_when` criterion with non-empty text. Nothing else blocks.
- Completeness counts: required n/2 (objective, done when), suggested n/4 (context, constraints, out of scope, deliverable); approach is not counted.
- A suggestion never proposes a `check`; the agent never marks a task ready.
- The prompt only grows (rule 7): the sheet reaches a running session as an appended system-generated note, never by editing history.
- UI holds no logic: completeness, provenance marks, missing fields and the sheet markdown come from the server.
- UI: primitives from `web/src/components/ui`, icons + tooltip, one primary text button (**Ready**), never dull (`docs/ui.md`). i18n strings in en and fr.
- Every PR: `just check` and `just secrets` green; `docs/map.md` regenerated when modules change; `docs/api.md` updated with routes; commit trailers `Assisted-by: Claude Code:claude-opus-5-5` (+ `Assisted-by: local:qwen3.8-27b` when qwen wrote code).
- Hostile input: sheet text fields capped at `MAX_IDEA_CHARS` each, at most 20 criteria, check commands single line ≤ 500 chars (vulnerability class: resource exhaustion / log bloat); suggest output is untrusted text shown as a draft only.

## Review Focus

1. Editing the sheet of a **Ready** task: the queued plan must not silently go stale → a sheet save on a Ready idea returns it to `Framing` (if it has a session) or `Inbox`; the human presses Ready again. Test in Task 2.
2. Saving the sheet of a **running / dropped / completed** idea → 409, nothing appended. Test in Task 3.
3. A human save that re-sends an agent-written field **unchanged** must keep its model mark (the whole sheet is sent at each blur). Test in Task 2.
4. A model reply for suggest that is empty, is wrapped in code fences, or is not parseable as criteria → 502 with a message, nothing saved. Test in Task 4.
5. Apply to the sheet when no plan is pending, or the idea has no session → 409. Test in Task 6.

---

## PR 1: backlog domain (`crates/backlog`)

### Task 1: `TaskSheet`, completeness, markdown

**Files:**
- Create: `crates/backlog/src/sheet.rs`
- Modify: `crates/backlog/src/lib.rs` (module + re-exports)

**Interfaces:**
- Produces:
  - `pub struct TaskSheet { objective, context, constraints, out_of_scope, deliverable, approach: String, done_when: Vec<SheetCriterion> }`: `Serialize, Deserialize, Clone, PartialEq, Eq, Default, Debug`, camelCase, every field `#[serde(default)]`.
  - `pub struct SheetCriterion { text: String, #[serde(default, skip_serializing_if = "Option::is_none")] check: Option<String> }`
  - `pub enum SheetField { Objective, Context, Constraints, OutOfScope, Deliverable, Approach, DoneWhen }`: camelCase serde, `Copy, Eq, Hash, Ord`, with `SheetField::ALL: [SheetField; 7]`.
  - `pub enum Author { Human, Agent }`: lowercase serde.
  - `impl TaskSheet { pub fn missing(&self) -> Vec<SheetField>; pub fn completeness(&self) -> Completeness; pub fn field_eq(&self, other: &TaskSheet, field: SheetField) -> bool; pub fn to_markdown(&self, title: &str) -> String }`
  - `pub struct Completeness { required: u8, suggested: u8 }`: serde camelCase.

- [ ] **Step 1: Write the failing tests** in `sheet.rs`:
  - `missing_names_objective_and_done_when`: `TaskSheet::default().missing() == [Objective, DoneWhen]`; objective `"  "` still missing; a criterion with text `""` does not count.
  - `completeness_counts_required_and_suggested`: objective + 1 criterion + context → `{required: 2, suggested: 1}`; approach set alone → `{0, 0}`.
  - `markdown_has_the_plan_shape`: sheet with every field → output starts with `"## Plan: Title\n\nGoal: <objective>"`, contains `"## Context\n"`, `"## Constraints\n"`, `"## Out of scope\n"`, `"## Deliverable\n"`, `"## Approach\n"` in that order, then `"## Done when\n- [ ] Builds — check: `cargo build`\n- [ ] Reads well"`. Empty fields render no section.
- [ ] **Step 2:** `cargo test -p backlog sheet` → FAIL (module missing).
- [ ] **Step 3:** Implement. A multi-line field is emitted as is under its heading. Criterion lines use the exact form of `Plan::to_markdown` (`crates/fennec/src/domain/plan.rs:64-69`) so `parse_done_when` and `plan_scope` read them. The cross-crate read-back is tested in Task 3.
- [ ] **Step 4:** `cargo test -p backlog` → PASS.
- [ ] **Step 5:** Commit `feat(backlog): add the task sheet`.

### Task 2: `idea.sheet_edited` event, fold, provenance

**Files:**
- Modify: `crates/backlog/src/event.rs`: variant, `kind()`, `idea_id()`, `kind_returns_wire_names` test.
- Modify: `crates/backlog/src/fold.rs`: `Idea` fields, fold arm, tests.

**Interfaces:**
- Consumes: Task 1 types.
- Produces:
  - `BacklogEvent::SheetEdited { idea_id: String, sheet: TaskSheet, by: Author }`, wire `idea.sheet_edited`.
  - `Idea.sheet: Option<TaskSheet>`.
  - `Idea.model_fields: BTreeSet<SheetField>` (fields whose last change came from an agent save).

- [ ] **Step 1: Write the failing tests** in `fold.rs`:
  - `a_sheet_save_sets_the_sheet`: Added + SheetEdited(human) → `idea.sheet == Some(sheet)`, `model_fields` empty, `updated_at` = event time.
  - `an_agent_save_marks_the_fields_it_changed`: agent save setting objective + context → marks `{Objective, Context}`.
  - `a_human_change_clears_the_mark_and_an_unchanged_resend_keeps_it`: then a human save with the same context and a new objective → marks `{Context}` (Review Focus 3).
  - `a_sheet_save_on_a_ready_idea_unreadies_it`: Added, Framing(s1), Ready, SheetEdited → state `Framing`; without Framing → `Inbox` (Review Focus 1).
  - `sheet_edited_round_trips_on_the_wire`: JSON `{"type":"idea.sheet_edited","data":{"ideaId":"i1","sheet":{"objective":"o"},"by":"agent"}}` deserializes; missing sheet fields default.
- [ ] **Step 2:** `cargo test -p backlog` → FAIL.
- [ ] **Step 3:** Implement the fold arm. For each `SheetField::ALL` where `!previous.field_eq(&new, f)`, insert when `by == Agent` and remove otherwise. The previous sheet defaults to empty. Then apply the Ready → Framing/Inbox rule. Dropped/Completed ideas already skip events.
- [ ] **Step 4:** `cargo test -p backlog` → PASS; `just check` → green.
- [ ] **Step 5:** Commit `feat(backlog): fold the task sheet and its provenance`. Open PR 1 → develop, squash-merge on green, pull develop.

---

## PR 2: server routes

### Task 3: save, ready, board view

**Files:**
- Modify: `crates/fennec/src/web/routes/idea_routes.rs`: action arms `"sheet"`, `"turn-into-task"`; `"ready"` accepts no `plan` when the idea has a sheet.
- Create: `crates/fennec/src/web/routes/sheet_input.rs`: parse + cap a `TaskSheet` from request JSON (keeps `idea_routes.rs` one responsibility).
- Modify: `crates/fennec/src/web/views/idea_board.rs`: view fields.
- Modify: `docs/api.md`.

**Interfaces:**
- Consumes: `backlog::{TaskSheet, Author, SheetField}`, `BacklogEvent::SheetEdited`.
- Produces (HTTP, all under `POST /api/projects/{id}/ideas/{ideaId}/…`):
  - `sheet {sheet}`: append `SheetEdited{by: Human}`, then board. 409 for `Started | Dropped | Completed`, 400 for over-cap input, 404 for an unknown idea.
  - `turn-into-task {}`: for an idea without a sheet, append `SheetEdited{sheet: {objective: idea.text}, by: Human}`. 409 if it has one.
  - `ready {}` (idea with a sheet): 400 `{error, missing: [SheetField]}` when `missing()` is non-empty. Otherwise append `Ready{plan: sheet.to_markdown(&backlog::title(&idea.text, None))}`. Ideas without a sheet keep today's `ready {plan}`.
  - `pub fn parse_sheet(value: &Value) -> Result<TaskSheet, String>` in `sheet_input.rs`.
- Idea view gains `"sheet"`, `"sheetMarkdown"` (rendered, or null), `"missing"`, `"completeness"`, `"modelFields"`.

- [ ] **Step 1: Write the failing tests:**
  - In `sheet_input.rs`, `parse_sheet_rejects_oversize_and_multiline_checks`: a field of `MAX_IDEA_CHARS + 1`, 21 criteria, and a check with `\n` each fail.
  - In the route tests (follow the existing tests of `idea_routes.rs`):
    - `ready_without_objective_is_400_with_missing`
    - `ready_with_a_full_sheet_emits_a_plan_read_back_by_the_readers`: `criteria::parse_done_when(plan)` returns the sheet's criteria with checks, and `backlog::title::plan_title(plan)` returns the title.
    - `sheet_on_a_started_idea_is_409` (Review Focus 2)
    - `sheet_on_an_unknown_idea_is_404`
- [ ] **Step 2:** `cargo test -p fennec idea_routes sheet_input` → FAIL.
- [ ] **Step 3:** Implement. Add the view fields in `idea_board.rs:319` next to `"plan"`.
- [ ] **Step 4:** Tests PASS; update `docs/api.md` (the routes, the event `idea.sheet_edited`).
- [ ] **Step 5:** Commit `feat: save and ready a task sheet from the API`.

### Task 4: suggest one field

**Files:**
- Create: `crates/fennec/src/domain/sheet_suggest.rs`: pure prompt + reply parsing.
- Create: `crates/fennec/src/web/jobs/sheet_suggest.rs`: the model call, shaped like `web/jobs/commit_subject.rs`.
- Modify: `crates/fennec/src/web/routes.rs`: `("POST", ["api","projects",id,"ideas",iid,"sheet","suggest"])` before the generic action arm (`routes.rs:145-162`).
- Modify: `docs/api.md`.

**Interfaces:**
- Produces:
  - `pub fn prompt(idea_text: &str, text_attachments: &[(String, String)], sheet: &TaskSheet, field: SheetField, agents_md: Option<&str>, lang: Lang) -> String`. AGENTS.md is truncated to 8 000 chars. Each field has a fixed help line; `DoneWhen` asks for one criterion per line, no command.
  - `pub enum Suggestion { Text(String), Criteria(Vec<String>) }`, serialized `{text}` or `{criteria}`.
  - `pub fn parse_reply(field: SheetField, reply: &str) -> Result<Suggestion, String>`: strips code fences and bullets, and errors on empty.
  - Route `sheet/suggest {field}` → 200 `Suggestion`, 400 unknown field, 404 unknown idea, 502 model error or empty reply. Nothing is appended: the UI saves an accepted draft through `sheet` with `by: agent`, so `sheet` accepts an optional `"by": "agent"`.
- [ ] **Step 1: Write the failing tests** in `domain/sheet_suggest.rs`:
  - `the_prompt_carries_the_sheet_the_field_and_its_help`
  - `agents_md_is_truncated`
  - `parse_reply_strips_fences_and_bullets`
  - `an_empty_reply_is_an_error` (Review Focus 4)
  - `criteria_never_carry_a_check`: `"- Builds — check: \`rm -rf /\`"` → `Criteria(["Builds"])`
- [ ] **Step 2:** `cargo test -p fennec sheet_suggest` → FAIL.
- [ ] **Step 3:** Implement. Read AGENTS.md from the project's workdir root only, ≤ 64 KiB, no symlink follow (class: path traversal / oversized read). Text attachments come through `web/jobs/idea_assets.rs`.
- [ ] **Step 4:** Tests PASS; `just check` green; `docs/api.md` updated.
- [ ] **Step 5:** Commit `feat: suggest a task sheet field with the model`. Open PR 2, squash-merge on green, pull develop.

---

## PR 3: framing

### Task 5: plan gains the sheet's fields

**Files:**
- Modify: `crates/fennec/src/domain/plan.rs`: `context, constraints, out_of_scope, deliverable: String`, `#[serde(default, skip_serializing_if = "String::is_empty")]`. `to_markdown` emits `## Context` … `## Deliverable` after `Goal:` when non-empty, same headings as `TaskSheet::to_markdown`.
- Modify: `crates/fennec/src/agent/propose_plan.rs`: optional schema properties and parse (multi-line allowed, each ≤ `MAX_IDEA_CHARS`).
- Modify: `crates/fennec/agents/plan.agent.md`: ask for these four fields when the discussion gave them, never invent them; when the first message is a task sheet, keep its objective and criteria unless the human changes them.

- [ ] **Step 1: Failing tests:**
  - `plan.rs`: `optional_sections_render_only_when_set`. The existing `serde_uses_camel_case_and_skips_the_absent_check` must still pass unchanged, which proves the old JSON shape.
  - `propose_plan.rs`: `parse_reads_the_optional_sections`.
- [ ] **Step 2:** FAIL → **Step 3:** implement → **Step 4:** `cargo test -p fennec plan propose_plan` PASS.
- [ ] **Step 5:** Commit `feat: let a plan carry context, constraints, out of scope and deliverable`. The tool schema change only reaches new sessions: definitions are frozen at a session's first turn.

### Task 6: apply the pending plan to the sheet

**Files:**
- Create: `crates/fennec/src/domain/plan_sheet.rs`: `pub fn sheet_from_plan(plan: &Plan, current: &TaskSheet) -> TaskSheet`.
- Modify: `idea_routes.rs`: action `"sheet-apply" {}`.
- Modify: `web/src/components/session/PlanDecision.tsx` and `web/src/hooks/useSessionPlan.ts`: in a session bound to an idea that has a sheet, the "Idée prête" button becomes **Appliquer à la fiche** (`applyToSheet`), which calls `applySheet(projectId, ideaId)`. Sessions without a sheet keep today's button.

**Interfaces:**
- Mapping: goal → objective; done_when → done_when (text, check as kept by preflight); context/constraints/out_of_scope/deliverable 1:1; steps → approach as `"1. <title>: <change>"` lines; risks appended to context under a `Risks:` line. An empty plan field keeps the current sheet's value.
- Route `sheet-apply`: reads the idea's `session_id` events, takes `plan::pending(events)`, and appends `SheetEdited{by: Agent}`. 409 when there is no session or no pending plan (Review Focus 5).
- [ ] **Step 1: Failing tests:**
  - `plan_sheet.rs`: `maps_every_plan_field` and `an_empty_plan_field_keeps_the_sheet`.
  - Route: `apply_without_a_pending_plan_is_409`.
  - Web (Vitest, `PlanDecision.test.tsx`): `shows_apply_to_sheet_for_a_task_session`.
- [ ] **Step 2–4:** FAIL → implement → PASS.
- [ ] **Step 5:** Commit `feat: apply a framing plan to the task sheet`.

### Task 7: the sheet reaches the framing session

**Files:**
- Create: `crates/fennec/src/domain/sheet_notice.rs`, modelled on `domain/workdir_notice.rs`.
- Modify: `crates/fennec/src/agent/turn.rs`: field `task_sheet: Option<&'a str>` on `Turn`, plus `ensure_sheet_notice(turn)` called right after `ensure_workdir_notice` (`turn.rs:207`) and at `turn.rs:523`.
- Modify: `crates/fennec/src/turn_build.rs`: fill `task_sheet` from `store.ideas(project)` → `idea_for_session` → `sheet.to_markdown(title)`.
- Modify: `web/src/lib/ideas.ts`: `framingPrefill` appends `idea.sheetMarkdown` when present.

**Interfaces:**
- `pub fn needed(events: &[Event], sheet_markdown: &str) -> Option<String>` returns the note when the sheet differs from the last one the model was told. It looks at the last notice in `model_view`, prefix `"<system-reminder>\nThe task sheet now reads:\n"`, else at whether the first human message contains the markdown verbatim. Otherwise it returns None.
- [ ] **Step 1: Failing tests** in `sheet_notice.rs`:
  - `no_notice_when_the_first_message_carries_the_sheet`
  - `a_changed_sheet_gets_a_notice`
  - `the_same_sheet_twice_gets_one_notice`
  - `a_compaction_resets_what_the_model_knows`, following the workdir_notice tests.
- [ ] **Step 2–4:** FAIL → implement → `cargo test -p fennec sheet_notice turn` PASS.
- [ ] **Step 5:** Commit `feat: tell the framing agent when the task sheet changed`. Open PR 3, squash-merge on green, pull develop.

---

## PR 4: the drawer

### Task 8: `Drawer` primitive

**Files:**
- Create: `web/src/components/ui/Drawer.tsx`, `Drawer.test.tsx`.

**Interfaces:**
- `export function Drawer({ open, onClose, title, actions, children }: { open: boolean; onClose: () => void; title: ReactNode; actions?: ReactNode; children: ReactNode })`.
- Same focus trap and Escape handling as `Modal.tsx`. Right side, `w-[min(720px,100%)]`, full width under 900px, scrim `bg-scrim`, tinted header.
- [ ] **Step 1:** Failing tests: `escape_closes`, `focus_starts_inside`.
- [ ] **Step 2–4:** FAIL → implement (reuse Modal's focus helpers; if they are inline in Modal, move them to `ui/focusTrap.ts` in the same change) → `npm test` PASS.
- [ ] **Step 5:** Commit `feat(web): add a drawer primitive`.

### Task 9: task drawer, Sheet tab

**Files:**
- Create: `web/src/components/project/ideas/TaskDrawer.tsx`, `SheetTab.tsx`, `SheetSection.tsx`, `CriteriaEditor.tsx`, `TaskDrawer.test.tsx`.
- Create: `web/src/lib/taskSheet.ts`: `saveSheet(projectId, ideaId, sheet, by?)`, `markSheetReady`, `suggestField(projectId, ideaId, field)`, `turnIntoTask`, `applySheet`; types `TaskSheet`, `SheetField`, `Completeness`.
- Modify: `web/src/lib/ideas.ts`: `IdeaView` gains `sheet`, `sheetMarkdown`, `missing`, `completeness`, `modelFields`.
- Modify: i18n en/fr files: section titles and help lines (Objectif, Contexte, Contraintes, Hors périmètre, Livrable, Approche, Done when), `ready`, `missingTooltip`, `suggest`, `accept`, `reject`, `modelWritten`.

**Behaviour:**
- One tinted `Card` per section; the Approche section renders only when `sheet.approach` is non-empty.
- Click to edit with `TextArea`, saved on blur through `saveSheet` (whole sheet).
- A suggest icon button per section shows the draft under the field with accept/reject icons; accept saves with `by: "agent"`.
- `modelFields` shows a "model" `Badge` (indicative provenance).
- The header shows the title, the state `Badge`, "requis n/2 · suggérés n/4" from `completeness`, and **Prête**, disabled with a tooltip listing `missing`.
- [ ] **Step 1:** Failing Vitest: `ready_is_disabled_until_objective_and_done_when_are_set` (renders with `missing: ['objective','doneWhen']` then `[]`), `approach_section_hidden_when_empty`, `accepting_a_suggestion_saves_by_agent` (mock fetch).
- [ ] **Step 2–4:** FAIL → implement → PASS.
- [ ] **Step 5:** Commit `feat(web): edit a task sheet in a drawer`.

### Task 10: framing tab, raw idea, board wiring

**Files:**
- Create: `web/src/components/project/ideas/FramingTab.tsx`, `RawIdeaPanel.tsx`.
- Modify: `TaskDrawer.tsx`: `Tabs` Fiche / Cadrage; an idea without a sheet shows `RawIdeaPanel`, with **Lancer** (existing start-unframed call) and **Transformer en tâche** (`turnIntoTask`).
- Modify: `IdeaRow.tsx` / `TasksSection.tsx`: a row click opens the drawer; the URL search param `task=<id>` holds it (read and write with the router's search params).

**Behaviour (deviation from the ADR, recorded there):** the Cadrage tab does not embed the thread: `SessionThreadPane` needs the whole session page state. It shows **Cadrer avec l'agent** when the idea has no session (existing `frameIdea` flow + prefill, which now carries the sheet), else **Ouvrir la session de cadrage** (link). The session's decision bar holds **Appliquer à la fiche**.
- [ ] **Step 1:** Failing Vitest: `a_raw_idea_offers_run_and_turn_into_task`, `the_url_opens_the_drawer`.
- [ ] **Step 2–4:** FAIL → implement → PASS.
- [ ] **Step 5:** Docs: a paragraph in `docs/components.md`, the Drawer in `docs/ui.md`, regenerate `docs/map.md`, ADR 0006 status `accepted`. `just check` + `just secrets` green. Commit `feat(web): open tasks from the board in the drawer`. Open PR 4, squash-merge on green, pull develop, remove the worktree.
