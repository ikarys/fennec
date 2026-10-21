#!/usr/bin/env bash
# Try fennec here: seed a demo home with one project and one backlog task
# (a short title and a distinct description), then start the server.
# Usage: ./try.sh            # demo home in a temp folder, port 7777
#        H=/path ./try.sh    # seed that folder instead (must exist)
set -euo pipefail
cd "$(dirname "$0")"

PORT="${FENNEC_LEAD_PORT:-7777}"
if [ -n "${H:-}" ]; then
  HOME_DIR="$H"
else
  HOME_DIR="$(mktemp -d)"
fi
mkdir -p "$HOME_DIR/requests"

# The home is plain JSON (AGENTS.md): projects.json lists the projects,
# requests/<id>.json holds a task: its short title and its content.
printf '%s' "[{\"path\":\"$PWD\",\"archived\":false}]" > "$HOME_DIR/projects.json"
printf '%s' "{\"project\":\"$PWD\",\"title\":\"Short title\",\"text\":\"Distinct description\",\"status\":\"later\"}" > "$HOME_DIR/requests/cafe0001.json"

echo "fennec : http://127.0.0.1:$PORT (home: $HOME_DIR)"
exec env FENNEC_LEAD_HOME="$HOME_DIR" FENNEC_LEAD_ADDR="127.0.0.1:$PORT" \
  ./target/release/fennec
