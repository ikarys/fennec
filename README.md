# fennec

AI-assisted development for professional developers who want to stay the
authors of their software. Linux only for now.

fennec is being rebuilt as a Rust port of [openfox](https://github.com/co-l/openfox)
(MIT), one component at a time. The agent loop runs today, on the command line:

```bash
cargo build
cd /some/throwaway/repo
/path/to/fennec/target/debug/fennec run "create hello.txt containing hi"
fennec sessions          # list sessions
fennec log <session>     # replay one from the event store
```

It talks to any OpenAI-compatible endpoint (`FENNEC_LLM_URL`, default
`http://127.0.0.1:8000/v1`). Tools run on the host for now, with only a
working-directory guard on file paths: use a throwaway repository.

Contributor guide: [AGENTS.md](AGENTS.md). Licence: [AGPL-3.0-only](LICENSE);
third-party notices: [THIRD_PARTY.md](THIRD_PARTY.md).
