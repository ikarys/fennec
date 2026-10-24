# Step 2 — analysis and adoption of a project

Capabilities C14, C2 (draft), C12/C13 minimal of fennec's v1 definition.
Step 1 read a hand-written `ARCHITECTURE.md`; from this step fennec proposes
the project's knowledge (vision, zones, decisions) and keeps it up to date,
and the dev decides. A model only ever proposes; the kernel checks and
writes what the dev accepted.

## The project's knowledge

Committed in the project, readable without fennec, read at `HEAD`:

- `ARCHITECTURE.md` (root): the architecture vision.
- `.fennec-os/zones.json`: the sensitive zones (format of step 1).
- `docs/adr/NNNN-title.md`: architecture decision records, Nygard's short
  form:

  ```markdown
  # 7. Store sessions in SQLite

  Date: 2026-10-24
  Status: accepted

  ## Context
  ## Decision
  ## Consequences
  ```

  A decision that replaces another says `Status: supersedes 3`; the old one
  is kept and marked `Status: superseded by 7`.

All three are the always-on `architecture` zone (`docs/adr/**` joins it), so
a task's agent can never write them: protected for a non-sensitive task's
solve, sensitive otherwise.

## Proposals: the one way a model changes the knowledge

A proposal is a file's full new text, with why, drafted by a model. The UI
shows it as a diff against `HEAD`, tagged "model". The dev edits the text,
accepts it or rejects it.

On accept the kernel checks before writing, each check tagged "verified":

- `zones.json` parses; each zone's paths match at least one tracked file
  (a zone matching nothing is shown, not refused: it may be for files to come).
- An ADR's number is the next free one, given by the kernel, never the model;
  a superseded ADR gets its status line updated.
- Only knowledge files: a proposal for any other path is refused.

Accepted proposals are committed by fennec, alone, on the dev's current
branch, with the message shown before the click. Refused while a world holds
the project folder (same rule as a task's landing).

## Analysis tasks (C14)

A task of kind `analysis`: its conversation is a chat in the project, as a
change task's is, always read-only. It ends on a `report` tool instead of
`brief`:

```json
{"summary": "...",
 "findings": [{"title": "...", "detail": "...", "files": ["..."], "severity": "high|medium|low"}],
 "suggestions": [{"title": "...", "why": "...", "kind": "task|knowledge", "task": "..."}],
 "proposals": [{"file": "ARCHITECTURE.md", "content": "...", "why": "..."}]}
```

- The report stays on the task's page, tagged "model". Nothing is blocked:
  nothing is changed.
- A `task` suggestion becomes a Backlog task in one click, with its text.
- A `knowledge` suggestion carries proposals (vision change, new ADR): the
  proposal flow above.
- The dev can keep talking: a new turn may produce a new report.
- An analysis task closes without a commit, unless the dev accepted proposals.

Three ways to start one, all ending on a report:

| Kind | Started from | Asks the model |
|------|-------------|----------------|
| Question | "New task", kind analysis | The dev's question: explain a module, estimate, anything |
| Architecture audit | the project page | Extract the produced architecture, set it against the vision and ADRs: gaps, drift, risky zones, debt; suggestions argued and ranked |
| Adoption | the project page, offered when a project has no vision | Extract the produced architecture and propose the first vision, zones and ADRs |

### Adoption

Offered on the project page (and when a project is added) while
`ARCHITECTURE.md` is missing at `HEAD`. The model reads the project with its
tools: tree, manifests, README, `AGENTS.md` / `CLAUDE.md` / `.cursorrules` as
input to understand it (turning them into rules is step 4). Its report
proposes:

- `ARCHITECTURE.md`: shape, modules and their boundaries, dependencies'
  direction, conventions, where things go;
- `.fennec-os/zones.json`: the sensitive zones it sees (auth, crypto,
  payments, migrations, lockfiles, the vision's own files are implicit);
- up to five ADRs for decisions already visible in the code (a framework, a
  store, a layout), status `accepted`, dated the day of adoption.

The dev corrects and accepts them; one commit ("docs: adopt the project in
fennec"). From then on the step 1 cycle reads them.

## Capitalisation at the end of the cycle

After the review of a change task (before the proof), one model ask without
tools: given the vision, the ADR titles, the task's design and its diff,
propose an update of `ARCHITECTURE.md` and/or a new ADR, or nothing (the
usual case). Its proposals show on the ready card next to the AI review.

Accepted ones go into the task's own commit: the kernel writes them into
the world after the work, on the dev's decision, so the knowledge lands with
the code it describes. An agent still cannot write them. Rejected ones are
dropped, the rejection kept in the Journal.

## What the models read

- The design and the review read the vision and the ADR index (number,
  title, status); an ADR's full text on demand through their tools.
- Analyses read everything through their tools, read-only.

## Where it lives

- `fennec-os` `kernel/knowledge.rs` (new): the knowledge files at `HEAD`, the
  ADR index and numbering, a proposal's checks, writing accepted proposals
  (a commit of their own, or into a world before landing).
- `fennec-os` `kernel/zones.rs`: `docs/adr/**` in the vision's zone.
- `fennec-os` `engine/analysis.rs` (new): the analysis chat and its `report`
  tool, the three prompts, the capitalisation ask.
- `fennec-os` `engine/spec.rs`: the ADR index in the design's context.
- `fennec-lead` `work.rs` / `state.rs`: the `analysis` kind, its report and
  card, suggestions into tasks, proposals accepted or rejected,
  capitalisation on the ready card, written at landing.
- `fennec-lead` UI: "Adopt" and "Audit the architecture" on the project
  page; the report with its findings, suggestions and proposals (diff,
  editable); the proposals on the ready card.

## Build order

1. The knowledge kernel: ADRs in the zone, index, checks, commit.
2. Analysis tasks: the report, suggestions into tasks.
3. Adoption.
4. Architecture audit.
5. Capitalisation at the end of the cycle.
6. Real runs: adopt hominfer, then a task there through the cycle; audit
   fennec, one suggestion into a task.

## Not in this step

Diagrams, drift tracked over time, the knowledge map (C11), security
scanners (step 5), executable rules from `.cursorrules` (step 4).

## Done when

fennec adopts a second real project (hominfer) without manual preparation:
vision, zones and ADRs proposed, corrected and accepted in the UI, committed
by fennec, then read by a task's design there. An audit produces actionable
suggestions, one of which becomes a task.
