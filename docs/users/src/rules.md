# Rules

Define rules that fennec enforces before a tool call or workflow command runs. A **Block** rule on writes also reaches commands: [the sandbox](sandbox.md) mounts the files it covers read-only when a turn starts. A command can still create a blocked file that did not exist yet, and an **Ask** rule guards the file tools only.

## Where rules are managed

Manage rules in **Settings** > **Rules**. The section shows global, project, and session-granted rules. The project page shows the project's rules read-only.

Rules have a **Scope**, **Type**, **Pattern**, **Access**, **Action**, and optional **Note**. The description is **Applied by fennec before every tool call and workflow command. A path rule guards the file tools; a command can still reach what the sandbox allows.**

## Rule types

| Type | Pattern example | What it matches |
| --- | --- | --- |
| **Command** | `rm -rf` | A command, compared word for word |
| **Tool** | `web_search` | A tool name |
| **Path** | `~/.ssh/` | A path read or written |

Command matching is **word for word (program, words, options), through `sudo`, `sh -c`, `&&` and `|`**. fennec also parses `find -exec`. An **Allow** rule for a command covers only that exact command.

Path rules can use project-relative paths, absolute paths, or `~/…` paths. A path outside the project is refused, in read as in write, unless an **Allow** rule names it. An **Ask** rule pauses the turn until you answer.

## Actions

- **Allow** permits the matched call.
- **Ask** pauses the turn with the call open and shows **fennec awaits your approval for this call**.
- **Block** refuses the call before it runs. The tool card shows **Refused by the {scope} rule “{pattern}”** and **verified · rule**.

A block wins over the call: fennec does not run the matched command, tool, or path access.

## Approvals

When an **Ask** rule or a file tool reaching outside the workdir pauses a turn, the card offers four answers:

- **Once**: **Allow this call only**.
- **For this session: {scope}**: grant the path and its subdirectories, command, or tool for the session.
- **Always**: create a project rule, visible and removable in **Settings** › **Rules**.
- **Never**: refuse and block the exact path in a project rule, unless you pick a folder.

The folder scope is shown as **Folder allowed for Session, Always or Never** and ** and its subfolders**.

After you answer, fennec runs the recorded call itself or refuses it. The model does not retry the call. In night mode, an **Ask** is refused.

## File tools outside the workdir

A file tool reading outside the project normally asks for approval. If you already wrote that path, or a folder above it, in a message or answer in the session, a read runs without approval. `~/` and Windows `\\wsl.localhost\<distro>\` paths are recognized. Writes, commands, and your own **Ask** rules still ask.

## Kernel block rules

fennec enforces some limits directly:

- `.git` is read-only in the sandbox and refused to the file tools.
- Git hooks do not run on fennec's own git commands.
- These agent commands are refused outright: `git push`, `git update-ref`, `git branch -D`, `git worktree`, and `gh pr merge`.

## Protected files

Protected files are refused to the file tools and mounted read-only in the sandbox. The protected set includes fennec's `.agents`, `AGENTS.md`/`CLAUDE.md`, lockfiles in your own checkout, CI config, `.githooks`, accepted ADRs in `docs/adr`, and your `.agents/protected` list.

You can add paths to `.agents/protected`. In the integration dialog, **Protect the integrated tests ({count})** appends the integrated test files to `.agents/protected`.

If an agent needs a protected file, it can request a waiver. The **Waiver** card shows the reason and asks you to **Grant** or **Refuse**. A grant exempts exactly that path, in that session, for 1, 8, or 24 hours, and writes a MADR file to the session's `docs/adr/`. It also overrides a path rule that blocks that path, for the file tools. When a Block write rule covers the whole project folder, commands still cannot write it.

## Limits

Command rules match parsed tokens, so `eval` or a script can slip past them. The file tools run behind a working-directory guard, not in the command sandbox. A command can create a new file matching a protected pattern where none existed. The review gate and [sandbox](sandbox.md) are the last lines of defense.
