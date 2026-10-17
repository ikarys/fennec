# Virtual team

Date: 2026-10-17. Status: first slice built (see "Plan"). The user holds to it.

## Need

A team of virtual developers, each backed by an LLM, to build new projects and
maintain existing ones. Profiles asked for: senior developer, software
architect, QA, frontend designer, DevSecOps (advanced security), DevOps
(maintenance of infrastructure), software maintainer, project manager, UX
specialist. **The team must be configurable**: roles added, removed or edited
by the user, globally and per project.

**Not one team for everyone** (user, 2026-10-17): the nine profiles above are
this user's need, not a default. Other users will want other teams. So:

- The built-in team stays minimal and neutral (one role per stage). Richer
  profiles ship as a **catalogue** of role templates, inactive until picked.
- The team is configured **in the web UI** (fennec-lead rule 1): list, add from
  the catalogue, create, edit, remove, for the user or for one project.
  Storage stays Markdown files, so a team is shared by copying a folder or
  through git.
- A project can **remove** a role, not only add or override one.
- **One team per kind of work, not per user**: the same user needs a dev
  team for software and a different one for studies (researcher, critical
  reviewer, sources specialist, writer). Teams are named folders of roles
  ("Dev", "Études"...); each project picks its team in the UI; the
  catalogue offers ready-made teams as well as single roles. The three
  stages already generalise (frame the question, produce, check), but
  fennec's execution is code-shaped (failing tests, a product to run, a
  commit): a study fits when it lives in a git repository of documents with
  checks as commands (links, references, structure). Pure reflection with
  no checkable deliverable stays out of scope (usage analysis, mode 2).
- Extension points, each added when needed: a model per role, tools and veto
  per role, new stages (review, background maintenance).

## Fit with fennec

The lead scenarios already say "I lead a team across several projects"; the
team is what was missing. fennec-os stages already behave like roles:

| Existing stage | Natural role |
|---|---|
| `spec` (request → done when, sparring) | Project manager, UX |
| `agent` + `solve` | Senior developer, frontend |
| `proof` (launch the product, captures) | QA |
| `review` + `rules` | Architect |
| `schedule` + `sleep` (background work) | Maintainer, DevOps |
| none yet | DevSecOps, as a review pass |

## Design

- **One role = one file**, e.g. `~/.config/fennec/team/qa.md`, overridable per
  project (`.fennec-os/team/qa.md`). Front matter: stage it acts at, model,
  tools it may run (`cargo audit`, `semgrep`, `npm audit`...). Body: profile
  and standards. Adding a file adds a member; deleting it removes one.
- Injected through the `context` argument `agent::run` already takes. No
  database, no framework.
- **A role is a responsibility, not a persona.** "You are a senior dev" alone
  changes little. What matters: own tools and the right to block. QA refuses
  "ready" without proof; DevSecOps runs real scanners and emits
  "not verified: X"; the architect can send work back.
- **Maintenance of existing projects**: maintainer and DevOps run in the
  background (`schedule`): dependency updates, CVEs, tests that broke. They
  never interrupt; findings become proposed requests in "À moi".
- **On screen the team stays discreet**: the card says who asks a question or
  who signed a proof. Usage analysis says fewer interactions, not nine agents
  chatting.
- Roles live in fennec-os (execution); fennec-lead only shows them. No new
  repository.

## Plan

1. **Done (2026-10-17)**: `fennec-os/src/team.rs`. Front matter `name`,
   `stage` (`frame`, `solve`, `prove`). `~/.config/fennec/team/*.md` is the
   whole user team when the directory exists; otherwise the built-in
   `fennec-os/team/` (Chef de projet, Dev senior, QA) is used, so copy it
   there to start. `.fennec-os/team/*.md` in a project adds or overrides by
   file name. Spec, run and proof records carry `team`; the card's details
   show "Équipe : …".
   Not yet: a model per role, removing a role for one project only, tools
   and veto per role (only the brief changes for now), measuring that a
   role changes outcomes.
2. Make it everyone's: catalogue of role templates (the nine profiles),
   "Équipe" screen in fennec-lead, per-project removal.
   **Storage done (2026-10-17)**: `fennec-os/catalog/` holds 14 roles (the
   nine dev profiles plus five study roles). Built-in teams `dev` (project
   manager, senior developer, QA) and `etudes` (research lead, researcher,
   writer, critical reviewer, sources specialist) are lists of catalogue
   keys. `~/.config/fennec/teams/<team>/` holds the human's teams and
   replaces a built-in one of the same name. A project picks its team and
   removes roles in `.fennec-os/team.json`; default team `dev`. With no
   background stage yet, maintainer and DevOps act at `solve`, DevSecOps at
   `prove`.
   **"Équipe" screen done (2026-10-17)**: a dialog for the selected project:
   pick the team, tick roles in or out, add a catalogue role (copied to the
   project's `.fennec-os/team/`, where it can be edited). Each change is
   saved at once. A role added to the project stays when the team changes.
   Then (same day): a role is edited or written from scratch in the dialog
   (saved in the project); "Enregistrer comme équipe…" turns the project's
   working roles into a team of the user's own; built-in teams are named
   as shown (Développement, Études); a team that cannot be found opens with
   a way to pick another. "Team" is the seventh word on screen (AGENTS.md
   rule 2).
3. Then extension points, one at a time, each proven useful before the next:
   model per role, tools and veto (DevSecOps first), background stage
   (maintainer/DevOps).
   **Tools and veto done (2026-10-17)**: a `prove` role lists commands as
   `check:` lines (edited in the dialog, one per line). The harness runs them
   on every proof, beside the model's tries: a failure blocks "ready" ("X
   bloque : `cmd` a échoué"), a tool the machine lacks (exit 127) is shown
   as not verified. They do not count as trying the product. The catalogue's
   DevSecOps runs `gitleaks detect --no-git`. First trial: after a veto
   (`test -f SECURITY.md`), the retry wrote a SECURITY.md, since the reason
   becomes the next attempt's lesson; the first role that changed an outcome.
   Not yet: tools that need the network (cargo audit, pip-audit) cannot run
   in the sandbox.

## Risks

- ⚡ The Fennec post-mortem: "too big while the core loop did not work".
  Nine roles at once would repeat it; hence three first.
- ✅ Vocabulary: "équipe" became the seventh word on screen (2026-10-17).
- ⚡ Persona prompts without tools or veto would be theatre; each role must
  change an outcome that can be measured. Checks are the first answer.
