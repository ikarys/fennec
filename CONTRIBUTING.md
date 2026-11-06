# Contributing to fennec

Thanks for your interest. fennec is early; issues and pull requests are welcome.

## License of contributions

fennec is licensed under [AGPL-3.0-only](LICENSE). By submitting a contribution, you agree that it is
licensed under the same terms (inbound = outbound). There is no CLA to sign: nobody, the maintainer
included, can relicense your code without your consent.

You are responsible for what you submit, whoever or whatever wrote it: make sure you have the right to
contribute it, and that you understand it.

## Before opening a pull request

- Run `just setup` once per clone (git hooks), then `just check`: formatting, clippy, tests, UI lint,
  tests and build. CI runs the same gate.
- Open the pull request against `develop`, with a [Conventional Commits](https://www.conventionalcommits.org)
  title (`feat:`, `fix:`, `docs:`…), in English. It is squashed under that title.
- Read [AGENTS.md](AGENTS.md) for the architecture, layers and style; [docs/releasing.md](docs/releasing.md)
  for branches and versions.
