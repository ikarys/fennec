# fennec tasks. `just` alone builds the UI and the server, then runs fennec.

# Build everything and run fennec on http://127.0.0.1:7777
run: build
    cargo run --release -p fennec

# Build the web UI, then the server that embeds it
build: web
    cargo build --release -p fennec

# Build the web UI into web/dist (dependencies installed on first use)
web:
    cd web && ([ -d node_modules ] || npm ci) && npm run build

# Work on the UI: the server, plus Vite on http://localhost:5173 with live reload
dev:
    #!/usr/bin/env bash
    set -euo pipefail
    (cd web && ([ -d node_modules ] || npm ci))
    cargo build -p fennec
    target/debug/fennec & server=$!
    trap 'kill $server' EXIT
    cd web && npm run dev

# Everything CI would check
check: web
    node web/scripts/check-ui.mjs
    cargo fmt --all --check
    cargo clippy --all-targets -- -D warnings
    cargo test

# Once per clone: use the repository's git hooks (Conventional Commits, no direct push to main/develop)
setup:
    git config core.hooksPath .githooks

# Open the PR that promotes develop to main (then release-please prepares the release)
promote:
    gh pr create --base main --head develop --title "chore: promote develop to main" --body "Promote develop to main."
