# Releasing

How a change reaches `main` and becomes a versioned release with Linux binaries.

## Branches and PRs

```text
feat/x
  -> PR to develop (CI, squash merge, Conventional Commits title)
  -> develop
  -> push to develop (CI + rolling edge prerelease)

just promote
  -> PR develop -> main (merge commit, never squash)
  -> push to main
  -> release-please opens "chore(main): release X.Y.Z"
     (updates Cargo.toml, Cargo.lock, version.txt, CHANGELOG.md)
  -> merging it tags vX.Y.Z
  -> publishes release with Linux binaries (x86_64, aarch64)
  -> merges main back into develop
```

## Versions

- SemVer from Conventional Commits: `feat` -> minor, `fix` -> patch, `!` -> major.
- Before 1.0, a breaking change bumps the minor.
- Never edit versions by hand.

## Local protection

Run `just setup` once per clone. Two hooks, both bypassed by `--no-verify` (the human's escape hatch):

- `pre-push` refuses direct pushes to `main` and `develop`.
- `commit-msg` refuses, on `develop` and `main` only, a subject that is not a Conventional Commit
  (git's own merge, revert and autosquash subjects pass). Feature branches take any message
  (`wip` included): the PR is squashed under its title, and CI runs the same script on that title,
  so the rule lives in one place.

An integration lands on the branch the human has checked out, so integrate on a feature branch,
not on `develop`.

## Repo settings

One-time, done with `gh api`:

- Allow squash and merge commit; disable rebase merge.
- Delete branch on merge: off. Without branch protection it would delete `develop` when a promote PR merges; delete feature branches by hand.
- Allow Actions to create PRs (release-please needs it).

## When fennec goes public

Add branch protection/rulesets for `main` and `develop`:

- Require PR.
- Require the `check`, `tests` and `title` status checks. `ci.yml` skips documentation-only changes
  (`paths-ignore`), and a skipped workflow leaves a required check pending forever: when
  they become required, move the docs filter into the jobs (a paths step) instead.
- No force-push.
- No deletion.

Today this is unavailable because the repo is private on a Free plan.

## Installing a binary

1. Download the tarball and `.sha256` from the release.
2. Run `sha256sum -c fennec-<label>-<target>.tar.gz.sha256`.
3. Extract and run `./fennec --version`.

It needs glibc 2.35 or later (built on Ubuntu 22.04) and `bwrap` for the command sandbox.
The `edge` prerelease is the latest develop build, the same way: not a release.

Requires glibc ≥ 2.35 (built on Ubuntu 22.04) and `bwrap` for the sandbox.
