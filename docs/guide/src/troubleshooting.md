# Troubleshooting

Fix common setup, sandbox, integration, disk, and cost problems.

## Model and turns

| Symptom | Cause | Fix |
| --- | --- | --- |
| `fennec doctor` says `model` is to do | The model endpoint is not reachable, no model is listed, or the API key is missing. | Set `FENNEC_LLM_URL`, `FENNEC_MODEL`, and `FENNEC_API_KEY`, or add a provider in Settings > **Models**. Check **Probing…**. |
| A turn has no output | The model endpoint is down, the selected model is not listed, or the API key is missing. | Check Settings > **Models**, the provider's **Base URL**, and **API key**. Run `fennec doctor`. |
| An answer is cut short | `FENNEC_MAX_TOKENS` or **Max tokens per response** is too low. | Raise the limit in Settings > **Models**. |
| Slow prompts or high **Lost cache** in Insight | The local model cache is lost, several sessions take the model, or the context grows. | Compact the session, raise `FENNEC_MODEL_CONCURRENCY` if the endpoint tolerates parallel requests, and set `FENNEC_MODEL_AFFINITY_SECS` to keep a session on the model. See [insight](insight.md). |

## Sandbox

See [the sandbox](sandbox.md) for the full model of what is blocked.

| Symptom | Cause | Fix |
| --- | --- | --- |
| `Read-only file system` | Writing outside the work folder, a protected file, `.git`, or a mounted dependency folder. | Write in the worktree. For protected files, the agent asks you for a waiver. For dependencies, install them in your checkout first. |
| `Could not resolve host` or `Network is unreachable` | The command network is off. | Turn on **Network for commands** in Settings > **Sandbox**, or do the step yourself. |
| npm `ENOTCACHED`, cargo `--offline was specified`, or `failed to download` | `npm` and `cargo` run in offline mode. | Turn on the command network, or install the dependency in your checkout. |
| `No such file or directory` on `~/.ssh/...` or `Permission denied` reading a secret | Secrets are hidden on purpose. | Never give the agent your keys. Push, publish, and deploy are your actions. |
| `bwrap: ... Operation not permitted` or “bwrap cannot create its isolated space” | AppArmor on Ubuntu 23.10 and later forbids the user namespaces bwrap needs. | Run the command shown on Settings > **Sandbox**, then reload the page. |
| `Background commands ('&') are not supported` | Commands run in the foreground with a timeout. | Use the play button for servers. |

## Port in use

| Symptom | Cause | Fix |
| --- | --- | --- |
| Port `7777` is taken | Another process uses the port. | Run `fennec --port 7788`, set `port: 7788` in `~/.config/fennec/settings.yaml`, or change **Port** in Settings > **Security** > **Network** and restart. |

## Integration conflicts

| Symptom | Cause | Fix |
| --- | --- | --- |
| **Conflict with your branch** | Your branch changed the same lines as the agent since it started. The cherry-pick was cancelled. | Click **Resolve with the agent**, resolve by hand, or **Discard this work**. |
| **Uncommitted changes would be overwritten** | Your folder has local changes that the integration would overwrite. | Commit or stash them before integrating. |
| **The work started from …: you have changed branches since** | The current branch is not the branch the run started from. | Switch back to the expected branch, or resolve/discard the work. |
| **Nothing to integrate** | There are no commits, or the work is already in your branch. | Integrate by hand if needed, then click **Finish the task**. |

## Disk space

| Symptom | Cause | Fix |
| --- | --- | --- |
| Worktrees and build cache take disk space | Session worktrees and shared caches live under `~/.local/share/fennec/worktrees/`. | Open the project's **Disk space** section. Use **Free the build cache** and **Delete the worktree and its branch**. |
| **Orphans** remain | Folders no session or git knows anymore. | Delete them in a terminal. fennec never deletes them on its own. |

## Budget refusals

| Symptom | Cause | Fix |
| --- | --- | --- |
| A new turn is refused on a project | The project's monthly cost budget is reached. | Raise or remove the budget from the project page's **Set the monthly budget** tile. |
| The budget alert appears at 80% | Spend is approaching the limit. | Review [insight](insight.md) and the model's **Billed** setting in Settings > **Models**. |
