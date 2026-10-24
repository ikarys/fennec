# Reminding yourself of pending grants

`fennec-os grant remind` sends a desktop notification (`notify-send`, else
stderr) when a grant waits for `grant keep` or `grant revert`. Every other
`fennec-os` command prints the same one-line reminder on stderr. Nothing is
kept or reverted for you.

To be reminded daily, install a user timer (`~/.config/systemd/user/`):

```ini
# fennec-os-grants.service
[Service]
Type=oneshot
ExecStart=%h/.cargo/bin/fennec-os grant remind
```

```ini
# fennec-os-grants.timer
[Timer]
OnCalendar=daily
Persistent=true
[Install]
WantedBy=timers.target
```

`systemctl --user enable --now fennec-os-grants.timer`. Adjust `ExecStart` to
where the binary lives, and `FENNEC_OS_HOME` with `Environment=` if it is set.
