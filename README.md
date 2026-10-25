# fennec

AI-assisted development for professional developers who want to stay the
authors of their software. Linux only for now.

fennec is being rebuilt as a Rust port of [openfox](https://github.com/co-l/openfox)
(MIT), one component at a time. Today: the agent loop, with a web UI and a CLI.

```bash
(cd web && npm ci && npm run build)   # the UI, embedded by cargo build
cargo build
target/debug/fennec                   # web UI on http://127.0.0.1:7777
cd /some/repo && /path/to/fennec/target/debug/fennec run "create hello.txt containing hi"
```

It talks to any OpenAI-compatible endpoint (`FENNEC_LLM_URL`, default
`http://127.0.0.1:8000/v1`). Tools run on the host for now, with only a
working-directory guard on file paths.

Contributor guide: [AGENTS.md](AGENTS.md). Licence: [AGPL-3.0-only](LICENSE);
third-party notices: [THIRD_PARTY.md](THIRD_PARTY.md).
