# AGENTS.md — fennec

How to work in this repository. What fennec is and how to use it: `README.md`.

## Thesis

Code written by AI is abundant; understanding, mastery and responsibility are rare. fennec tools
what is rare: the human frames the work, the agent loop runs the steps, then waits for their
review. Nothing is integrated without a human decision. Every change serves that.

fennec is built one component at a time, each one working end to end before the next. When one
lands, add its paragraph to `docs/components.md`.

## Where things are

- `docs/map.md`: the modules and their dependencies, generated from the imports (regenerate it,
  never edit it). Each file's leading comment says what it is for.
- `docs/components.md` what fennec already does; `docs/api.md` the HTTP API and event wire format;
  `docs/ui.md` the UI's design rules; `SECURITY.md` the threat model and the known gaps;
  `docs/releasing.md` branches, versions, binaries.

## Architecture rules

- Dependencies point inward: adapters → agent → domain. The domain does no I/O; reading the clock
  and minting ids (`domain/clock.rs`, `domain/id.rs`) are its only impure leaves. The agent loop
  only knows the ports in `agent/ports.rs`; a port exists because it decouples a layer or has a
  test double, not for show.
- A root file named like a domain file is its adapter: `domain/budget.rs` decides, `budget.rs`
  reads the store for it. `main.rs` and `turn_build.rs` are the composition roots.
- A reusable, fennec-free concern gets its own crate (`crates/backlog`, `rules`, `canvas`,
  `token-usage`, and llm-providers-connector on crates.io for talking to models).
- One responsibility per file. No god file, no catch-all module (`utils`, `misc`, `common`,
  `helpers`). A Rust root file is an adapter or a composition root; `web/src` holds only
  `main.tsx` and `App.tsx` at its root.
- The UI holds no logic: the server folds and computes, the UI renders. One exception: the UI
  folds the live event stream into the thread's messages (`web/src/lib/fold.ts`) so a streamed
  token needs no round trip; pending decisions, numbers and provenance still come folded from the
  server. It uses the primitives of `web/src/components/ui` and is never dull (`docs/ui.md`).

## Non-negotiable rules

1. **Integration is always a human decision.** No agent, plugin or schedule commits to the
   human's branch on its own.
2. **Zero trust applies to models only.** The human is never blocked; the kernel (code that
   decides: integration, plugin activation) never runs what a model wrote; everything a model
   outputs is untrusted and runs in the sandbox. An agent can never install or enable a plugin.
3. **Provenance is shown.** What the UI shows is marked as verified (deterministic) or produced by
   a model (indicative).
4. **The server only answers local pages**: `Host` and `Origin` must be 127.0.0.1/localhost and
   POST bodies must be JSON. Never relax this. The one exception, opt-in remote access, requires
   a password (`web/guard.rs`, `web/login.rs`).
5. **The kernel never runs a repository's hooks**: every git command goes through `git.rs`, which
   disables hooks and fsmonitor.
6. **Storage is SQLite** (`rusqlite`, bundled). The event wire names and the SQLite schema are
   frozen: stored logs depend on them.
7. **The prompt only grows.** The inference server reuses its cache only for an exact prefix.
   Within a session never rewrite what was sent: the system prompt and tool definitions are frozen
   at the first turn, earlier messages fold back byte for byte. Editing history (compaction) costs
   a full re-read: do it deliberately, measured.
8. **Resources are a feature**: fennec stays light in RAM and CPU. Measure a change that could cost
   them, before and after.

## Working method

- Every change leaves the code at least as clean as it found it: when a file takes a second
  responsibility or a function stops fitting on a screen, split it in the same change.
- Name things after fennec's domain; prefer types (enums, newtypes) to strings and flags. fennec
  started from openfox (MIT, notice in `THIRD_PARTY.md`): no origin names in code, tests or UI.
- Code written by a model (the local LLM included) is reviewed against these rules, not only
  against the tests.
- Every input is hostile: arguments, files, environment, network, model output. A change touching
  parsing, SQL, command building, authentication, crypto, credentials or file import names the
  vulnerability class it guards against and its test (`SECURITY.md`).
- No new dependency without a stated reason in the commit body, checked against the registry and
  taken at its current version. Stdlib first.
- Deleting, skipping or weakening a test needs its reason in the commit message.
- No TODO, placeholder, commented-out or dead code in a delivered change. No secret anywhere.

## Build and check

```bash
just dev      # server + Vite dev server on :5173
just check    # the gate before any commit: fmt, clippy -D warnings, tests, UI lint, UI tests, UI build
just secrets  # gitleaks over the history
just setup    # once per clone: the git hooks
```

Both `just check` and `just secrets` pass before a change is called done. Other recipes: `just --list`.

## Commits and branches

- Conventional Commits, imperative, in English, subject and body, whoever writes them.
- Every commit with AI-written content ends with one `Assisted-by: <Tool>:<model>` trailer per
  model (`Assisted-by: Claude Code:claude-opus-5-5`, `Assisted-by: local:qwen3.8-27b`); never
  `Co-Authored-By` for a model.
- Feature branches go to `develop` by PR (squash, Conventional title). Versions are never edited by hand.
- Never without a human: merge to `main`, release, delete data.

Licence AGPL-3.0-only, contributions under the same licence (`CONTRIBUTING.md`). This project
follows the Vibecoding Convention at level 2 (`VIBECODING.md`).
