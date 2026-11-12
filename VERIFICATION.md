# How fennec is built and verified

## AI assistance

fennec is written with AI assistance, and this is stated openly. The human frames the work, reviews every change, and decides every integration.

Models used appear in each commit as `Assisted-by: <tool>:<model>` trailers: Claude Code with Claude models appears as `Assisted-by: Claude Code:<model>`, a local qwen3.8-27b through fennec itself appears as `Assisted-by: fennec:qwen3.8-27b`, and a local qwen3.8-27b delegated by Claude Code appears as `Assisted-by: local:qwen3.8-27b`. Commits before 2026-10-27 were written with openfox driving a local qwen3.8-27b and carry no per-commit trailer; the README section "How fennec is written" lists the models and how the history's trailers were rewritten before publication. Trust comes from the checks below, which anyone can re-run, not from who wrote the code.

## Requirements and how each is verified

| Requirement | Verified by |
| --- | --- |
| Formatting | `just lint` |
| Lint | `just lint` |
| Rust tests | `just check` runs `just test` |
| UI design rules | `web/scripts/check-ui.mjs` in `just lint` |
| UI unit tests | `just check` runs `just test-web` with Vitest |
| UI build | `just check` runs `just web` |
| Architecture rules | `crates/fennec/src/architecture.rs::fennec_keeps_its_own_architecture_rules` runs `.agents/architecture` (layer imports, no I/O in the domain) over fennec's production code in `just test`; the rest of AGENTS.md "Architecture rules" and "Working method" is human review |
| Security claims | SECURITY.md table |
| No secrets in history | `just secrets` (gitleaks) in CI on every pull request and in the pre-commit hook on staged files; `.gitleaksignore` lists the one known finding, a fake test token once in `crates/fennec/src/security_scan.rs` |
| Coverage, advisories, outdated dependencies, duplication | `just evidence` writes them to `audit/evidence/`; `vibecheck` checks them against the thresholds in VIBECODING.md and writes `audit/status.json` (the README badge) |
| Dependencies | Human review of the commit body: Rule 5 requires SQLite and no new dependency without a stated reason |
| Dependency licences | `just licenses` |
| Commit format | Conventional Commits, enforced by the git hook from `just setup` |
| Resources | Measured before and after changes that could cost RAM/CPU with `just perf`; results tracked in [`docs/devs/perf/`](docs/devs/perf/latest.md) |

## Continuous integration

The workflows run on GitHub Actions.

`ci.yml` runs on pull requests to `develop` and `main`. Its `guard` job runs on every pull request: it scans the history for secrets and validates the PR title with the commit-msg hook. The `check` and `tests` jobs are skipped on documentation-only changes. The `check` job builds the UI, runs lint, checks licenses, and runs UI tests, while the `tests` job installs bubblewrap, allows unprivileged user namespaces on Ubuntu, and runs `just test`.

`convention.yml` runs on pushes to `main`: it regenerates the evidence, runs `vibecheck --strict` and commits `audit/status.json` and `audit/evidence/` back to `main`.

`build.yml` is called by `release-please.yml` or by hand. It builds the web UI and the Rust binary for Linux x86_64, Linux aarch64, macOS aarch64, and macOS x86_64, packs a release tarball, copies `LICENSE`, `README.md`, and `THIRD_PARTY.md`, generates `THIRD_PARTY_LICENSES.md`, writes a SHA-256 file, and uploads the tarball and checksum as artifacts.

`pages.yml` runs on pushes to `main` for site and docs paths, and it can also run by hand. It builds the mdBook user guide into `site/docs` and deploys `site` to GitHub Pages.

`release-please.yml` runs on pushes to `main` and creates a release when release-please says one is needed. It calls `build.yml` with the release tag, uploads the build artifacts to the release, and merges `main` back into `develop`.

## Usability

Blind tests live in their own repository, https://github.com/ikarys/fennec-blind-tests. A blind test gives an agent only the user guide and the running UI, and the agent follows the guide on a project of its own; each test records the date, commit, findings, what was done about each one, and the agent's scripts and screenshots.

## Re-run it

Run these commands from the repository root: `just setup`, `just check`, and `just test`. `just setup` installs the git hooks once per clone.

`just check` runs `web` (the UI build), `lint` (`cargo fmt --check`, `cargo clippy -D warnings`, and `web/scripts/check-ui.mjs`), `test` (`cargo test`), `test-web` (Vitest), and `licenses`. `just test` runs all Rust tests, including the sandbox e2e tests, in a private TMPDIR; the sandbox e2e tests need a working bwrap, and on Ubuntu 24.04 AppArmor may block it, with the Settings page showing the command to allow it.

`just perf` builds the release binary and runs a scripted workload against it. The workload creates a project, runs several sessions with real tool calls (read, write, command), a compaction and a workflow, then times the pages. It runs in a temporary data directory and a throwaway git repository, so your own data is never touched. The model is a local fake with a fixed latency, so runs on the same machine compare across commits. `just perf real` runs the same workload against the model at `FENNEC_LLM_URL` (default `http://127.0.0.1:8000/v1`). The terminal table gives the delta against the previous run on the same machine and flags anything more than 20 % worse. Each run is appended to `docs/devs/perf/history.jsonl`, and `docs/devs/perf/latest.md` holds the last one.
