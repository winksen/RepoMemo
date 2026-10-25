# Metrics in containers

The server answers `/v1/system/metrics` in the OpenMetrics text format (uptime, requests, errors, users, sessions,
workspaces, artifacts, storage, jobs, backups, disk) to a caller with a `system`-scope token. This folder is a working
example of reading it with Prometheus. It is off unless you ask for it.

1. Make a personal access token of the **system** scope (Profile > API tokens, as a system administrator) and put it,
   alone on a line, in `deploy/monitoring/metrics-token`. The file is not committed and the token can be revoked from
   the same page.
2. Start Prometheus with the stack: `docker compose --profile monitoring up -d` (or add `monitoring` to
   `COMPOSE_PROFILES`).
3. Open <http://127.0.0.1:9090> and query, for example, `repomemo_users`, `repomemo_http_requests_total` or
   `repomemo_disk_free_bytes`. Prometheus listens on this machine's loopback only; put your own front door in front of it
   or point an existing Prometheus at `server:3020` (its address on the compose network) with the same scrape block.

Prometheus keeps its data in the `prometheus-data` volume. The dashboards and the alerts are yours to write; the server's
metric names are in the operations guide (section 7).
