# fennec-lead tasks: build, run in dev, install as a systemd user service

# Build the release binary the service runs
build:
  cargo build --release

# Run fennec in the foreground (debug build)
dev:
  cargo run

# Install fennec as a systemd user service and start it
service: build
  ROOT="$(dirname "{{justfile()}}")"
  mkdir -p ~/.config/systemd/user
  cat > ~/.config/systemd/user/fennec.service <<EOF
  [Unit]
  Description=fennec lead (local web UI)

  [Service]
  Type=simple
  ExecStart=$ROOT/target/release/fennec
  Restart=on-failure
  EOF
  systemctl --user daemon-reload
  systemctl --user enable --now fennec.service
