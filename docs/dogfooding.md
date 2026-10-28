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
`git cherry-pick <base>..<branch>` (the context panel shows the command).
A project outside a git repository still runs in place.

Open edges:
- Ignored files (`node_modules`, `target`, `.env`) are not copied: the first
  `just check` in a worktree reinstalls and rebuilds from scratch (time, disk).
- Worktrees are removed on the human's decision only: the integration dialog's
  "Supprimer le dossier" box, or "Abandonner ce travail".
- A plain session (no workflow) still writes the human's tree.
- The session's agents, skills and instructions are read from the worktree.
  An agent can no longer edit them: `.agents/` and `AGENTS.md` are protected
  by default (see `.agents/protected` in docs/api.md).

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

## The local inference server takes the machine down — open

2026-10-25. The local LLM brought the GPU driver down (`dxg` timeouts in the
kernel log, no OOM), and once the whole PC; it later died mid-request. Not
fennec's code, but fennec's load: long runs on one local GPU. To watch
alongside the review loop above.
