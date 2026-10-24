# AGENTS.md — fennec

## What this repository is

**fennec's thesis** (the human, 2026-10-24): code written by AI is abundant;
understanding, mastery and responsibility are rare. fennec tools what is rare.
It is for professional developers who work with AI and want to stay the
authors of their software: they frame the work, the agent loop runs the steps
their loop plans, then waits for their review. Nothing is integrated without
a human decision.

The full definition (needs, principles, capabilities C1–C26, conceptual
architecture, what is out of scope) lives in the Notion page "Définition de
Fennec (v1)"; the build steps in "Étapes de construction de la première
version". This repository is at **step 0** (foundation): the two earlier
repositories brought together, nothing of the v1 cycle built yet.

## Layout

```
crates/fennec-os/    the library: worlds (sandbox), agent engine, model gateway, guard, rules, roles
                     — its own AGENTS.md describes every module
crates/fennec-lead/  the `fennec` binary: local web UI on 127.0.0.1:7777, tasks, Fleet, plugins relay
                     — its own AGENTS.md describes the UI and its rules
```

`fennec-lead` depends on `fennec-os` only. Both histories were imported with
`git subtree` (2026-10-24) from `ikarys/fennec-os` and `ikarys/fennec-lead`.

## Trust zones

Every change is weighed against these three zones (the definition's
conceptual architecture):

1. **The human**: the UI, the configuration, the signing key. The human is
   free; fennec never blocks them, it only shows what they did.
2. **The trusted kernel**: code that runs on the host and decides — the guard,
   the decision registry, protected files, plugin activation, the decision to
   integrate. It never runs what a model wrote. In `fennec-os` it lives in
   `src/kernel/`.
3. **The untrusted zone**: everything a model outputs. It runs only inside a
   world (bubblewrap sandbox). The code that drives models lives in
   `src/engine/`; it is trusted code, but what it handles is not.

Modules that still mix zones (`engine::solve` lands work, `engine::spec` keeps
records) are split when the kernel is hardened (step 3).

## Build, test, run

Linux only (bubblewrap). From the repository root:

```bash
cargo build
cargo test --workspace          # needs bwrap and unprivileged user namespaces
cargo clippy --workspace --all-targets   # must be clean
cargo fmt --all
cargo run -p fennec-lead        # http://127.0.0.1:7777
```

UI tests: see `crates/fennec-lead/AGENTS.md`.

## Non-negotiable rules

1. **Integration is always a human decision.** No agent, plugin or schedule
   commits to the human's branch on its own.
2. **Zero trust applies to models only.** What a model outputs is untrusted;
   the human and the plugins they enable do what they want with their tool.
   An agent can never install or enable a plugin.
3. **Risk-proportional control.** Review, validated tests and the
   comprehension dialogue are mandatory for architecture, security and
   sensitive zones, optional elsewhere; what was skipped is marked "not
   reviewed" / "not validated".
4. **Provenance is shown.** What the UI shows is marked as verified
   (deterministic) or produced by a model (indicative).
5. **No database; no new dependency without a stated reason** in the commit
   body. Stdlib first.
6. Licence: AGPL-3.0-only. The repository stays private until the plugin SDK
   licence and contribution terms (DCO or CLA) are decided.

## Style

Rust 2024, rustfmt defaults, clippy clean. English for code, comments, docs
and commit messages. Comments explain *why*.

## Commit format

Conventional Commits, imperative mood (`feat:`, `fix:`, `docs:`, `refactor:`,
`test:`, `chore:`).
