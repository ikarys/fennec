# The sandbox

The short version: the agent writes and tests; you run the app, install new dependencies, and push. The sandbox keeps the agent's commands contained.

Every command the model asks to run, and every shell node in a [workflow](workflows.md), runs in the sandbox. File tools are checked by fennec before they touch a path, but they are not run inside the sandbox. You are never sandboxed: your terminal, your editor, your git, and the servers you start run with your normal rights.

## What runs in the sandbox

A sandboxed command can:

- Read the system read-only.
- Write its working folder: the session's worktree, or the project folder for a session without one.
- Run builds and tests offline, such as `cargo test`, `npm test`, and `pytest`.
- Use the checkout's `node_modules` or `.venv` read-only, and share the project's Cargo build folder.
- Read your checkout from a worktree, so git and a `.venv`'s scripts work even when the project lives under `/tmp` (the sandbox's own `/tmp` is empty).

A sandboxed command cannot:

- Write outside its working folder. `/tmp` is a private, empty folder.
- Change protected files.
- Change `.git` (config, hooks, refs).
- Reach the network unless you turn on **Network for commands**.
- Keep a process running after the command ends. Background `&` is not supported.
- See your secrets, fennec's database, or the Docker/Podman socket.

## What it protects

### Files

The system is read-only. The working folder is writable. Protected files are read-only.

fennec protects its own files and the files you list in `.agents/protected`. In your own checkout, the protected set includes `.agents`, `AGENTS.md` and `CLAUDE.md`, lockfiles, CI config, `.githooks`, and accepted ADRs in `docs/adr`. In a session worktree, the agent can write the worktree's lockfile; the review gate shows that change.

If the agent needs a protected file, it can ask for a waiver. You **Grant** or **Refuse** it. A grant is for that path, that session, and 1, 8, or 24 hours. It writes an ADR to the session's `docs/adr/`.

### Network

By default, commands have no network. `npm` and `cargo` run in offline mode. Turn on **Network for commands** in [Settings](settings.md) → **Sandbox & network** when a step needs the network.

### Secrets

These folders are hidden from commands: `~/.ssh`, `~/.gnupg`, `~/.aws`, `~/.azure`, `~/.config/gcloud`, `~/.config/gh`, `~/.docker`, `~/.kube`, `~/.password-store`, `~/.netrc`, `~/.git-credentials`, `~/.npmrc`, and `~/.pypirc`. fennec's own store is hidden too.

## What it does not protect

- File tools are not run in the sandbox. They are guarded by fennec's path checks and protected-file rules.
- A command can create a new file that matches a protected pattern where no protected file existed.
- A protected file below the top of the folder can be replaced by renaming its parent folder. The read-only mount only holds at the top level, such as `.agents`, `.github`, or a root lockfile. The review gate still blocks integration on a protected-path change.
- Command rules match parsed tokens. `eval` or a script can slip past them. The sandbox is the wall; [rules](rules.md) are guard rails.
- With the network on, a command can reach the internet. Turn the network on for the step that needs it, then off again.
- fennec's own server refuses a sandboxed command, except through Vite's dev proxy on `:5173` while you run `just dev`.
- The servers you start with **Try it** run outside the sandbox, as you, with your environment. They run the project's code, so start them on a branch you have read.
- `~/.cargo/registry`, `~/.cargo/git`, `~/.cache`, and `~/.npm` stay writable so builds and installs work. A command could leave a tampered file there that a later build reuses.
- The rest of your home stays readable to commands, except the hidden secrets and fennec's store.
- `web_fetch` and `web_search` run in fennec's process, not in the sandbox. They reach the web even with the network off.
- The review gate is the last line. Nothing lands on your branch without your click. See [review gate](review-gate.md).

## Day to day

### See and test what the agent wrote

The session's **Changes** page shows each changed file as a diff. The work lives in a git worktree on its own branch, `fennec/<session-id>`, in a folder under `~/.local/share/fennec/worktrees/`. From your checkout, `git log fennec/<session-id>` and `git diff HEAD...fennec/<session-id>` work too. Nothing reaches your branch until you click **Integrate**.

The **Work** panel opens your own tools in the worktree. These run outside the sandbox, with your environment. You can use **Open a terminal in the worktree**, open the **IDE**, or open the folder in your file manager. The project page has **Open a terminal in the project**. For CLI projects, a terminal opens in the worktree.

**Try it** starts the project's servers in the session's worktree, outside the sandbox. It appears in the **Work** panel, the integration dialog, and the last run's end card. It opens the **Servers** panel, which shows each server's state, port, URL, and last output lines. You can **Add a server**, **Configure** it, or **Show the output**.

The servers come from `.agents/servers.json`. With none, fennec guesses a command from the Justfile, `package.json`, `Makefile`, or `Cargo.toml` and saves it once you confirm. The main checkout's `.env*` files are linked in, never copied. Make your dev server read `$PORT` so several sessions can run side by side.

Example `.agents/servers.json`:

```json
[{"name": "web", "command": "npm run dev -- --port $PORT"}]
```

The agent cannot run the app itself in the usual way. Its commands are foreground with a timeout, so a server would block until killed. With the network off, a port it opens is invisible to your browser. Let the agent run tests and builds; you run the app with **Try it**. If the agent must drive the app in a browser, the browser skill needs the server started with **Try it** and **Network for commands** turned on.

### Dependencies

Offline by default, `npm` and `cargo` use what is already in your caches (`~/.npm`, `~/.cargo/registry`) and your checkout's `node_modules`.

Adding a dependency is your decision. Lockfiles are protected in your own checkout. In a worktree, the lockfile is writable and the review gate shows the change, but `node_modules` and `.venv` are your checkout's, mounted read-only (linked on macOS). `npm install <pkg>` inside the sandbox fails even with the network on.

The recipe: put the package on disk yourself, then let the agent declare it in the worktree.

| Stack | You do | Then the agent |
| --- | --- | --- |
| npm | In your checkout: `npm install --no-save <pkg>` | Adds it to the worktree's `package.json`; imports resolve at once. You settle `package-lock.json` with `npm install` after integrating. |
| Rust | Turn **Network for commands** on for that turn | Adds it to `Cargo.toml`; `cargo build` downloads it and updates the worktree's `Cargo.lock`. Turn the network off again. |
| Python | In your checkout: `uv pip install <pkg>` (or `.venv/bin/pip install <pkg>`) | Adds it to `pyproject.toml` / `requirements.txt`; the worktree's mounted `.venv` already has it. |

The worktree sees your checkout's `node_modules` and `.venv` live. If your checkout had none when the turn started, it sees them from the next turn. Outside the sandbox, the worktree's `.venv` or `node_modules` can be an empty folder, the sandbox's mount point, so a command you run by hand in the worktree does not find them. Run it from your checkout, or through a shell step.

### When a command fails because of the sandbox

With **Run commands in the sandbox** on, you do not mark a command as sandboxed. fennec sandboxes it. If a command fails with one of these symptoms, the sandbox is the likely cause.

| Symptom | Cause | What to do |
| --- | --- | --- |
| `Read-only file system` | Writing outside the folder, a protected file, `.git`, or a mounted `node_modules`. | Write in the worktree. For protected files, the agent asks you for a waiver. For dependencies, see Dependencies. |
| `Could not resolve host` / `Network is unreachable` | Network is off. | Turn on **Network for commands** in [Settings](settings.md) → **Sandbox & network**, or do the step yourself. |
| npm `ENOTCACHED` / cargo `--offline was specified` / `failed to download` | Offline mode. | Turn on the network, or install in your checkout. |
| `No such file or directory` on `~/.ssh/...`, `Permission denied` reading a secret | Secrets are hidden on purpose. | Never give the agent your keys. Push, publish, and deploy are your actions. |
| `bwrap: ... Operation not permitted` / "bwrap cannot create its isolated space" | AppArmor on Ubuntu 23.10+ forbids the user namespaces bwrap needs. | The Settings page shows the command to allow bwrap; see [install](install.md) and [troubleshooting](troubleshooting.md). |
| `Background commands ('&') are not supported` | No background processes. | Use **Try it** for servers. |

### Git and the sandbox

fennec commits the agent's work in its worktree after each turn and cherry-picks it at integration, on the host. Because a model could have written a repository's hooks or config, `.git` is read-only in the sandbox and refused to the file tools. Git hooks never run on fennec's own git commands: the per-turn commit, the integration's cherry-pick and commit, status, and diff. Your hooks still run on your own `git commit`.

If a check you rely on lives in a hook, such as commit message format or a linter, run it after integrating, or make it a `check:` line in the plan's **Done when** list so fennec runs it in the sandbox before you integrate. See [plans](plans.md).

`git push`, `git update-ref`, `git branch -D`, `git worktree`, and `gh pr merge` are refused to the agent.

## Settings

[Settings](settings.md) → **Sandbox & network** has two switches.

**Run commands in the sandbox** is on by default. Off, the model's commands run on your machine with your full rights and secrets. The review gate still guards integration, but nothing guards your machine. Use this only for a throwaway VM or container.

**Network for commands** is off by default. On, commands reach the network: package installs, a dev server, `git fetch`. Secrets stay hidden, but a command could then send what it reads in the project elsewhere. `web_search` and `web_fetch` are fennec's own tools and do not depend on this switch.

## Setup

### Linux

Install bubblewrap (`bwrap`). If Settings shows **bwrap unavailable**, install the bubblewrap package and reload the page.

On Ubuntu 23.10 and later, AppArmor may forbid the user namespaces bwrap needs. Run the command shown in [Settings](settings.md) → **Sandbox & network**, then reload the page. See [install](install.md).

### macOS

fennec uses Seatbelt. Commands get their own `TMPDIR`, not `/tmp`. Dependency folders are linked into the worktree rather than mounted, and writes through the links are refused. Renaming a protected file's parent folder is not refused.

### WSL

Use the Linux bwrap setup. If bwrap is missing, install it in the WSL distribution and reload the page.
