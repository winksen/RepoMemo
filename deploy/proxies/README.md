# Sample front doors

Configurations for putting RepoMemo behind a proxy you already run. Each points at the web image on
`127.0.0.1:8080` (start the stack **without** the `edge` profile: leave `COMPOSE_PROFILES` out of `.env`), which serves
the app and forwards the API.

| File | For |
|---|---|
| [nginx.conf](nginx.conf) | nginx with its own certificate |
| [traefik.yml](traefik.yml) | Traefik's file provider |

What every front door must do, and what both samples do:

- **Pass the client's address** in `X-Forwarded-For`, and say so to the server: `REPOMEMO_TRUST_PROXY=true` and, when the
  proxy is not on the same machine, `REPOMEMO_TRUSTED_PROXIES=<its address>`. Without it, every request seems to come from
  the proxy and the rate limits and the audit trail are shared by everyone.
- **Not buffer the live event streams** (`/v1/workspaces/<id>/events`, `/v1/system/events`), or events arrive in bursts.
- **Allow long uploads** (`client_max_body_size 1g` and no request buffering in nginx).
- **Send HSTS** (the web image speaks plain HTTP and sends none).
- **Tell the server its public address:** `REPOMEMO_PUBLIC_URL=https://…` and `REPOMEMO_ALLOWED_ORIGIN=https://…`.

`npm run container:check` reads these files: each must name the server's trusted-proxy setting and turn buffering off.
