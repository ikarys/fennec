#!/bin/sh
# Second render pass: tokens section + an expanded criterion. Run: sh mockups/shot2.sh
cd "$(dirname "$0")"
python3 -m http.server 8765 >/dev/null 2>&1 &
SERVER=$!
sleep 1

# Tokens section (scrolled to the bottom of the reworked panel)
npx -y @playwright/cli open --browser chromium http://127.0.0.1:8765/context-panel-v2.html
npx -y @playwright/cli resize 1440 900
npx -y @playwright/cli eval "() => { document.querySelectorAll('.pbody')[1].scrollTop = 99999; }"
npx -y @playwright/cli screenshot --filename context-panel-v2-tokens.png
npx -y @playwright/cli close

# 4/7 state with C7 expanded (its check command)
npx -y @playwright/cli open --browser chromium "http://127.0.0.1:8765/context-panel-v2.html#st1"
npx -y @playwright/cli resize 1440 900
npx -y @playwright/cli eval "() => { document.querySelectorAll('#critlist .crit')[6].classList.add('open'); }"
npx -y @playwright/cli screenshot --filename context-panel-v2-crit-open.png
npx -y @playwright/cli close

kill $SERVER 2>/dev/null
exit 0
