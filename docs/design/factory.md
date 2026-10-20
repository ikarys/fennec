# Factory, plugins, self-improvement

Date: 2026-10-19. Status: steps 1 (the Fleet), 2 (outbound plugins), 3 (framing ahead) and 4 (approval from a chat) built; §8 step 2 (Backlog → Ready) built 2026-10-20. Asked by the human after a
comparison with OpenFox (task board, Planner/Builder agents, plugins).

Purpose test (AGENTS.md): **fast**, **efficient**, **safe**. Each part below
says which it serves; a part that serves none is cut.

## 1. The Fleet — fast, safe

**Think in tasks, not projects.** The decision to run alone belongs to the
task; the project stays a place (the folder), only a constraint.

- Each task kept for later is either "Plus tard" (nothing starts, as today)
  or "Dans la Fleet" (it starts on its own).
- **One queue for every project**, ordered by task: the urgent task of any
  project moves up.
- The Fleet's work always goes apart (one worktree per task), whatever the
  project chose: a task ready and waiting for the human never holds the
  folder from the next one (changed 2026-10-19, after the first Fleet task
  stopped the queue until the human committed it).
- **The Fleet page** (named by the address, like Team and Stats): run/pause
  for everything (what runs finishes), the ordered queue (project as a tag,
  state: being framed, waits for its folder, next), what runs now, optionally
  how many tasks at once. Questions, offers, landed work, plugins and costs
  stay where they are ("À moi", the list, Réglages, Stats).
- No board, no columns, no project-level switch.

Part of this already exists unsaid: a launched task waits its folder's turn.
What is missing is that waiting made ordered, global, visible and
controllable, and tasks entering it without starting at once.

## 2. Plugins and hooks — fast (no need to watch), safe (you are reached)

One mechanism: the Journal already records every step; each entry is an event.

- `~/.config/fennec/plugins/<name>/` holds an executable and `plugin.json`
  (events listened to). fennec runs it with the event as JSON on stdin, with a
  timeout. A hook is a plugin listening to a step event ("before commit").
- Events: started, question, ready, failed, landed (a proof that fails is a
  failure, "not verified" included). Default: question, ready, failed — the
  moments a task needs the person.
- No new Rust dependency: the plugin does its own HTTPS. A shipped example
  calls Apprise (Teams, Discord, Telegram, Slack, ntfy...).
- Enabled in Réglages, "Plugins" tab.

## 3. Framing in the background — fast

Tasks kept for later are framed (conversation, "done when") while waiting.
They arrive ready, or with one question in "À moi".

## 4. Inbound plugins: approval from a chat — fast, safe by limits

Allowed by rule 1 (changed 2026-10-19). A long-running plugin (Telegram bot
on long polling, Discord bot) calls fennec's local API with its own token.

- Sender allow-list; anything else ignored.
- Only decisions already offered: validate a proven request, drop, answer a
  question. Never a grant, rule, setting or wider access.
- Token per plugin, revocable in Réglages; every outside decision written in
  the Journal ("validé via Telegram").

## 5. Offers from measured needs — efficient

Extends what exists (`review.rs` corrections said twice → rule, `sleep.rs`
know-how, `offer_rule`). **Code counts, the model drafts, the human decides.**
Signals come from Journals and Stats; each offer is a question in "À moi"
with the number behind it; "Jamais" is remembered.

| Measured signal | Offer |
|---|---|
| Repeated rejections in one area | a role from the catalogue |
| A stage failing often or costly on its model | another model for that role |
| Same instructions or know-how in 2+ projects | a skill |
| A repeated correction a command can check | a QA `check:` rather than a rule |
| Questions unanswered for hours | a notification plugin |
| Many turns lost on compile errors | a build check after each edit |

Runs in the background (`schedule`), like sleep.

## 6. Stages added by a role — efficient, safe

`STAGES` is fixed to frame, solve, prove. Let a role declare `stage: review`
(after solve): its own agent pass. It cannot veto on opinion (the model never
judges): it blocks through commands (`check:`, `cargo audit`, `semgrep`) or
sends findings back to the builder for another try.

## 7. The proof sends work back — fast, efficient

Added 2026-10-20, asked by the human after a Fleet task failed four times on
its proof (`uvicorn: command not found`, no Node, no `ruff`) with nothing sent
back to the builder: "si QA rencontre des soucis, il faut renvoyer au dev".

- When the proof of a ready run fails (a try exits non-zero, a role's check
  vetoes, a page does not show, nothing could be tried), its findings (each
  failing command and the end of its output) go back to the builder, which
  starts from its own work (fennec-os `spec::fix`), then the product is proven
  again. Twice at most (`MAX_FIXES`), then the task fails with the proof's
  reason.
- The builder is told that a command missing here is to be made easy to try
  (a script, a README line), not only the code fixed.
- A question while fixing goes to the human, like any question.
- Journal: "Preuve ratée : renvoyée au travail" with the findings.
- Not yet: sending a "done when" the proof shows unreachable back to framing.

## 8. Backlog → Ready: the human validates what fennec understood — safe, fast

Added 2026-10-20, asked by the human after describing their day-to-day use:
an Agent mode to discuss, then everything cut into tasks that work alone,
which only start once the human validated that fennec understood them.
Replaces "Plus tard" / "Dans la Fleet".

```
Backlog ──► Ready ──► running ──► to validate ──► done
   │                                 └──► failed (why, what fennec offers)
   └─ Split ─► N tasks back in the Backlog, already framed
```

- **Backlog**: a task comes from a discussion, a split, or is typed. While
  the model is free, fennec frames it (restates it, "done when", the skills
  it will use, its size, what it comes after). A question goes to "For you".
- **Validation**: only the human moves a task to Ready, after reading what
  fennec understood: Validate, Discuss (Agent mode on the task), Split,
  Delete. Several at once with the boxes. A task not framed yet cannot be
  validated; reworded, it leaves Ready and is framed again.
- **Ready**: one ordered queue. Run / Stop takes only Ready tasks, in order,
  never one from the Backlog. "Start" on a task still starts it at once.
- **Linked tasks** (`after`): a task that comes after another starts once
  that one is committed on the project's `fleet` branch, from that branch:
  it sees the work before it, so no merge conflict later. Independent
  tasks of one project run side by side, within "tasks at once". A failed
  one keeps its followers in Ready, "waits for X".
- **Disk**: worlds share the git store; build folders are shared per
  project in `.fennec/` (`CARGO_TARGET_DIR`, pnpm and uv stores), and a
  world is removed once its task is committed or dropped.
- **Dashboards** (piloting, not watching): the global one opens on the last
  five things since the last visit, each with a button to its page, then
  what runs (task, step, for how long), resources (tokens, model time, RAM,
  disk of the worlds) and costs. A project's: tasks per section, open
  discussions, what runs, time and tokens, quality (proofs passed first
  time, sent back, failed), disk, what is ready to run next, its settings.

Plan, in order, each step landing alone:

1. Docs (this section, "task" in AGENTS.md).
2. Backlog and Ready sections, validation, Run takes only Ready.
3. Split a Backlog task; `after` links honoured by the queue.
4. Skills chosen per task at framing, shown in the validation.
5. Global dashboard, then project dashboard (RAM and disk are new figures).
6. Shared build caches and world clean-up.

## Not built

- **Kanban board**: the single view covers it; dragging cards is managing,
  not leading.
- **LSP**: the agent already runs `cargo check` / `tsc` in its sandbox and the
  proof runs the real build. Built only if offer 5 measures turns lost on
  compile errors.
- **"Agent" as a word**: roles and the team already are Planner/Builder/Verifier
  (frame/solve/prove).

## Plan

Each step lands alone, tested, clippy clean, UI screenshotted (light and
dark) when it shows. The status choke point is `Store::update`
(`state.rs`): every status change goes through it, so events start there.

**0. Docs** — commit rule 1, the purpose line, this file.

**1. The Fleet** (fast, safe)
- Built 2026-10-19 in `src/fleet.rs`. `fleet.json` in the store holds the
  queue (task ids, one order across projects), run/pause and how many at
  once (default 1). A task launched, forgotten or archived leaves the queue.
- Every 3 s, when running and under the number at once, the first queued
  task starts (`work::talk`), apart. Stopping stops launching only.
- "Après une réussite": wait for the human (default), or commit proven work
  on the project's `fleet` branch, never the human's; the next Fleet task
  starts from that branch, so it builds on what landed before, and the human
  merges `fleet` when they choose. A commit on `fleet` that fails (conflict,
  checks after the replay) counts as a failure for "Après un échec".
- "Passer" on work in the folder parks it on its branch and gives the folder
  back; "Reprendre" brings it back (fennec-os `park` / `unpark`).
- A failure of a task the Fleet started stops the Fleet by default ("Après
  un échec": arrêter / continuer), so a model server down never fails the
  whole queue; the page says which task stopped it.
- From the list: a Fleet line (state, queued count, start/stop) and boxes to
  tick in "Plus tard" to send several tasks to the Fleet, or take them out.
- The Fleet page: start/stop, figures (queued, working, waiting for you,
  committed, failed, model time and tokens of what it started), the queue,
  what works, what it started lately, and its settings.
- UI: "Dans la Fleet" / "Plus tard" on a task; `/fleet` page: run/pause, the
  ordered queue (move up/down, take out), what runs now.
- Done when: three Fleet tasks in two projects land in rank order with no
  click, one per folder at a time; pause stops the next launch, not the
  running one (tests with a scripted `Model`).

**2. Plugins and hooks, outbound** (fast, safe)
- Built 2026-10-19. Enabled plugins are listed in `~/.config/fennec/plugins.json`
  (off until the person turns one on). Each event carries `message` (French,
  ready to send), `title`, `project`, `url` (the task's page). A Réglages
  "Plugins" tab lists them, turns them on, sends a test message.
- `fennec-os/src/plugin.rs`: read `~/.config/fennec/plugins/*/plugin.json`
  (`run`, `events`), run the executable with the event JSON on stdin, 10 s
  timeout, outside the request's thread; a failure goes to the Journal, never
  blocks work. Execution logic, so fennec-os (lead rule 4). Plugins run on
  the host with the person's rights, so they are read from
  `~/.config/fennec/plugins/` only, never from a project or a world (os rule 2).
- `state.rs`: `Store::update` compares status before/after and emits
  `started`, `question`, `ready`, `failed`, `landed`.
- Hooks: same plugin, step events (`before_land`); a hook's non-zero exit
  stops the landing with "not verified: hook X".
- `examples/plugins/apprise/`: shell script calling `apprise`.
- UI: Réglages, "Plugins" tab: list, on/off, the events each listens to
  (chosen in its `plugin.json`), a test message.
- Done when: a test plugin writing stdin to a file receives the events of
  one request's life; a failing `before_land` hook stops the landing.
- Not yet: the hook gets the project folder and branch, not the world's own
  folder when work goes "À part".

**3. Framing in the background** (fast)
- Built 2026-10-19. When no task works (the model is free), the Fleet frames
  its first queued task not framed yet (`work::prepare`, one `discuss` turn),
  even while stopped; the status stays `later`. A brief is kept (`ahead`) and
  the launch starts from it; a question is kept in the thread (`asked`): the
  task goes to "À moi", the Fleet passes it by ("attend ta réponse"), the
  answer leaves it for later and framed again; "Assez clair" keeps the
  conversation as the brief. Rewording drops what was framed. Only Fleet
  tasks: "Plus tard" alone starts nothing, model included.
- UI: a framed backlog task says so with its "done when"; a question goes to "À moi".
- Done when: a task kept for later gets its "done when" without being
  launched, and launching it skips the conversation.

**4. Inbound plugins: approval from a chat** (fast, safe)
- Built 2026-10-19. `plugin.json` names a `serve` program and its senders
  (`allow`); fennec-os `plugin::start` runs it with `FENNEC_URL` and
  `FENNEC_TOKEN`. `src/relay.rs` starts it when enabled and fennec listens,
  stops it when disabled (the token dies with it: that is the revocation),
  and restarts one that exited only when the person turns it off and on.
- A call with `Authorization: Bearer <token>` reaches `/api/land`,
  `/api/drop`, `/api/answer`, `/api/say`, `/api/reply` only (POST), with a
  `from` in the plugin's `allow`; anything else is refused (403).
- Journal: "Via <plugin> — commit validé par <sender>".
- `examples/plugins/telegram/`: stdlib Python, long polling; buttons Valider /
  Abandonner, a reply answers a question. Not tried against Telegram itself.
- The local API stays open to the person's own programs (Host + JSON
  check): the token names who decided and keeps a plugin to its decisions,
  it does not fence a hostile local program.
- Done when: a token can land a proven request and is refused on
  `/api/settings`; a sender off the list is ignored.

**5. Offers from measured needs** (efficient)
- New `fennec-os/src/offer.rs`: counts from runs and Journals (rejections per
  area, stage failures and cost per model, repeated instructions across
  projects, repeated checkable corrections, unanswered questions, turns on
  compile errors) → one offer per signal over a threshold; the model only
  drafts the role, skill, check or plugin text.
- Runs in the background like `sleep`; offers are questions in "À moi" with
  their number; "Jamais" stored in `~/.config/fennec/offers.json`.
- Done when: five scripted UI rejections produce the "Designer front" offer;
  "Jamais" silences it.

**6. Stages added by a role** (efficient, safe)
- `team.rs`: `stage: review` allowed, run between solve and prove with its
  own agent pass; its `check:` lines veto, its findings send the work back once.
- Done when: a review role with a failing `check:` sends a ready run back to
  solve.
