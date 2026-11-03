#!/bin/sh
# Renders the context-panel mockup to PNGs. Run: sh mockups/shot.sh
cd "$(dirname "$0")"
python3 -m http.server 8765 >/dev/null 2>&1 &
SERVER=$!
sleep 1

npx -y @playwright/cli open --browser chromium http://127.0.0.1:8765/context-panel-v2.html
npx -y @playwright/cli resize 1440 900
npx -y @playwright/cli screenshot --filename context-panel-v2-desktop.png
npx -y @playwright/cli close

npx -y @playwright/cli open --browser chromium "http://127.0.0.1:8765/context-panel-v2.html#st1"
npx -y @playwright/cli resize 1440 900
npx -y @playwright/cli screenshot --filename context-panel-v2-state47.png
npx -y @playwright/cli close

kill $SERVER 2>/dev/null
exit 0
