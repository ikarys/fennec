# Configuration

Reference for environment variables, `settings.yaml`, and the files and folders fennec reads or writes.

## Environment variables

| Variable | Default | Meaning |
| --- | --- | --- |
| `FENNEC_LLM_URL` | None | OpenAI-compatible endpoint of the `local` provider. Without it, set one on **Settings** > **Providers**. |
| `FENNEC_MODEL` | First model the endpoint lists | Model of the `local` provider. |
| `FENNEC_API_KEY` | None | API key of the `local` provider. |
| `FENNEC_MAX_TOKENS` | `32000` | Output token limit of a model call. |
| `FENNEC_COMPACT_AT` | `0.85`; `0` disables | Context threshold for automatic compaction. |
| `FENNEC_MODEL_CONCURRENCY` | `1` | How many model rounds may run at once in the process; useful for local endpoints that tolerate parallel requests. |
| `FENNEC_MODEL_AFFINITY_SECS` | `10` | How long a local model gate waits for the session it just served before another session may take the model. |
| `FENNEC_DB` | `~/.local/share/fennec/fennec.db` | Main SQLite store. |
| `FENNEC_USAGE_DB` | `usage.db` next to the store | Usage ledger. |
| `FENNEC_RULES_DB` | `rules.db` next to the store | Rules store. |
| `FENNEC_CANVAS_DB` | `canvas.db` next to the store | Canvas artifact store. |
| `FENNEC_VERSION` | Latest release | Release tag used by `install.sh`. |
| `FENNEC_INSTALL_DIR` | `~/.local/bin` | Directory used by `install.sh`. |
| `FENNEC_SERVICE` | Install script starts a service | `none` skips the systemd user service on Linux or the launchd agent on macOS. |
| `MCP_AUTH_TOKEN` | Not set | Set by fennec, not by you: a command-line MCP server receives its **Authentication token** in this variable. |

These runtime variables are defaults. The Settings page adds providers and overrides parameters per model, from the next turn on.

## settings.yaml

`~/.config/fennec/settings.yaml` (or `$XDG_CONFIG_HOME/fennec/settings.yaml`) stores server settings.

| Key | Meaning |
| --- | --- |
| `port: <n>` | Server port. `--port` wins over this value; the default is `7777`. |
| `listen: 0.0.0.0` | Allow remote connections. The default is local only. |
| Password hash | Written by `fennec remote enable` or `fennec password`; a PBKDF2 hash used for remote login. |

Settings > **Security** > **Network** edits the port and password and restarts the server. Other lines are kept.

## Files and folders

| Path | What fennec uses it for |
| --- | --- |
| `~/.config/fennec/settings.yaml` | Server port, listen address, and remote password hash. |
| `~/.local/share/fennec/fennec.db` | Main SQLite store. |
| `~/.local/share/fennec/usage.db` | Usage ledger, kept beyond sessions. |
| `~/.local/share/fennec/rules.db` | Rules. |
| `~/.local/share/fennec/canvas.db` | Canvas artifacts. |
| `~/.local/share/fennec/secret.key` | Master key that seals stored API keys at rest. |
| `~/.local/share/fennec/worktrees/` | Session worktrees, on branches named `fennec/<session-id>`. |
| `~/.agents/agents/` | Your global agents. |
| `~/.agents/skills/` | Your global skills. |
| `~/.agents/workflows/` | Your global workflows. |
| `~/.agents/plugins.json` | Plugin definitions. |
| `~/.agents/plugins/<name>/` | Plugin code. |
| `.agents/agents/` | Project agents. |
| `.agents/skills/` | Project skills. |
| `.agents/workflows/` | Project workflows. |
| `.agents/protected` | Project protected paths. |
| `.agents/architecture` | Architecture rules. |
| `.agents/servers.json` | Project servers. |
| `AGENTS.md` | Project instructions; protected from agents. |
| `docs/adr/` | MADR architecture decision records; listed and protected. |
| `docs/map.md` | Module map written from the project page. |

Project paths are relative to the project root.

## Precedence

- Environment variables provide defaults. Stored Settings page preferences override them from the next turn.
- `--port` wins over `settings.yaml`; the default port is `7777`.
- Workflows are found in the project's `.agents/workflows/`, then `~/.agents/workflows/`, then the embedded defaults; the first found wins for an id.
- API keys are sealed at rest with `secret.key` next to the store.
