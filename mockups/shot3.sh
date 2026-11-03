#!/bin/sh
# Headers study render. Run: sh mockups/shot3.sh
cd "$(dirname "$0")"
python3 -m http.server 8765 >/dev/null 2>&1 &
SERVER=$!
sleep 1
npx -y @playwright/cli open --browser chromium http://127.0.0.1:8765/context-headers.html
npx -y @playwright/cli resize 1440 800
npx -y @playwright/cli screenshot --filename context-headers-desktop.png
npx -y @playwright/cli close
kill $SERVER 2>/dev/null
exit 0
