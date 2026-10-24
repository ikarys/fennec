#!/usr/bin/env python3
"""fennec plugin: sends each event to the services listed in urls.txt
through Apprise (Discord, Telegram, Teams, Slack, ntfy, mail... one URL per
line, see https://github.com/caronc/apprise/wiki). Needs `apprise` on PATH
(`pipx install apprise`)."""

import json
import subprocess
import sys

event = json.load(sys.stdin)
body = event.get("message", "")
if event.get("url"):
    body += "\n" + event["url"]
try:
    done = subprocess.run(
        ["apprise", "--config", "urls.txt", "-t", "fennec · " + event.get("title", ""), "-b", body]
    )
except FileNotFoundError:
    # Shown as is in fennec's settings and Journal, which speak French.
    sys.exit("apprise n'est pas installé : pipx install apprise")
sys.exit(done.returncode)
