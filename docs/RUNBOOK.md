# Operator runbook

Short procedures for the person who keeps a RepoMemo server running: what to look at, what a healthy answer is, what to do when it is not. Every command here is one the product ships: the console commands are in the [admin console reference](../mindmap/technical/admin-console.md#first-slice-commands) and the scripts are in `deploy/`. The reasons and the limits are in the [production guide](../mindmap/technical/operations.md).

**Conventions.** `repomemo-console` needs a system administrator's session or a personal access token with the `system` scope (`REPOMEMO_TOKEN`). A console command that changes something only previews its effect until you add `--yes`. In containers, prefix server commands with `docker compose exec server`. The examples use `http://127.0.0.1:8080`, the web image's loopback port.

## 1. Is it healthy?

```bash
curl -s http://127.0.0.1:8080/health
curl -s -o /dev/null -w '%{http_code}\n' http://127.0.0.1:8080/health/ready
repomemo-console status
```

| Signal | Healthy | If not |
|---|---|---|
| `/health` | 200, `status: ok`, the `version` you expect, `setup_required: false` | No answer: the process or the proxy is down (§ 2). `setup_required: true` on a server that had accounts: it is looking at a new, empty environment ([Troubleshooting](TROUBLESHOOTING.md)) |
| `/health/ready` | 200 | 503: the database does not answer; read the logs, check the disk and the data folder |
| `status` | version, uptime, free disk above the floor, queues draining, a recent backup | Act on whichever line is off: disk (§ 5), backup (§ 6), queues (§ 4) |
| Containers | `docker compose ps` shows every service `healthy` | `docker compose logs <service>` for the one that is not |

## 2. Logs

```bash
repomemo-console logs --category security --level warn --limit 50
repomemo-console logs --category jobs --level error
docker compose logs --since 30m server
```

Categories: `security`, `jobs`, `http`, `ai`, `performance`. The same events are in *System › Logs*, and the files are in `<data folder>/logs/`. To see more for a while: `repomemo-console settings set log_http_level debug --yes`, then reset it with `settings reset log_http_level --yes`.

**Expected:** a start line with the version, then ordinary `http` and `jobs` entries. **Recover:** a repeated `security` warning is a lockout or a bad token; `users unlock <email> --yes` frees a locked account.

## 3. Metrics

`GET /v1/system/metrics` (OpenMetrics; a `system`-scope token) feeds Prometheus. The example alerts in the production guide's *Monitoring* section are the ones to start with:

| Metric | Alert when |
|---|---|
| `repomemo_disk_free_bytes` | below twice the disk floor |
| `repomemo_backup_last_timestamp_seconds` | older than a day and a half |
| `repomemo_backup_copy_last_timestamp_seconds` | older than a day and a half, with a second copy set |
| `rate(repomemo_http_server_errors_total[5m])` | above 0 for ten minutes |
| `repomemo_jobs{status="failed"}` | rising |
| `repomemo_indexing_queued` | not falling for an hour |
| `repomemo_maintenance_failed_steps` | above 0 |

## 4. Jobs and maintenance

```bash
repomemo-console jobs list --status failed
repomemo-console jobs list --status running
repomemo-console jobs cancel <job-id> --yes
repomemo-console maintenance status
repomemo-console maintenance run --yes
repomemo-console workspace reindex <workspace> --yes
```

**Expected:** failed jobs are few and have a reason; running jobs finish. A file that fails to index is retried a few times and again every few hours. **Recover:** cancel a job that is stuck; run maintenance when `repomemo.sqlite-wal` is large; re-index a workspace after you add an AI provider.

## 5. Disk

Uploads answer `507 disk_low` when free space is under `REPOMEMO_DISK_FLOOR_MB` (1,024 by default). **Recover:** free space (delete old backups with `backup delete <backup> --yes`, or move the second copy), or lower the floor knowingly. Keep the floor above the size of one backup.

## 6. Backup

```bash
repomemo-console backup create --yes
repomemo-console backup list
```

**Expected:** the backup appears in `backup list` as complete; with `REPOMEMO_BACKUP_COPY_DIR` set, its copy is checked and `sha256sum -c SHA256SUMS` passes in the destination. Automatic backups run every `REPOMEMO_BACKUP_INTERVAL_HOURS`; `REPOMEMO_BACKUP_KEEP` (7) are kept. For containers' volumes: `deploy/backup-volumes.sh` (add `--all` with the stack stopped for the database and files).

**Recover:** a failed copy is retried on the next maintenance pass, or run *Copy to the second location now*. Check that backups happen with the metrics in § 3.

## 7. Restore

A restore never overwrites the running environment.

```bash
repomemo-console backup restore backup-20261014-020000 restored-2026-10 --yes
repomemo-console backup list
```

1. Wait for "Restored … Attach it from the environment menu".
2. Attach the new environment in *System › Settings › Environment*, or set `REPOMEMO_SERVER_DATA_DIR=restored-2026-10` and restart.
3. Sign in; compare *System › Overview* counts with what you expect; run a search.

**Expected:** the restore runs SQLite `integrity_check` before it reports success. **If it fails:** try the second copy, or an older backup; a sealed backup needs `REPOMEMO_BACKUP_KEY` (or `REPOMEMO_BACKUP_KEY_PREVIOUS`). Stored provider keys need the same `REPOMEMO_SECRET_KEY`. For containers' volumes: `deploy/restore-volumes.sh` fills the empty volumes of another stack. Practise once on a spare machine; `npm run backup:drill` runs the generated-fixture drill from a checkout.

## 8. Upgrade

1. `repomemo-console backup create --yes`, then copy the backup off the machine.
2. Read the *Upgrading* part of each [changelog](../CHANGELOG.md) entry you skip.
3. **Containers:** `deploy/upgrade.sh --version X.Y.Z --token-file <file with a system token>` (or `--no-backup` if you made one). It backs up, pulls, restarts and waits until every service is healthy, and puts the old version back if they are not. **Direct install:** [Direct install upgrade](DIRECT_UPGRADE.md).
4. Check `/health` names the new version, `status` is clean, and a search returns results.

**Expected:** the server starts and runs migrations. It refuses to start on a failed or newer-than-itself migration, naming both, and changes nothing.

## 9. Roll back

A migration that already ran is not undone, so the way back is the backup.

1. Stop the new version.
2. Restore the pre-upgrade backup into a new environment (§ 7).
3. Start the previous release against that environment. Containers: set `REPOMEMO_VERSION` to the previous tag and run `docker compose up -d --wait`. Direct install: the old release folder, see [Direct install upgrade](DIRECT_UPGRADE.md).
4. Check `/health` and a search.

## 10. A failed upgrade, in order

| Symptom | Look at | Do |
|---|---|---|
| `upgrade.sh` stops before pulling | its message: no token, no backup | give a `system` token or `--no-backup` |
| A service never becomes healthy; the script restores the old version | `docker compose logs server` | fix the cause in the log (a setting, a folder permission, a refused migration), then upgrade again |
| The server exits at start with a migration message | the message names both versions | do not delete data; roll back (§ 9), then read the changelog entry |
| Started, but sign-in fails for everyone | `REPOMEMO_JWT_SECRET` changed | restore the old value; sessions are signed with it |
| Provider keys "missing" | `REPOMEMO_SECRET_KEY` differs | restore the old key, or enter the keys again |

More symptoms and remedies: [Troubleshooting](TROUBLESHOOTING.md) and the production guide's *When something is wrong*.

## What was verified

The commands above were checked against the product's own command catalog (`mindmap/technical/admin-console.md`), the argument lists of `deploy/upgrade.sh`, `deploy/backup-volumes.sh` and `deploy/restore-volumes.sh`, and the production guide. The container backup, volume restore and health paths were run in the V0.4.71 validation ([record](../mindmap/technical/release-validation-0.4.71.md)); `upgrade.sh` against released images was **not** run (no tag exists), so treat § 8 and § 9 as written-to-work, not shown-to-work.
