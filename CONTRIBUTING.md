# Contributing to fennec

Thanks for your interest. fennec is early; issues and pull requests are welcome.

## License of contributions

fennec is licensed under [AGPL-3.0-only](LICENSE). By submitting a contribution, you agree that it is
licensed under the same terms (inbound = outbound). There is no CLA to sign: nobody, the maintainer
included, can relicense your code without your consent.

You are responsible for what you submit, whoever or whatever wrote it: make sure you have the right to
contribute it, and that you understand it.

## Say which AI models you used

If an AI model wrote or changed part of your contribution, name it, with its version:

- in each commit concerned, as a trailer: `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`,
  `Co-Authored-By: Qwen3.8-27B (local) <noreply@example.com>`…;
- in the pull request description, a line `AI models: <model, version>` (or `AI models: none`).

This is not a judgement on the contribution: reviewers read model-written code differently, and fennec's own
risk view uses these trailers to find the code no human has reviewed yet.

## Dependencies

A new dependency, Rust or npm, must come under a licence the AGPL can absorb (MIT, Apache-2.0, BSD,
ISC, MPL-2.0…). `just licenses` checks it, and so does CI on every pull request; the list lives
in `scripts/licenses.py`. Say why the dependency is needed in the commit body (stdlib first).

## Before opening a pull request

- Run `just setup` once per clone (git hooks), then `just check`: formatting, clippy, tests, UI lint,
  tests and build. CI runs the same gate.
- Open the pull request against `develop`, with a [Conventional Commits](https://www.conventionalcommits.org)
  title (`feat:`, `fix:`, `docs:`…), in English. It is squashed under that title.
- Read [AGENTS.md](AGENTS.md) for the architecture, layers and style; [docs/releasing.md](docs/releasing.md)
  for branches and versions.
