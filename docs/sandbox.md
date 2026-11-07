# The sandbox

> The short version: the agent writes and tests; **you** run the app (play button), install new dependencies and push. Everything below explains why, and how to do each of those in one step.

Every command the model runs, and every shell node in a workflow, runs isolated: bubblewrap on Linux, Seatbelt on macOS. The reason is simple: a model's output is untrusted, and a prompt injection in a README, a dependency, or a web page can make it run anything. The sandbox keeps that risk contained. You, the human, are never sandboxed: your terminal, your editor, the servers you start, and your own git all run with your normal rights.

## In one minute

| Inside the sandbox, a command… | Can | Cannot |
| --- | --- | --- |
| Files | Read the whole system read-only; write its working folder (the session's worktree, or the project folder for a session without one); run builds and tests offline (`cargo test`, `npm test`, `pytest`) — the worktree sees your checkout's `node_modules`/`.venv` read-only and shares the project's Cargo build folder. | Write outside its folder (`/tmp` is a private, empty one); change protected files (fennec's `.agents`, `AGENTS.md`/`CLAUDE.md`, lockfiles in your own checkout, CI config, `.githooks`, accepted ADRs in `docs/adr`, plus your `.agents/protected` list); change `.git` (config, hooks, refs). |
| Network | Reach the network only if you turned "Network for commands" on. | Reach the network by default; `npm` and `cargo` are put in offline mode. |
| Processes | Run a command in the foreground with a timeout. | Keep a process running after the command ends; no background `&`. |
| Secrets and sockets | — | See your secrets (`~/.ssh`, `~/.gnupg`, `~/.aws`, `~/.azure`, `~/.config/gcloud`, `~/.config/gh`, `~/.docker`, `~/.kube`, `~/.password-store`, `~/.netrc`, `~/.git-credentials`, `~/.npmrc`, `~/.pypirc`) nor fennec's own database; reach the Docker/Podman socket. |

## I want to see and test what the agent wrote

### See the changes

The session's Changes page shows each changed file as a diff. The work lives in a git worktree on its own branch, `fennec/<session-id>`, in a folder under `~/.local/share/fennec/worktrees/`. From your checkout, `git log fennec/<session-id>` and `git diff HEAD...fennec/<session-id>` work too. Nothing reaches your branch until you click "Intégrer".

### Open it in your own tools

The session panel's buttons open a terminal, your editor, or your file manager in the worktree. These are your tools: they run outside the sandbox, with your environment.

### Run the app

The play button in the session header, and the "Tester" section of the integration dialog, start the project's servers in the session's worktree, outside the sandbox. It is your action. The server gets a free port in `$PORT`. The panel shows starting/ready/exited, the URL it printed, and the last output lines.

The servers come from `.agents/servers.json`. With none, fennec guesses a command from the Justfile, `package.json`, `Makefile`, or `Cargo.toml` and saves it once you confirm. The main checkout's `.env*` files are linked in.

Example `.agents/servers.json`:

```json
[{"name": "web", "command": "npm run dev -- --port $PORT"}]
```

Make your dev server read `$PORT` so several sessions can run side by side.

### Why the agent cannot run the app itself

Its commands are foreground with a timeout, so a server would block until killed. With the network off, it has its own network namespace, so a port it opens is invisible to your browser. Let the agent run tests and builds; you run the app with the play button. If the agent must drive the app in a browser, the browser skill needs the server started with the play button and "Network for commands" turned on so its browser can reach it.

## Dependencies

- Offline by default: `npm` and `cargo` run in offline mode. They use what is already in your caches (`~/.npm`, `~/.cargo/registry`) and your checkout's `node_modules`.
- Adding a dependency is your decision: lockfiles are protected in your own checkout. In a worktree the lockfile is writable, and the review gate shows the change, but `node_modules`/`.venv` there are your checkout's, mounted read-only. `npm install <pkg>` inside the sandbox fails even with the network on.
- The recipe: put the package on disk yourself, then let the agent declare it in the worktree.

| Stack | You do | Then the agent |
| --- | --- | --- |
| npm | In your checkout: `npm install --no-save <pkg>` (it lands in the shared `node_modules` without touching your `package.json`). | Adds it to the worktree's `package.json`; imports resolve at once. You settle `package-lock.json` with `npm install` after integrating. |
| Rust | Turn "Network for commands" on for that turn. | Adds it to `Cargo.toml`; `cargo build` downloads it and updates the worktree's `Cargo.lock`. Turn the network off again. |
| Python | In your checkout: `uv pip install <pkg>` (or `.venv/bin/pip install <pkg>`). | Adds it to `pyproject.toml` / `requirements.txt`; the worktree's mounted `.venv` already has it. |

The worktree sees your checkout's `node_modules`/`.venv` live; if your checkout had none when the turn started, from the next turn.

## When a command fails because of the sandbox

| Symptom | Cause | What to do |
| --- | --- | --- |
| `Read-only file system` | Writing outside the folder, a protected file, `.git`, or a mounted `node_modules`. | Write in the worktree. For protected files, the agent asks you for a waiver. For dependencies, see Dependencies. |
| `Could not resolve host` / `Network is unreachable` | Network is off. | Turn on "Network for commands" in Settings → Sandbox, or do the step yourself. |
| npm `ENOTCACHED` / cargo `--offline was specified` / `failed to download` | Offline mode. | Same: turn on the network, or install in your checkout. |
| `No such file or directory` on `~/.ssh/...`, `Permission denied` reading a secret | Secrets are hidden on purpose. | Never give the agent your keys. Push, publish, and deploy are your actions. |
| `bwrap: ... Operation not permitted` / "bwrap cannot create its isolated space" | AppArmor on Ubuntu 23.10+ forbids the user namespaces bwrap needs. | The Settings page shows the command to allow bwrap; details are in `docs/releasing.md`. |
| `Background commands ('&') are not supported` | No background processes. | Use the play button for servers. |

## Settings

Settings → Sandbox has two switches.

"Run commands in the sandbox" is on by default. Off, the model's commands run on your machine with your full rights and secrets. The review gate still guards integration, but nothing guards your machine. Use this only for a throwaway VM or container.

"Network for commands" is off by default. On, commands reach the network: package installs, a dev server, `git fetch`. Secrets stay hidden, but a command could then send what it reads in the project elsewhere. `web_search` and `web_fetch` are fennec's own tools and do not depend on this switch.

## Git and the sandbox

fennec commits the agent's work in its worktree after each turn and cherry-picks it at integration, on the host. Because a model could have written a repository's hooks or config, `.git` is read-only in the sandbox and refused to the file tools, and **git hooks never run on fennec's own git commands**: the per-turn commit, the integration's cherry-pick and commit, status, and diff. Your hooks still run on your own `git commit`.

If a check you rely on lives in a hook, such as commit message format or a linter, run it after integrating, or make it a `check:` line in the plan's "Done when" so fennec runs it in the sandbox before you integrate. `git push`, `git update-ref`, `git branch -D`, `git worktree`, and `gh pr merge` are refused to the agent outright.

## macOS

Seatbelt gives the same world with three differences: commands get their own `TMPDIR`, not `/tmp`; dependency folders are linked into the worktree rather than mounted, and writes through the links are refused; renaming a protected file's parent folder is not refused, though the review gate still sees the change.

## What the sandbox does not promise

- The file tools, read/write/edit, run in fennec's process behind a path guard, not in bwrap.
- A command can create a new file matching a protected pattern where none existed.
- Command rules match parsed tokens, so `eval` or a script can slip past them. The sandbox is the wall; rules are guard rails.
- With the network on, a command can reach the internet.
- The review gate is the last line: nothing lands on your branch without your click.
