# Command line

Run fennec from a shell: start the server, run an agent turn or a workflow, check setup, replay sessions, and update the binary.

## Start the server

```bash
fennec
# or
fennec serve
```

Starts the web UI on `http://127.0.0.1:7777`, or on the port in `~/.config/fennec/settings.yaml` (`port: <n>`).

Use a different port:

```bash
fennec --port 7788
fennec serve --port 7788
```

`--port` wins over the saved port. See [settings](settings.md).

## Run an agent turn

```bash
cd ~/code/my-project
fennec run "add a --verbose flag"
```

Runs the agent loop in the current directory. The default agent is `plan`, read-only. Use `builder` to change code:

```bash
fennec run --agent builder "fix the failing test"
```

Continue a session:

```bash
fennec run --session <id> "now update the docs"
```

Attach images:

```bash
fennec run --image ./diagram.png --image ./screenshot.png "explain this flow"
```

| Flag | Meaning |
| --- | --- |
| `--session <id>` | Continue an existing session. |
| `--agent <id>` | Use an agent id; `plan` is the default, `builder` writes code. |
| `--image <path>` | Attach an image; repeat the flag for more images. |
| `--quiet` | Hide the model's reasoning; answers and tool results still print. |

## Run a workflow

```bash
cd ~/code/my-project
fennec run --workflow plan-build-verify "add a --verbose flag"
```

Starts the workflow on the running server (start it with `fennec` or `fennec service start`), in the current directory's project, as if you had sent the task from the web UI. Each step prints its name, then its result: the plan prints in full before its gate.

A gate prints its question and numbered choices:

```text
Go with this plan?
[1] Go  [2] Revise
answer> 2 split step 2 in two
```

Type the number or the choice's name; the rest of the line is your comment. An agent's question inside a step takes a number or free text. A rule approval or a protected file change waits for you in the browser: the CLI prints the link.

At the end the CLI prints what fennec verified and the review page's link, `Review and integrate: http://127.0.0.1:7777/#/s/<id>/changes`. Integration stays a click in the browser.

| Flag | Meaning |
| --- | --- |
| `--workflow <id>` | The workflow to run, e.g. `plan-build-verify` or `quick`. |
| `--night` | Take the first choice at every gate, and the first option of every question (or let the agent decide). A permission request (a rule approval or a waiver) ends the run: it waits for you in the browser. |
| `--quiet` | Hide the model's reasoning; steps, gates and results still print. |
| `--port <n>` | The server to talk to, when it was started with `--port`; the default is the configured port. The CLI prints the server it uses before starting. |

The command exits with status 1 when the run does not end done.

## Check setup

```bash
fennec doctor
```

Checks a reachable model, the sandbox, git, an open project, and security scanners. It prints the same lines as the home page **Getting started** checklist and exits with status 1 while a line is to do.

## Sessions

```bash
fennec sessions
fennec log <session>
fennec compact <session>
```

- `fennec sessions` lists sessions.
- `fennec log <session>` replays a session from the event store.
- `fennec compact <session>` compacts a session in the current directory.

## Remote access

```bash
fennec remote enable
fennec remote disable
fennec remote status
fennec password
```

- `fennec remote enable` opens the server to the network (`listen: 0.0.0.0` in `~/.config/fennec/settings.yaml`). It first prompts for a password twice when none is set.
- `fennec remote disable` keeps the server on `127.0.0.1`.
- `fennec remote status` shows the listen address and whether a password is set.
- `fennec password` sets or changes the password, stored as a PBKDF2 hash.

Restart the server to apply a change (`fennec service restart`). See [remote access](remote-access.md).

## Update and version

```bash
fennec update --check
fennec update
fennec --version
```

`fennec update --check` reports whether a newer release is out. `fennec update` installs the latest release with the install script, which restarts the service (systemd user service on Linux, launchd agent on macOS). The update offered in the UI waits until no turn is running. `fennec --version` (or `-V`) prints the version.

## Run in the background

```bash
fennec service install
fennec service start
fennec service stop
fennec service restart
fennec service status
fennec service uninstall
```

`fennec service install` writes a systemd user service on Linux or a launchd agent on macOS, then starts it now and at each login. It runs the binary that ran the command, with the `PATH` of your shell. A service file already there is yours: it is kept as it is.

`fennec service status` reports running, installed but stopped, or not installed. `fennec service uninstall` stops the service, disables it, and deletes its file (the systemd unit or launchd agent); the binary stays.

## Environment defaults

The command line reads the same environment defaults as the server: `FENNEC_LLM_URL`, `FENNEC_MODEL`, `FENNEC_API_KEY`, `FENNEC_MAX_TOKENS`, `FENNEC_COMPACT_AT`, `FENNEC_MODEL_CONCURRENCY`, and `FENNEC_DB`. The Settings page overrides them. See [configuration](configuration.md).
