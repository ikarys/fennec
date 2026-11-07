# fennec tasks. `just` alone builds the UI and the server, then runs fennec.

# Build everything and run fennec on http://127.0.0.1:7777
run: build
    cargo run --release -p fennec

# Build the web UI, then the server that embeds it
build: web
    cargo build --release -p fennec

# Build fennec with a local build number and install it to ~/.local/bin (systemd user service restarted)
install: web
    #!/usr/bin/env bash
    set -euo pipefail
    # <commits>.<sha>, -dirty when uncommitted changes went into the build.
    build="$(git rev-list --count HEAD).$(git rev-parse --short HEAD)"
    [ -z "$(git status --porcelain)" ] || build="$build-dirty"
    FENNEC_BUILD="$build" cargo build --release -p fennec
    FENNEC_LOCAL_BINARY=target/release/fennec sh install.sh

# Build the web UI into web/dist (dependencies installed again when the lockfile changes)
web:
    cd web && ([ node_modules/.package-lock.json -nt package-lock.json ] || npm ci) && npm run build

# Work on the UI: the server, plus Vite on http://localhost:5173 with live reload
dev:
    #!/usr/bin/env bash
    set -euo pipefail
    # The port the human opens: $PORT from the fennec servers panel, else 5173.
    # fennec's API runs on a private port next to it, so worktrees don't fight
    # over the default 7777; the UI proxies /api to it.
    UI_PORT="${PORT:-5173}"
    API_PORT="${PORT:-7777}"
    if [ "$API_PORT" = "$UI_PORT" ]; then
      API_PORT=$(( UI_PORT + 1 ))
    fi
    # npm ci again when package-lock.json changed (a pull added a dependency):
    # npm stamps node_modules/.package-lock.json at each install.
    (cd web && ([ node_modules/.package-lock.json -nt package-lock.json ] || npm ci))
    cargo build -p fennec
    # The installed fennec service holds 7777 and the same store: pause it while
    # the dev server runs, start it again on exit (Ctrl-C included).
    resume=:
    if [ "$API_PORT" = 7777 ]; then
      if command -v systemctl >/dev/null && systemctl --user is-active --quiet fennec 2>/dev/null; then
        systemctl --user stop fennec && resume='systemctl --user start fennec'
      elif command -v launchctl >/dev/null && launchctl list dev.fennec >/dev/null 2>&1; then
        plist="$HOME/Library/LaunchAgents/dev.fennec.plist"
        launchctl bootout "gui/$(id -u)" "$plist" && resume="launchctl bootstrap gui/$(id -u) $plist"
      fi
    fi
    target/debug/fennec --port "$API_PORT" & server=$!
    trap 'kill $server 2>/dev/null || true; wait $server 2>/dev/null || true; $resume' EXIT
    # strictPort: a busy UI port fails loudly instead of drifting to a port
    # the server's origin guard does not know (5174+ → "forbidden origin").
    cd web && FENNEC_API="http://127.0.0.1:$API_PORT" npm run dev -- --port "$UI_PORT" --strictPort

# Everything CI checks: the conventions (web, lint) and the tests
check: web lint test test-web licenses

# Every dependency the binary ships has a licence the AGPL can absorb (needs web/node_modules)
licenses:
    python3 scripts/licenses.py check

# The UI primitives rule, formatting, clippy
lint:
    node web/scripts/check-ui.mjs
    cargo fmt --all --check
    cargo clippy --all-targets -- -D warnings

# The unit tests of every crate. Their temp dirs go to one folder per checkout, emptied
# at each run: tests that forget to clean up no longer pile up in /tmp.
test:
    #!/usr/bin/env bash
    set -euo pipefail
    dir="${TMPDIR:-/tmp}/fennec-tests/$(basename "{{justfile_directory()}}")"
    rm -rf "$dir" && mkdir -p "$dir"
    TMPDIR="$dir" cargo test

# The unit tests of the UI's pure modules (the folds, the plan contract)
test-web:
    cd web && npm test

# Once per clone: use the repository's git hooks (Conventional Commits, no direct push to main/develop)
setup:
    git config core.hooksPath .githooks

# Open the PR that promotes develop to main (then release-please prepares the release)
promote:
    gh pr create --base main --head develop --title "chore: promote develop to main" --body "Promote develop to main."
