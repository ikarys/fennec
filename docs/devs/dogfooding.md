# Dogfooding

fennec builds fennec. What goes wrong while it does is logged here, one entry
per friction: what happened, why it matters, and its status.

## An agent erased uncommitted human work — fixed

2026-10-25. A workflow's `fix-review` agent rewrote `store.rs` and `web.rs`
from its own view of them and dropped a fix the human had made in the same
files while the run was paused. The agent only knows its own edits; the
human's tree is not its to write.

Fixed: a session that starts a workflow runs in its own git worktree
(`worktree.rs`), on the branch `fennec/<session>`, under
`~/.local/share/fennec/worktrees/`. The human's uncommitted work, untracked
files included, is copied in and committed as a snapshot; the base ref
`refs/fennec/base/<session>` marks where the agent's work starts. fennec
commits the worktree after each job, as `fennec`. The human integrates with
`git cherry-pick <base>..<branch>` (the decision bar offers the integration).
A project outside a git repository still runs in place.

Open edges:
- Ignored files (`node_modules`, `target`, `.env`) are not copied: the first
  `just check` in a worktree reinstalls and rebuilds from scratch (time, disk).
- Worktrees are removed on the human's decision only: the integration dialog's
  "Supprimer le dossier" box, or "Abandonner ce travail".
- A plain session (no workflow) still writes the human's tree.
- The session's agents, skills and instructions are read from the worktree.
  An agent can no longer edit them: `.agents/` and `AGENTS.md` are protected
  by default (see `.agents/protected` in docs/devs/api.md).

## A crash left a session stuck running — fixed

2026-10-25. The machine crashed during a turn; the log ended on
`running.changed {isRunning: true}` and the UI kept the send button disabled.
Fixed: the server closes such turns at start (`close_dangling_turns`).

Open edge: a CLI turn running in another process while the server starts is
closed too.

## The review loop runs on minor findings — fixed

2026-10-25. The default workflow sent minor findings back to the builder five
times before `accept` was reached on `exhausted`. Each pass costs a full
verify and review round on the local GPU. Wanted: a review that reports only
minor findings goes to `accept` directly, the findings listed for the human.

Fixed: the review node has a third outcome, `minor`, wired straight to the
human node; its question lists the review's findings.

## Abandoning the work left a run asking for approval — fixed

2026-10-28. A session planned an increment and paused on "Go with this plan?".
The human abandoned the worktree ("Abandonner ce travail") while a sibling
session did and integrated the same increment. Reopening the first session
still showed the pending approval: answering it would have built the plan in
the human's tree, on a main that already contained the work — redoing it.

Fixed: discarding the worktree ends a run still waiting on the human
(`run::finish_on_discard`, appended as `workflow.finished` with outcome
`stopped`, reason "the work was abandoned"). The run is no longer active, so
the approval card is no longer actionable; a `RunEndCard` shows the outcome
and the reason. To redo the work, the human starts a new session (a fresh
context on the current main).

## A session's change could not be previewed — fixed

2026-10-31. The panel's servers ran in the project's main directory, on the
port their command hard-coded: they served the human's checkout, never the
agent's worktree, so a new design or API change could only be seen after
integrating it.

Fixed: a server starts in the session's worktree, with the main checkout's
`node_modules`/`.venv` linked in (never committed) and a free port in `$PORT`;
the panel links to it. To preview fennec itself, `.agents/servers.json` can
hold a server on a fresh store, never the live one (a second server on it
would resume the night queue):

```json
[{"name": "fennec", "command": "cd web && npm run build && cd .. && cargo build && FENNEC_DB=$(mktemp -d)/fennec.db target/debug/fennec --port $PORT"}]
```

## The local inference server takes the machine down — open

2026-10-25. The local LLM brought the GPU driver down (`dxg` timeouts in the
kernel log, no OOM), and once the whole PC; it later died mid-request. Not
fennec's code, but fennec's load: long runs on one local GPU. To watch
alongside the review loop above.
