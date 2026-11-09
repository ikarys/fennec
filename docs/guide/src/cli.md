# Command line

Run fennec from a shell: start the server, run an agent turn, check setup, replay sessions, and update the binary.

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

## Remote password

```bash
fennec password
```

Prompts for a password twice and stores a PBKDF2 hash in `~/.config/fennec/settings.yaml`. After that, allow remote connections in Settings > **Network**, or set `listen: 0.0.0.0` in that file. See [remote access](remote-access.md).

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
```

`fennec service install` writes a systemd user service on Linux or a launchd agent on macOS, then starts it now and at each login. It runs the binary that ran the command, with the `PATH` of your shell. A service file already there is yours: it is kept as it is.

## Environment defaults

The command line reads the same environment defaults as the server: `FENNEC_LLM_URL`, `FENNEC_MODEL`, `FENNEC_API_KEY`, `FENNEC_MAX_TOKENS`, `FENNEC_COMPACT_AT`, `FENNEC_MODEL_CONCURRENCY`, and `FENNEC_DB`. The Settings page overrides them. See [configuration](configuration.md).
