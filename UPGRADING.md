# Upgrading RepoMemo

How to move an existing server from any V0.1.x release (V0.1.40 and later) to **0.2.0**, and what you will notice. [CHANGELOG.md](CHANGELOG.md) has the detail of each release; this page is the checklist.

## Before

1. **Back up.** Back up from System › Jobs & maintenance, or `repomemo-console backup create --yes` (V0.1.75 and later). From an older version, stop the server and copy the whole environment folder (`workspace-data/<name>`, including `secret.key`).
2. **Copy the backup to another machine.** A backup sits in the data folder. If you seal backups (`REPOMEMO_BACKUP_KEY`, V0.2.42 and later), keep that passphrase apart from them: restoring needs it.
3. Note your environment variables. The ones that changed meaning are below.

## The upgrade

1. Stop the service (a graceful stop: SIGTERM, Ctrl-C or a normal service stop).
2. Replace the server binary and the web app (`dist/`).
3. Start it. Database migrations run on their own and the server refuses to start on a failed or newer one, so a failed upgrade leaves your data as it was: put the old binary back.
4. Open System › Overview and check the version, the free disk space and that a search returns results.

Environments from V0.1.40 and later upgrade in place. An older one: open it first with V0.1.40.

## What you must do, by the version you come from

### From before V0.1.64

- People who were added to a single workspace kept an organization membership and so every workspace of it. Review organization members once and remove anyone who should only see one workspace.

### From before V0.1.67

- **Registration is closed by default.** A server that relied on open registration sets `REPOMEMO_ALLOW_REGISTRATION=true`, or people join with invitation links (preferred).
- A server bound beyond loopback over plain HTTP stops at startup until it has `REPOMEMO_PUBLIC_URL=https://…`, `REPOMEMO_TRUST_PROXY=true` or `REPOMEMO_ALLOW_INSECURE_HTTP=true`.
- Scripts that added existing accounts directly to a workspace create an invitation instead; scripts that made System changes with a read-and-write token need a token with the `system` scope.

### From before V0.1.68

- Serve the web app and the API from **one host** behind a reverse proxy (`/v1` and `/health` to the server). Send the header from `dist/content-security-policy.txt` and rebuild the web app with `VITE_REPOMEMO_API_URL` set to your host.

### From before V0.1.69

- Providers outside this machine and OpenRouter need `REPOMEMO_AI_ALLOWED_HOSTS`. Repository links need `REPOMEMO_REPO_ROOTS` (they are off without it). `git config --global` of the service account is no longer read: use `REPOMEMO_GIT_SAFE_DIRECTORIES`.

### From before V0.1.71

- Requests are cut off after two minutes (`REPOMEMO_REQUEST_TIMEOUT_SECONDS`); raise it for very large uploads over slow links. Scripts that matched error *texts* use the status code and `error.code`.

### From before V0.1.72

- Search hides outdated and superseded evidence by default. Ask for it with the evidence state filter.

### From before V0.1.74

- "Open in Office" links last one minute and belong to one client; links issued before the upgrade stop working.
- On Windows, `secret.key` is readable only by the account that runs the server. Give a backup account access explicitly, or keep the key in `REPOMEMO_SECRET_KEY`.
- Searches and reports are limited to 240 a minute per person, and background AI to 2,000 calls per workspace per day (`REPOMEMO_HEAVY_REQUESTS_PER_MINUTE`, `REPOMEMO_BACKGROUND_AI_PER_DAY`).

### From before V0.1.75

- Uploads are refused beyond 20 GiB per workspace and while less than 1 GiB of disk is free (`REPOMEMO_WORKSPACE_STORAGE_MB`, `REPOMEMO_DISK_FLOOR_MB`); there are also 600 uploads an hour and 25 owned workspaces per person. All are settings, `0` turns one off.
- Turn on automatic backups (`REPOMEMO_BACKUP_INTERVAL_HOURS`, for example `24`), and read the [production guide](mindmap/technical/operations.md).

## Rolling back

Stop, restore the backup into a new environment (`backup restore <backup> <new-name> --yes` on the new version, or copy the folder you saved by hand), point `REPOMEMO_SERVER_DATA_DIR` at it and start the old binary. Data written after the backup is not in it.
