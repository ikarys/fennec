#!/usr/bin/env bash
# Try fennec here: seed a demo home (two projects, a Backlog task and three
# landed tasks with stats) and start the server. The Stats page (#stats) then
# shows its tiles, by day, by task, by model and by project.
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
PROJ_A="$PWD"
PROJ_B="/tmp/fennec-demo-b"
printf '%s' "[{\"path\":\"$PROJ_A\",\"archived\":false},{\"path\":\"$PROJ_B\",\"archived\":false}]" > "$HOME_DIR/projects.json"

# A Backlog task, as before: framed ahead, validated into Ready.
printf '%s' "{\"project\":\"$PROJ_A\",\"title\":\"Short title\",\"text\":\"Distinct description\",\"status\":\"later\"}" > "$HOME_DIR/requests/cafe0001.json"

# Landed tasks with stats, on three different days and two models, so the
# Stats page has figures to show. `since` is the day the task was created.
NOW="$(date +%s)"
# The stats shape is what work::totals keeps: totals, per model, per stage.
stats() { # turns sent cached received ms model
  printf '%s' "{\"v\":4,\"failed_attempts\":0,\"turns\":$1,\"sent\":$2,\"cached\":$3,\"received\":$4,\"thinking\":0,\"ms\":$5,\"models\":{\"$6\":{\"turns\":$1,\"sent\":$2,\"cached\":$3,\"received\":$4,\"thinking\":0,\"ms\":$5}},\"stages\":{\"Work\":{\"turns\":$1,\"sent\":$2,\"cached\":$3,\"received\":$4,\"thinking\":0,\"ms\":$5}}}"
}
printf '%s' "{\"project\":\"$PROJ_A\",\"title\":\"Tune the stats page\",\"text\":\"Make the figures honest\",\"status\":\"landed\",\"since\":$((NOW-3600)),\"stats\":$(stats 4 1200 600 300 42000 gpt-a)}" > "$HOME_DIR/requests/cafe0002.json"
printf '%s' "{\"project\":\"$PROJ_B\",\"title\":\"Add a by-day table\",\"text\":\"Tokens per day\",\"status\":\"landed\",\"since\":$((NOW-3*86400)),\"stats\":$(stats 3 800 200 220 30000 gpt-b)}" > "$HOME_DIR/requests/cafe0003.json"
printf '%s' "{\"project\":\"$PROJ_A\",\"title\":\"Fix the project filter\",\"text\":\"Keep the page honest\",\"status\":\"landed\",\"since\":$((NOW-10*86400)),\"stats\":$(stats 2 500 100 120 18000 gpt-a)}" > "$HOME_DIR/requests/cafe0004.json"

echo "fennec : http://127.0.0.1:$PORT (home: $HOME_DIR)"
echo "Stats page: http://127.0.0.1:$PORT/#stats"
exec env FENNEC_LEAD_HOME="$HOME_DIR" FENNEC_LEAD_ADDR="127.0.0.1:$PORT" \
  ./target/release/fennec
