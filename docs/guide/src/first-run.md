# First run

fennec needs a model, a sandbox, git, and a project before a useful first session. This page walks through the setup checklist, model configuration, opening a project, and running a first session.

## Check what is missing

The home page shows a **Getting started** checklist. It checks what fennec needs before a first session and hides when all is ready.

| Check | What it verifies |
| --- | --- |
| **A model answers** | A reachable model endpoint |
| **Commands run in the sandbox** | bubblewrap on Linux, Seatbelt on macOS |
| **git is installed** | git is available |
| **A project is open** | A project folder is open |
| **Security scanners (optional)** | scanners for the security audit |

Each line shows **Ready**, **To do**, or **Check**, and a **Fix in Settings** link where applicable. `fennec doctor` prints the same lines in the terminal and exits 1 while one is to do.

```bash
fennec doctor
```

## Point fennec at a model

fennec starts with a `local` provider that uses an OpenAI-compatible endpoint.

| Variable | Default | Purpose |
| --- | --- | --- |
| `FENNEC_LLM_URL` | none | Endpoint for the `local` provider |
| `FENNEC_MODEL` | first model the endpoint lists | Model for the `local` provider |
| `FENNEC_API_KEY` | none | API key for the `local` provider |

You can also add providers on **Settings** > **Models**. The Settings page stores per-model parameters and applies them from the next turn. The composer changes a session's model.

With no model named, fennec still starts and everything that needs no model works, but a new session takes you to **Settings** > **Models** first.

If the endpoint is down, fennec still starts. You can reach the UI, and the chosen model is used once the endpoint is back.

## Open a project

From the home page, click **Open a project**. Choose the project's folder, then click **Open this folder**. To create a new folder, use **New project**, enter a folder name, and click **Create and open**.

The project page opens with its tasks, sessions, module map, and documentation.

## Start a first session

Click **New session in this project** or open a session from the project page. Describe the change in the composer.

The default agent is **Plan**. It is read-only and explores the codebase before proposing a plan. The plan card shows its **Done when** criteria and these actions:

- **Launch**: start the default workflow from this plan.
- **Builder alone, without criteria or review**: build without the full check and review round.
- **Save to backlog**: keep the plan in the project backlog without launching it now.
- **Revise**: revise the plan with the agent.

When you click **Launch**, fennec builds in a worktree, runs the criteria checks, verifies the result in a fresh context, and presents the review gate. You integrate only by clicking **Integrate**.

See [plans](plans.md) and [the review gate](review-gate.md).

## Use the CLI

```bash
fennec doctor
fennec
cd ~/code/my-project && fennec run "add a --verbose flag"
```

`fennec` opens the web UI on `http://127.0.0.1:7777`. `fennec run` runs the agent loop in the current directory.

## Change the port

If port 7777 is taken:

```bash
fennec --port 7788
```

Or set it for good in `~/.config/fennec/settings.yaml`:

```yaml
port: 7788
```

You can also edit it on **Settings** > **Security** > **Network**.

## What to do next

- [Sessions](sessions.md)
- [Configuration](configuration.md)
- [Remote access](remote-access.md)
- [Troubleshooting](troubleshooting.md)
