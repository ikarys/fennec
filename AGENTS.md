# AGENTS.md — fennec-lead

## What this repository is

**fennec's purpose** (the human, 2026-10-19): an assistant to build things with
AI **fast**, but **efficiently** and **safely**. Every feature is weighed against
these three; one that serves none of them is not built.

The lead layer of fennec: a developer leads one or several projects the way
they would lead a team. They state a request (fennec asks only when it could not
start at all), leave, and come back to a single view of what needs them. No tests
are written before the code by default: test-first is a rule the human says, and
what makes a request ready is the proof of the product. Execution lives in
`../fennec-os` (library); this repository only adds the human surface.
Scenarios: `../lead-scenarios.md`. Inventory and decisions: `../inventory.md`.

## Layout

```
src/main.rs   `fennec` binary: std HTTP server, routes, folder browser
src/fleet.rs  the Fleet: one queue of tasks for later, across projects, started on their own (`fleet.json`)
src/state.rs  requests and projects in files; the single view (what needs you, running, landed); a status change tells the enabled plugins (fennec-os `plugin`)
src/work.rs   request life in the background: stats per request and overall ("Stats"); conversation, direct work (fennec-os `spec::direct`), proof, retry; correct, commit, pass or drop; rule offers; journal of every step
ui/           index.html + app.js, embedded in the binary
examples/plugins/  plugins to copy into ~/.config/fennec/plugins/ (apprise: Discord, Telegram, Teams, ntfy...)
```

## Build, test, run

```bash
cargo build
cargo test
node --check ui/app.js && for t in ui/*.test.js; do node $t || echo "FAIL $t"; done  # UI tests (each extracts pieces of app.js, so --check guards the whole file)
cargo clippy --all-targets  # must be clean
cargo fmt --all
fennec                      # http://127.0.0.1:7777
```

Env: `FENNEC_LEAD_HOME` (default `~/.local/share/fennec-lead`), `FENNEC_LEAD_ADDR`
(default `127.0.0.1:7777`), `FENNEC_LEAD_ATTEMPTS` (default 1). The model server and
models are set in "Réglages" (`~/.config/fennec/model.json`); fennec-os's `FENNEC_OS_*`
variables override them.

## Non-negotiable rules

1. **The web UI is the only complete surface.** No CLI or TUI surface for the
   user; `fennec-os`'s CLI is a debug tool only. A plugin the human enabled may
   relay outside it (changed 2026-10-19, asked by the human): a notification,
   an answer to a question, or a decision already offered (validate, drop) —
   from senders they allowed, written in the request's Journal. Nothing else:
   no grant, rule, setting or wider access is ever given through a plugin.
2. **Seven words on screen**: project, request, done when, proof, question, rule,
   team (made of roles; added 2026-10-17, see `docs/design/team.md`). Internal
   concepts (world, run, grant, permissions, limits, stage keys) never reach the UI.
   One exception, outside the work itself: the "Réglages" dialog names the model
   server and its models, since choosing them is the human's setup; the "Équipe"
   dialog may name the model a role runs on, for the same reason. A second, asked
   for by the human: a request's "Journal" shows each step's exchange with the model
   as it was (messages, tool calls, tokens), since that is what they want to audit. A third, asked for
   too (2026-10-18): how the work was kept safe and what it cost, in plain words
   (isolated, without network, failed tries undone; model time, tokens, cache,
   speed) on each card, in the "Stats" dialog and on the project page, which
   also sets where fennec works ("Dans le dossier" or "À part") and lists the
   rules followed, to add or take back. A fourth, asked for too (2026-10-18):
   "Skills", the person's own word for the Agent Skills they keep, enabled once for
   every project and named on a request when its work read one. A fifth, asked for
   too (2026-10-18): "discussion", the person's word for a plain conversation
   with fennec in a folder of their choosing (git or not); the page follows the
   turn live, and the exchange can become a request. A sixth, asked for too
   (2026-10-19): "Fleet", the person's word for the one queue of tasks fennec
   runs on its own, across projects (see `docs/design/factory.md`).
3. **Nothing is "ready" without proof.** What could not be proven is shown as
   "not verified: X".
4. **Execution logic goes in fennec-os**, not here. This repository stays thin.
5. **No database.** State lives in files, as in fennec-os.
6. No new dependency without a stated reason in the commit body. Stdlib first.

## Style

Rust 2024, rustfmt defaults, clippy clean. UI: plain JS, no build step, served
by the binary. English for code, comments and docs; French for UI text.
Comments explain *why*.

## UI design

What the human expects without having to ask; apply it on every UI change.

- **The list shows what waits, runs and landed — nothing else.** Writing
  something (a request, a project) opens on demand in a dialog that closes once
  done; a form never sits open on the dashboard. Team, Stats and Réglages are
  pages named by the address, not dialogs: dialogs are only for short input.
- **Each part of a page is visibly its own**: a box or a separated block with
  an icon heading and a count; what waits for the human stands out (accent).
  Empty states are one small muted line.
- **Icons instead of words** on buttons and menus whenever the meaning stays
  clear (Lucide strokes in `ICONS`, `data-icon="name"` in HTML, `setIcon` in
  JS). An icon-only button carries `aria-label` and `title`. Words stay on the
  main action and wherever an icon alone would be a guess.
- **Mail-client layout**: projects on the left, requests in the middle, the
  chosen request or page on the right; the decision buttons on the right above
  the cost, the rest behind tabs.
- Works in light and dark (tokens on `:root`); desktop only: fennec is for computers, phone width is not a target.
- **Check before saying done**: screenshot the change (Playwright, headless
  Chromium) at desktop width in dark and light, against a
  copy of `FENNEC_LEAD_HOME` on a spare port — never the live instance.

## Commit format

Conventional Commits, imperative mood (`feat:`, `fix:`, `docs:`, `refactor:`,
`test:`, `chore:`).
