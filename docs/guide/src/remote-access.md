# Remote access

Keep fennec on your machine by default, or open it to other machines with a password and TLS.

## Local by default

fennec listens on `127.0.0.1` only and needs no password there. The server checks that requests come from local pages, so a web page cannot drive the agent. Local use stays password-free.

## Enable remote access

From the UI:

1. Open Settings > **Security** > **Network**.
2. Set the **Remote password** and click **Save password**. If no password is set, the page says **Set a password first**.
3. Tick **Allow remote connections**.
4. Click **Restart the server**.

From a shell:

```bash
fennec password
echo "listen: 0.0.0.0" >> ~/.config/fennec/settings.yaml
fennec
```

`fennec password` stores a PBKDF2 hash in `~/.config/fennec/settings.yaml`. With `listen: 0.0.0.0`, fennec refuses a non-local address without a password.

## Login and cookies

A remote browser gets a login page. After login, fennec sets a session cookie that lasts 30 days and is lost when fennec restarts. Local browsers do not log in.

## What stays local-only

The play button's servers and the preview still bind to `127.0.0.1`. Open them through an SSH tunnel or another local path, not through the remote address.

## Safe setups

fennec speaks plain HTTP. Across an untrusted network, put it behind TLS.

### SSH tunnel

```bash
ssh -L 7777:127.0.0.1:7777 nas
```

Then use `http://127.0.0.1:7777` in your browser. No remote password is needed because the connection stays local to the tunnel endpoint.

### Tailscale

Run fennec on a machine in your Tailscale network and open its Tailscale address. Tailscale provides the encrypted path.

### Reverse proxy

Use a reverse proxy that terminates TLS and passes the original `Host`. Caddy does this by default:

```caddyfile
fennec.example.lan {
    reverse_proxy 127.0.0.1:7777
}
```

nginx needs:

```nginx
proxy_set_header Host $host;
```

## Check the state

Settings > **Network** shows **password set** or **no password**, and **remote on** when remote connections are allowed. The **Port** setting is saved in `~/.config/fennec/settings.yaml` and applied at the next start; **Restart the server** applies it now.
