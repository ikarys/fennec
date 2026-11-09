---
version: "0.1.1"
level: copilot
processes:
  design: pair
  implementation: copilot
  testing: copilot
  documentation: copilot
  review: hint
  deployment: none
convention:
  version: "0.1"
  level: 2
  adopted_at: 8dd0d293
  status: active
  tools:
    - "Claude Code:claude-opus-5-5"
    - "Claude Code:claude-fable-5-1"
    - "local:qwen3.8-27b"
    - "fennec:qwen3.8-27b"
  attribution_trailers: [Assisted-by]
  coverage: { threshold: 80, kind: line, tool: cargo-llvm-cov }
  maintainability: { tool: "jscpd (duplication), clippy cognitive_complexity (complexity)", duplication_max: 4, complexity_max: 25, function_max: 110, file_max: 2900, markers_max: 170 }
  freshness: { max_outdated: 8 }
  evidence: { coverage: audit/evidence/coverage.txt, audit: audit/evidence/audit.json, outdated: audit/evidence/outdated.txt, maintainability: audit/evidence/jscpd-report.json }
  commands:
    test: "just test"
    lint: "cargo clippy --all-targets -- -D warnings"
    format: "cargo fmt --all --check"
    coverage: "cargo llvm-cov --workspace --summary-only"
    audit: "cargo audit --json"
    static_analysis: "cargo clippy --all-targets -- -D warnings"
    secrets: "gitleaks git --no-banner --redact ."
    outdated: "python3 scripts/outdated.py"
  maintainers: [ikarys]
  review: "single maintainer reads every diff; changes reach develop by pull request, direct pushes to main and develop are refused by the pre-push hook"
  security_response: "the maintainer acknowledges it within 7 days"
  licence: AGPL-3.0-only
---

# Vibecoding declaration

This project follows the [Vibecoding Convention](https://github.com/ikarys/vibecoding-convention)
version 0.1 at level 2. How each quality claim is checked is in
[VERIFICATION.md](VERIFICATION.md); the security claims are in [SECURITY.md](SECURITY.md).

## AI assistance

Most of fennec's code and text is AI-written. The maintainer frames each change,
reads it and decides; Claude Code (Opus, Fable) reviews and fixes; a local model
(qwen3.8-27b) writes bulk code, called from Claude Code (`local:`) or from fennec
itself (`fennec:`). Every AI-written commit carries an `Assisted-by: <tool>:<model>`
trailer.

## Review

Single maintainer. Every diff is read before it is merged into `develop`, by a
squashed pull request; `just check` (format, clippy, tests, UI lint, tests and
build) must pass first, locally and in CI.

## Provenance and licence

fennec is licensed under AGPL-3.0-only; the MIT notice of the code it started
from is kept in [THIRD_PARTY.md](THIRD_PARTY.md). Parts of it are written by AI
assistants and may not be protected by copyright in some jurisdictions; the
maintainer accepts this. To the maintainer's knowledge, no verbatim reproduction
of licence-incompatible code was included. Safeguards used: the maintainer reads
every diff; `just licenses` refuses a dependency under a licence the AGPL cannot
absorb.

## Status

Active, early. Vulnerabilities are reported privately on GitHub and acknowledged
within 7 days ([SECURITY.md](SECURITY.md)).

## Exceptions

- D3: commits before 2026-10-27 carry no attribution trailer; they were written
  with AI assistance too (the earlier code and its rebuild in Rust). The trailer
  is required from then on. Declared 2026-11-09.
- T4: no end-to-end test drives the UI in a browser yet. The user journeys are
  covered server-side: the Rust tests run the agent loop with a fake model over a
  real store, and the UI only renders what the server computes. Browser tests
  come after the first release. Declared 2026-11-09.
- T8: complexity is enforced, not recorded: clippy's `cognitive_complexity` lint
  at the declared threshold fails the lint step. File and function length and the
  markers are measured by `vibecheck`; the thresholds sit just above the state at
  adoption and only go down. No public API documentation lint: the library crates
  are not published and only this workspace uses them. Declared 2026-11-09.
