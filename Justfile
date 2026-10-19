# fennec-lead tasks: build, run in dev, install as a systemd user service

# Build the release binary the service runs
build:
  cargo build --release

# Run fennec in the foreground (debug build)
dev:
  cargo run

# Install fennec as a systemd user service (enable, do not start)
service-install: build
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
  systemctl --user enable fennec.service

# Start the fennec user service
service-start:
  systemctl --user start fennec.service

# Restart the fennec user service
service-restart:
  systemctl --user restart fennec.service

# Stop the fennec user service
service-stop:
  systemctl --user stop fennec.service
