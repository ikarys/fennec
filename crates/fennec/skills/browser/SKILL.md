---
name: browser
description: Interactive browser control via playwright-cli commands (open, snapshot, click, fill, screenshot, etc.)
---

# @playwright/cli

Interactive browser automation via the official Microsoft [`@playwright/cli`](https://github.com/microsoft/playwright-cli) package.
Use `run_command` to execute these commands.

Invoke it through npx — works on any machine with Node, no global or project-local install required:

```bash
npx -y @playwright/cli <command>
```

> `-y` auto-confirms npx's one-time package download (cached afterwards). If you already installed it globally (`npm install -g @playwright/cli@latest`), the plain `playwright-cli` command works the same.

## In fennec's sandbox

fennec may run your commands in a sandbox (bwrap). Two consequences:

- Every process a command started ends with that command: the browser and any server you started do not survive to the next `run_command`. Chain a whole browser session in one command: `npx -y @playwright/cli open <url> && npx -y @playwright/cli snapshot && npx -y @playwright/cli screenshot; npx -y @playwright/cli close`. For several steps, put the commands in a script in the working directory and run it.
- Without network (the default), npx works from its cache only, and the browser reaches only servers started inside the same command, not one on the human's machine. If a command fails for lack of network, say so: the human can turn the network on in Settings > Security > Sandbox.

## Core Workflow

open a page -> snapshot to get element refs -> interact using refs -> snapshot again. Every command echoes the latest snapshot.

```bash
# Open a URL (headless by default; add --headed to show a window)
npx -y @playwright/cli open https://example.com
# Interact using refs from the snapshot
npx -y @playwright/cli click e42
npx -y @playwright/cli fill e15 "hello world"
# Re-snapshot to see the result
npx -y @playwright/cli snapshot
# Screenshot (only when a visual matters — snapshots are the norm)
npx -y @playwright/cli screenshot
# Close the browser when done — leaving it open leaks CPU
npx -y @playwright/cli close
```

## Local files

`@playwright/cli` cannot open `file://` URLs — `open file:///path/to/index.html` will not work. To inspect a local HTML file, serve it over HTTP from a detached process, then open the URL.

Outside a sandbox, `run_command` waits for the command's output to close, so detach the server from it so the command returns (in the sandbox, start it in the same command as the browser instead):

```bash
# Serve a specific folder on a custom port
nohup npx -y serve ./dist -p 8080 > /tmp/serve.log 2>&1 &
# Note the PID to stop it later
echo $!
```

Then open the address in the browser:

```bash
npx -y @playwright/cli open http://localhost:8080
```

Stop the server when you're done:

```bash
kill <pid>
```

## Cleaning up

Closing the browser is part of the workflow, not an afterthought: Chromium can leave orphaned processes behind (a known upstream quirk) that keep burning CPU. If you suspect a leftover browser, reclaim it:

```bash
# See what's still running
npx -y @playwright/cli list
# Close every browser session
npx -y @playwright/cli close-all
# Force-kill stale/zombie browser processes
npx -y @playwright/cli kill-all
```

## Screenshot workflow

`npx -y @playwright/cli screenshot` saves a PNG under `.playwright-cli/` in the working directory and prints its path. Always `read_file` that path right after: you see the image, and the human sees it in the session's Vision panel. Take a screenshot only when the visual matters (layout, colours, rendering); snapshots are the norm for structure and refs.

## All Commands

- `open [url]` -- open the browser (optionally to a URL)
- use `npx -y @playwright/cli --help` to get all available commands
