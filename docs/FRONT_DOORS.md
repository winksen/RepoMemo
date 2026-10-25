# A front door for a direct install

A direct install (the archive, [operations guide §4](../mindmap/technical/operations.md)) serves the web app and the API on
one port, 3020, in plain HTTP. This page gets you from there to a working `https://` address. Pick one of three ways.

| Way | Choose it when |
|---|---|
| **Caddy** in front | you have a public name and want the certificate to be automatic (the simplest) |
| **nginx** in front | nginx already runs on the machine |
| **The server alone** | a trusted network, no TLS, no public name: `http://machine:3020` |

Whichever you pick, three settings go in `/etc/repomemo/repomemo.env` (Windows: `C:\ProgramData\RepoMemo\repomemo.env`), and the
service is restarted after changing them:

```
REPOMEMO_PUBLIC_URL=https://memo.example.com
REPOMEMO_ALLOWED_ORIGIN=https://memo.example.com
REPOMEMO_TRUST_PROXY=true
```

The server keeps listening on `127.0.0.1:3020` (`REPOMEMO_SERVER_ADDR`), so only the front door on the same machine reaches it;
it believes the front door's forwarding headers because it is on the loopback.

## 1. Caddy (Linux)

1. Point your name's DNS record at the machine; open ports 80 and 443 (below).
2. Install Caddy from your distribution or [caddyserver.com](https://caddyserver.com/docs/install).
3. Put [deploy/direct/Caddyfile](../deploy/direct/Caddyfile) in `/etc/caddy/Caddyfile` with your name in place of `memo.example.com`.
4. `sudo systemctl reload caddy`. Caddy gets the certificate and renews it by itself.
5. Open `https://memo.example.com` and finish the first-run setup.

## 2. nginx (Linux)

1. DNS and ports as above. `sudo apt install nginx certbot python3-certbot-nginx`.
2. `sudo certbot certonly --nginx -d memo.example.com`.
3. Put [deploy/direct/nginx.conf](../deploy/direct/nginx.conf) in `/etc/nginx/conf.d/repomemo.conf` with your name.
4. `sudo nginx -t && sudo systemctl reload nginx`. Certbot renews on its own timer.

## 3. Windows

Run [Caddy for Windows](https://caddyserver.com/docs/install#windows) with the same Caddyfile, or put the server behind IIS
with URL Rewrite and Application Request Routing (turn response buffering off for the event streams). With Caddy:
`caddy run --config C:\caddy\Caddyfile`, or register it as a service with the method Caddy's page describes.

## Opening the ports

- **Linux, ufw:** `sudo ufw allow 80,443/tcp`. **firewalld:** `sudo firewall-cmd --permanent --add-service=http --add-service=https && sudo firewall-cmd --reload`.
- **Windows:** `New-NetFirewallRule -DisplayName "RepoMemo HTTPS" -Direction Inbound -Protocol TCP -LocalPort 80,443 -Action Allow` in an administrator PowerShell.
- Do **not** open 3020: it is for the front door on the same machine.

## Checking it

```
curl -I https://memo.example.com/health/ready      # 200, with Strict-Transport-Security
sudo journalctl -u repomemo -n 20                  # the server's own log (Windows: the logs folder under workspace-data)
```

If sign-in works but live updates stop after a minute, the front door is buffering the event streams: use the files above as they
are. If every request seems to come from one address, `REPOMEMO_TRUST_PROXY` is not set.
