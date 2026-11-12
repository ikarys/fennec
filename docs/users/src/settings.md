# Settings

Configure fennec from the **Settings** page. Changes apply from the next turn, no restart, except server settings that need a restart.

The page opens as sub-pages in a side navigation, grouped under **General**, **Models**, **Agents**, **Security**, **Extensions**, and **About**.

## Appearance

**Appearance** sets **Language** (**English** or **Français**), **Theme**, **Reasoning open by default**, **Tool results open by default**, and **Text size**.

Choose the **IDE** that open-in-editor buttons target.

**Notices** has a **Sounds** switch. A short sound plays when a session needs you: a question, work to integrate, an error. Use **Play a sample** to hear a cue.

## Providers

The **Providers** page chooses the **Default model** for new sessions. The composer changes a session's model. The default model is marked ★; favorites (♥) are offered in the composer.

**Model parameters** include:

- **Max tokens per response**
- **Context window**
- **Compact at (%)** — 0 disables automatic compaction
- **Reasoning effort** — **Auto (no effort)** or an accepted effort

**Sent to the model** shows the stored values; **Save** applies them. The resolved values are shown as **Max tokens**, **Context**, **Compaction**, and **Effort**, with **sent**, **local**, or **not sent** badges. Use **Pin** to pin the context window.

**Auto config** makes a few one-token calls to the stored endpoint to discover accepted efforts and limits. Results are **indicative** until you click **Save**.

**Price ($ per million tokens, for the Insight estimate)** sets input, cached, and output prices. The **Billed** switch controls project budgets; a local model is not billed by default. See [insight](insight.md).

**Add a provider** asks for **Id** (no `/`, no spaces, unique), **Name**, and **Base URL**. An empty **Base URL** uses the environment URL (`FENNEC_LLM_URL`).

The provider is stored in the settings; its key, if any, is sealed. API key options are **None**, **Environment variable**, and **Stored key (encrypted)**.

Add models by id or click **Fetch** to read the endpoint's `/models` list. **Remove** deletes a provider; **Remove the model** deletes a model.

## Instructions

**Instructions** are added to the system prompt of each new session, before the project's `AGENTS.md` files. A session in progress keeps the prompt of its first turn.

**Agent style** chooses how the agents write to you; it applies to new sessions. Options are **Concise**, **Normal**, **Explanatory**, and **Technical**.

## Agents & delegation

Agents are the modes of a session: their method and the tools they are allowed to use. An added agent appears in the composer menu.

- **Yours** are files in `~/.agents/agents`. You write, modify, and delete them.
- **Provided by fennec** are embedded in the binary and read-only.
- **From the project** are files in `.agents/agents`. They belong to the repository; modify them in the repository.

Use **New agent**, **Edit**, **Duplicate**, or delete an agent to manage agents. The agent editor uses **Id** (lowercase letters, digits and hyphens), **Phase**, **Default workflow**, **Color**, **Description**, **Commands** (**Read-only** or **Write allowed**), **Subagent**, **Tools**, and **Method**. **Save** writes the agent.

**Ignored files** lists files in `~/.agents/agents` that cannot be read. Their agent appears in no menu.

**Delegation** controls whether an agent can hand a self-contained task to a read-only child agent. Use **Allow delegation** and **Max delegations per turn**.

## Skills

**Available skills** lists the skills fennec can load. **Always-active skills** put skill instructions into the system prompt of new sessions without `load_skill`. A session already started keeps its first-turn prompt.

## Workflows

**Available workflows** lists embedded workflows. **Optional workflows** enables the ones that are disabled by default. When checked, they appear in the list of each project's workflows.

## Autonomy

**Integration commit** can **Name the AI models in the commit**. When on, the integration commit adds one `Assisted-by: fennec:<model>` line per model that answered in the session. When off, it keeps only the `Fennec-Session` line.

**Night mode** controls unattended queue runs. Set **Max duration per idea (minutes)** and **Max tokens per idea**. A value of 0 means no limit. See [tasks](tasks.md).

## Sandbox & network

**Sandbox** has two switches:

- **Run commands in the sandbox** is on by default.
- **Network for commands** is off by default.

See [sandbox](sandbox.md) for what the sandbox blocks and how to work around it.

**Network** controls where fennec listens and who can connect. **Allow remote connections** listens on all interfaces; remote browsers must log in with the password. Set a password first in **Remote password**, then click the check mark next to it. See [remote access](remote-access.md).

**Port** is saved in `~/.config/fennec/settings.yaml` and applied at the next start. **Started with --port: the flag wins over this setting.** Use **Restart the server** to apply a port change. **Restart now? Open turns must be finished; the page follows the server.**

## Rules

The **Rules** page manages the allow, ask, and block rules fennec enforces before tool calls and workflow commands. Use **Add a rule** to add a rule by **Scope**, **Type**, **Pattern**, **Access**, and **Action**. **Note** is optional. See [rules](rules.md).

## Scanners

**Scanners** shows the security scanners fennec can find on your machine. fennec does not install them. The security audit agent runs the installed scanners, then reads the code. Scanners that query an online database need the sandbox network.

## Plugins

The **Plugins** page manages your plugins. Definitions live in `~/.agents/plugins.json`; code lives in `~/.agents/plugins/<name>/`. Enabling a plugin and granting it network access are your decisions. A changed definition or code deactivates it.

Use **Enable**, **Disable**, and **Network** for each plugin. If the code changed, **The code changed: enable it again to run this version.** appears. **Left out of ~/.agents/plugins.json** lists definitions fennec could not activate. See [extending](extending.md).

## MCP servers

The **MCP servers** page configures the model's tool servers. **On (default for new sessions)** sends a server's tools to the model. **Shown in the composer** shows them in the composer's dropdown. The composer's per-session setting overrides the server's default.

Use **New server** to add a server by **MCP server name**, **Command**, **Arguments**, **URL**, and **Transport**. **Connect** lists a server's tools. **Authorize with OAuth (opens the server’s consent page; back here, fennec connects)** starts OAuth. **Delete** removes a server.

Tools are grouped by access: **Read**, **Write**, **Destructive**, and **Undeclared**. **Declared by the server, not verified** marks access the server declared but fennec did not verify. An **Authentication token** is sent as a Bearer header to a server reached by URL, and given to a command-line server in its `MCP_AUTH_TOKEN` environment variable.

## Web search

The **Web search** page configures the SearXNG instance used by the `web_search` tool. Set the **SearXNG instance URL** and **API key (optional)**. The instance must accept the JSON format.

**Allow web_fetch** lets the model download pages whose URL it chooses. Local and private addresses are refused. fennec keeps pages in cache for 7 days. This is independent of the sandbox network.

## Shortcuts

The **Shortcuts** page changes the keys that open pages, start sessions, switch agents, and move between tabs. Use **Change the shortcut**, then **Press keys…**. **Restore the default** resets one action; **Restore every default** resets all. **None** disables a binding.

## About

The **About** page shows the running version, **Check for updates**, **What's new**, and **All releases on GitHub**. The top bar shows a star when new releases are available. **Release notes** opens the release notes for a version.
