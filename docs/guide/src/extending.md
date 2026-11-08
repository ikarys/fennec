# Extending fennec

Add your own agents, skills, MCP servers, and project servers. fennec keeps its embedded defaults, reads your global files, and lets project files take the place of defaults for that project.

## Agents

An agent is a `*.agent.md` file. fennec reads agents from three places:

| Location | Where it appears | Ownership |
| --- | --- | --- |
| `~/.agents/agents/` | Settings > **Agents & skills** > **Yours** | You write, modify, and delete them. |
| `.agents/agents/` in the project | Settings > **Agents & skills** > **From the project** | They belong to the repository; modify them in the repository. |
| Embedded defaults | Settings > **Agents & skills** > **Provided by fennec** | Read-only. |

When the same agent id exists in more than one place, the project file is the one used for that project.

Create a global agent from Settings > **Agents & skills** > **New agent**. The editor asks for:

- **Id**: lowercase letters, digits, and hyphens.
- **Phase**: the group in the composer menu: **Start**, **Frame**, **Build**, **Understand**, **Check**, or none.
- **Default workflow**: optional workflow a message to this agent starts in.
- **Color** and **Description**.
- **Commands**: **Read-only** or **Write allowed**.
- **Session mode** or **Subagent**. A subagent runs only in a workflow node.
- **Tools**: the tools the agent may call.
- **Method**: the agent's prompt, step by step, and the format of its output.

An added agent appears in the composer menu. You can also **Duplicate** an embedded agent and edit the copy.

A minimal agent file:

```yaml
---
id: my-agent
name: My agent
description: One line: what this agent does.
phase: build
commands: read-only
allowedTools:
  - read_file
  - run_command
  - ask_user
---
Explain the method here.
```

### Embedded agents

| Agent | Use |
| --- | --- |
| `analyze` | **Charter**: frames a project with you, drafts `AGENTS.md`, and proposes first backlog tasks; changes nothing. |
| `ask` | **Question**: answers a human question about the project, read-only, every claim sourced. |
| `audit-architecture` | **Architecture audit**: audits responsibilities, layers, modularity, and maintainability without changing anything. |
| `audit-security` | **Security audit**: audits secrets, injections, input handling, dependencies, supply chain, and CI; runs known scanners first. |
| `builder` | **Builder**: implements the task by changing code and verifying it. |
| `narrate` | **Narrate**: narrates a branch's change file by file in review order, each claim sourced, without changing anything. |
| `plan` | **Plan**: explores the codebase and plans the task with you, without changing anything. |
| `remedy` | **Remedy**: turns measured risk zones into a short set of backlog tasks without changing anything. |
| `review` | **Review**: reviews a diff for you and for the project in a fresh context; subagent. |
| `tests` | **Tests**: writes the tests derived from the approved plan's criteria before any code; the human validates them, then they are protected. |
| `understand` | **Understand**: talks you through a branch's change, or any part of the project, until you can explain it; read-only and sourced. |
| `verify` | **Verify**: checks independently that the work meets its plan in a fresh context; subagent. |

## Skills

Skills are `SKILL.md` packages in `~/.agents/skills` and the project's `.agents/skills`. fennec lists them in the model's prompt and reads one with `load_skill`. You can also type `/<skill>` in a message; the composer menu offers the available skills.

Settings > **Agents & skills** > **Available skills** lists the skills. **Always-active skills** put their instructions into the system prompt of new sessions without `load_skill`. A session already started keeps its first-turn prompt.

### Browser skill

The embedded **browser** skill drives a browser through `run_command` with `@playwright/cli`:

```bash
npx -y @playwright/cli <command>
```

Commands include `open`, `snapshot`, `click`, `fill`, and `screenshot`. Screenshots are read back with `read_file` and kept out of the worktree's commits. To reach a dev server, start it with the play button and turn on **Network for commands** in Settings > **Sandbox**.

## MCP servers

Settings > **MCP** configures Model Context Protocol tool servers. Add one with **New server**.

The fields are:

- **MCP server name**
- **Command** and **Arguments** for a local command
- **URL** and **Transport** for a remote server
- **Authentication token**, optional; sent as a Bearer header to a server reached by URL, given in the `MCP_AUTH_TOKEN` environment variable to a command-line server

Use **Connect** to list a server's tools. For OAuth, use **Authorize with OAuth (opens the server’s consent page; back here, fennec connects)**.

The **On (default for new sessions)** setting sends the server's tools to the model. **Shown in the composer** shows them in the composer dropdown. A session's MCP setting overrides the default.

## Project servers

The project's servers are listed in `.agents/servers.json`. Open **Try the change** in a session header, or **Try it** in the integration dialog, then **Configure** to edit the list.

Example:

```json
[{"name": "web", "command": "npm run dev -- --port $PORT"}]
```

With no file, fennec guesses a command from `Justfile`, `package.json`, `Makefile`, or `Cargo.toml` and saves it once you confirm. Servers start in the session's worktree, outside the sandbox, with a free port in `$PORT`. The main checkout's `.env*` files are linked, not copied. Make your dev server read `$PORT` so several sessions can run side by side.

## Plugins

Plugins are global extensions defined in `~/.agents/plugins.json`, with code in `~/.agents/plugins/<name>/`. They observe session events and render typed blocks in the session panel, project dashboard, status bar, or Settings > **Extensions**. Enabling a plugin and granting it network access are your decisions; a changed definition or file deactivates it until you enable it again. See [plugins](plugins.md).
