# Install

fennec installs as one binary and can run as a user service. This page covers supported platforms, installation methods, service setup, and updates.

## Supported platforms

| Platform | Support | Command sandbox |
| --- | --- | --- |
| Linux x86_64 and aarch64 | Full support | bubblewrap |
| macOS Apple Silicon and Intel | Binaries and launchd service | Seatbelt |
| Windows | WSL2 only | Linux bubblewrap inside WSL2 |

On Windows, use WSL2 and open `http://127.0.0.1:7777` in the Windows browser. fennec's security model depends on the command sandbox. Linux has bubblewrap and macOS has Seatbelt. Windows has no equivalent that an unprivileged program can set up as simply, so native Windows is not supported today.

## Install script

```bash
curl -fsSL https://raw.githubusercontent.com/ikarys/fennec/main/install.sh | sh
```

The script installs the latest release into `~/.local/bin` after checking its checksum. Use `FENNEC_VERSION` to pick a tag and `FENNEC_INSTALL_DIR` to choose the directory.

## With mise

```bash
mise use -g github:ikarys/fennec
```

## Download a release

Download a tarball from the project's releases page by hand.

## Linux requirements

- glibc 2.35 or later, such as Ubuntu 22.04, Debian 12, and newer
- `bwrap` from the `bubblewrap` package for the command sandbox

On Ubuntu 23.10 and later, AppArmor can forbid the user namespaces bwrap needs. The **Settings** page shows the command to allow bwrap. See [troubleshooting](troubleshooting.md).

## From source

Building from source needs Rust stable, Node.js, and [just](https://github.com/casey/just).

```bash
just
just check
```

`just` builds the UI and runs the release server on `http://127.0.0.1:7777`. `just check` runs the contributor gate.

## Run as a service

If you installed fennec another way (mise, by hand), `fennec service install` sets the service up; `fennec service start|stop|restart` controls it ([command line](cli.md)).

On Linux, `install.sh` writes a systemd user service at `~/.config/systemd/user/fennec.service` and starts it. It is never overwritten once present. Set `FENNEC_SERVICE=none` to skip it. Without a systemd user session, run `fennec serve` yourself. If you use model environment variables, put them in the unit as `Environment=` lines.

```bash
journalctl --user -u fennec -f
sudo loginctl enable-linger "$USER"
```

On macOS, `install.sh` writes a launchd user agent at `~/Library/LaunchAgents/dev.fennec.plist` and starts it. It is never overwritten once present. Set `FENNEC_SERVICE=none` to skip it. Logs go to `~/Library/Logs/fennec.log`.

```bash
launchctl kickstart -k gui/$(id -u)/dev.fennec
```

Under WSL2, enable systemd in `/etc/wsl.conf`:

```ini
[boot]
systemd=true
```

## Update

```bash
fennec update --check
fennec update
```

The top bar shows an arrow when a newer release is available. A click installs it and restarts the server. The update is refused while a turn runs.

## Next

See [first run](first-run.md) for the setup checklist, model configuration, and your first session.
