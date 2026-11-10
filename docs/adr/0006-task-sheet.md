---
status: proposed
date: "2026-11-10"
---

# An idea becomes a task through a hand-edited task sheet

## Context and problem statement

An idea is a line of text. "Frame" opens a plan session, a free discussion whose result is one
markdown string (`idea.ready {plan}`), editable only as raw text. Nothing guarantees that a task
reaching the queue has a clear objective, constraints or a definition of done, and the human
cannot fix one part of it by hand: the structure the plan agent produced (`plan.proposed`) is
lost at the idea boundary.

## Decision

An idea stays a quick note. From it the human either **runs** it (its text sent as the prompt,
as today) or **turns it into a task**: a task sheet, a set of fields edited by hand like a
ticket, that a model can help fill but never owns.

### The sheet (`crates/backlog`, no I/O)

`TaskSheet { objective, context, constraints, out_of_scope, deliverable, approach,
done_when: [{text, check?}] }`, every text field optional.

- Required to mark the task ready: a non-empty `objective` and at least one `done_when`
  criterion. Everything else is guidance: an empty field shows a help line, never blocks.
- `approach` exists only when the framing agent filled it (its plan's steps); the human may
  edit or clear it.

### Events

One new event, existing ones untouched (ADR 0005):

- `idea.sheet_edited {ideaId, sheet, by: human | agent}`: the whole sheet at each save; the fold
  keeps the last one.
- Provenance per field, deterministic: a field is marked as model-written when its last change
  came from an `agent` save; a human save that changes it clears the mark.

"Ready" validates the required fields (400 otherwise), renders the sheet as the plan's canonical
markdown (`## Plan: <title>`, `Goal: <objective>`, sections Context, Constraints, Out of scope,
Deliverable, Approach, `## Done when` with checks) and emits the existing
`idea.ready {plan}`. Every downstream reader (queue, `start_from_plan`, `parse_done_when`,
`plan_title`, the verified/indicative criteria) stays unchanged. An idea with a plan and no sheet
is shown as today; no migration.

### Model help, both indicative

- **Suggest one field**: `POST /api/projects/{id}/ideas/{ideaId}/sheet/suggest {field}` →
  `{text}` or `{criteria}`. One model call, same shape as `web/jobs/commit_subject.rs`; the prompt
  is a pure domain function over the idea's text and text attachments, the whole current sheet,
  the field and its help line, and the project's AGENTS.md (truncated). The UI shows it as a draft
  in the field with accept / reject; nothing is saved until accepted, and an accepted suggestion
  is saved `by: agent`. It does not read the code and never proposes a `check`: a check is only
  trusted once run in the sandbox.
- **Frame with the agent**: opens (or resumes) the idea's plan session (`idea.framing`), whose
  first message carries the sheet as markdown. When the human edited the sheet since the last
  turn, their next message is preceded by a system-generated note with the current sheet: the
  prompt only grows (rule 7). The agent still ends with `propose_plan`; `Plan` gains optional
  `context`, `constraints`, `outOfScope`, `deliverable` (serde default), and the plan agent's
  prompt asks for them. In a framing session the decision bar offers **Apply to the sheet**: the
  server maps the plan onto the sheet (`goal` → objective, `done_when` with their preflighted
  checks, the new fields one to one, steps → approach, risks appended to context) and saves it
  `by: agent`. The agent never marks a task ready.

### UI: a drawer

- A new `Drawer` primitive (`web/src/components/ui/`, modelled on `Modal`: focus trap, Escape,
  full width under 900px). A board row opens it; the URL keeps it (`?task=<id>`). A modal was
  rejected: too tight on small screens.
- Tab **Sheet**: one tinted `Card` per section, click to edit (`TextArea`), saved on blur;
  required markers on objective and done when; criteria edited inline (text, optional check);
  header with title, state badge, the counts "required n/2 · suggested n/4" (approach not counted) and one text button,
  **Ready**, disabled with a tooltip naming what is missing. Completeness, provenance marks and
  badges come from the server.
- Tab **Framing**: the framing session's thread with the existing session components, or a single
  "Frame with the agent" button when there is none.
- A raw idea's drawer offers **Run** and **Turn into a task**.

## Consequences

- One structured object per task, edited by hand, with the model's share visible field by field.
- The plan markdown stays the only contract downstream: no reader changes.
- Two required fields: a task can no longer reach the queue without an objective and a definition
  of done; a raw idea can still be run without a sheet.
- Tests: the sheet fold and provenance, ready validation, plan → sheet mapping, rendered markdown
  read back by `parse_done_when` and `plan_title`; routes (400 missing required, 404 unknown
  idea); the drawer (Ready disabled then enabled).
