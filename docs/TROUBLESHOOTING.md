# Troubleshooting and FAQ

What the server and its checks say when something is wrong, with the cause and the remedy of each (V0.4.64). Start with the three checks
below; then find the sentence you saw. Every error a start and the first-run setup can raise is listed here, and CI fails if a new one is
added without a row (`npm run troubleshooting:check`). For limits and day-to-day symptoms (uploads refused, slow searches) see
[the production guide](../mindmap/technical/operations.md), § 10.

## The three checks

| Check | What it tells you |
| --- | --- |
| `repomemo-server --check-config` | Reads the environment exactly as a start would and says whether it can start, **without serving**. Exit `0`: it can; `1`: a setting is invalid (the first error is named); `2`: a folder is wrong. In a container: `docker compose run --rm server repomemo-server --check-config`. |
| `GET /health` and `GET /health/ready` | `/health` answers 200 while the process is alive, names the version, commit and image, and says `setup_required` until the first-run setup is done. `/health/ready` answers 200 when the database answers and 503 when it does not (use it for probes). |
| *System › Overview* and *System › Logs* | The version, uptime, free disk, the last backup, queues and traffic; the logs carry a category per kind of event (`security`, `jobs`, `http`, `ai`, `performance`). In containers: `docker compose ps` (every service should be healthy) and `docker compose logs server`. |

Every line of the server's log is JSON when `REPOMEMO_LOG_FORMAT=json` (the compose files set it); audit events carry `"target":"audit"`.

## The server will not start: settings

Printed as the first line of the log and on standard error; `--check-config` prints the same. Fix the setting named and start again.

| You see | Cause | Remedy |
| --- | --- | --- |
| `REPOMEMO_JWT_SECRET must contain at least 32 characters` | The secret is empty or short. | Set 32 or more random characters (`openssl rand -hex 32`); `deploy/init.sh` does this. |
| `REPOMEMO_JWT_SECRET is too repetitive; use at least 32 random characters` | The secret is long but is one character or a pattern repeated. | Use random characters. |
| `REPOMEMO_SECRET_KEY must contain at least` | The key that seals stored provider keys and tokens is empty or short. | Set 32 or more random characters, and keep a copy: without it the stored keys must be entered again. |
| `REPOMEMO_SETUP_CODE must contain at least 12 letters or digits` | A setup code of your own is too short. | Use 12 or more letters or digits, or leave it unset to have one printed in the log. |
| `_FILE are both set; give the secret one way only` | A secret is given both directly and through its `NAME_FILE`. | Keep one. |
| `) could not be read` | A `NAME_FILE` secret (or another named file) cannot be read. | Check the path inside the container, the mount and the file's owner (the server runs as user `10001`). |
| `REPOMEMO_SMTP_CA_FILE (` … `could not be read` | The certificate file for the mail server cannot be read. | Fix the path or its permissions, or unset it. |
| `) has no index.html; build the web app (npm run build) and give the dist folder` | `REPOMEMO_WEB_DIR` names a folder that is not the built web app. | Point it at the `web` folder of the unpacked archive (or `apps/desktop/dist`). |
| `) is reachable from other machines over plain HTTP. Put a TLS reverse proxy in front and set REPOMEMO_PUBLIC_URL=https://… (or REPOMEMO_TRUST_PROXY=true), or set REPOMEMO_ALLOW_INSECURE_HTTP=true for a trusted network only.` | The server listens on all interfaces without TLS. | Put a front door in front ([FRONT_DOORS.md](FRONT_DOORS.md)); or, on a network you trust, allow plain HTTP. |
| `REPOMEMO_PUBLIC_URL must start with https:// (or http://)` | The public address is not a web address. | Give `https://memo.example.com`. |
| `REPOMEMO_ALLOWED_ORIGIN must list origins; "*" is not allowed` | `*` was given. | List the web app's origins, comma separated. |
| `REPOMEMO_ALLOWED_ORIGIN has an invalid origin:` | One of them is not `scheme://host[:port]`. | Fix it (no path, no trailing slash). |
| `REPOMEMO_TRUSTED_PROXIES has an invalid address:` | An entry is not an IP address or a range. | Fix it. |
| `REPOMEMO_COOKIE_SAMESITE must be strict, lax or none, not` | The value is something else. | Use `strict`, `lax` or `none`. |
| `REPOMEMO_COOKIE_SAMESITE=none needs REPOMEMO_PUBLIC_URL to be an https:// address, since a browser takes SameSite=None only with Secure` | A cross-origin cookie needs https. | Give an `https://` public address. |
| `REPOMEMO_PRODUCT_NAME must be one line of 60 characters or fewer, with no quote or backslash` | The name shown in the browser's tab does not fit. | Shorten it. |
| `REPOMEMO_DOCUMENT_READER must be "process" or "thread", not "` | A wrong value. | Use `process` (the default) or `thread`. |
| `REPOMEMO_DB_DIR, REPOMEMO_BLOB_DIR and REPOMEMO_BACKUP_DIR belong to one environment, so REPOMEMO_SERVER_DATA_DIR must be set too` | A split data folder is named without the environment it belongs to. | Set `REPOMEMO_SERVER_DATA_DIR` as well. |
| `must be an absolute path, not` | A folder setting is relative. | Give a full path. |
| `must be true or false, not` | A yes/no setting has another value. | Use `true` or `false`. |
| `has an invalid value:` | A setting has a value of the wrong kind (a number that is not one). | Read the setting's description in [DEVELOPMENT_COMMANDS.md](DEVELOPMENT_COMMANDS.md). |
| `REPOMEMO_BACKUP_KEY_PREVIOUS must contain at least` | The old backup key is too short to be one. | Give the key as it was, or leave it unset. |
| `REPOMEMO_BACKUP_KEY_PREVIOUS is the key before a change, so REPOMEMO_BACKUP_KEY must be set to the new one` | The old key is set but the new one is not. | Set `REPOMEMO_BACKUP_KEY` to the new key. |
| `REPOMEMO_BACKUP_KEY_PREVIOUS is the same as REPOMEMO_BACKUP_KEY; leave it unset until the key changes` | Both keys are the same. | Unset the previous key. |

## The server will not start: folders

The check of the folders runs before anything is opened (V0.4.5). The first line names the folder.

| You see | Cause | Remedy |
| --- | --- | --- |
| `is not a folder that exists` | A folder the check expects is missing. | Create it, or fix the setting. |
| `exists but is not a folder` | A file stands where the folder should be. | Move it. |
| `does not exist and cannot be created there` | The parent is missing or read-only. | Create the parent, or mount the volume. |
| `Give the server's account write access to it, or mount the volume read-write.` (after `is not writable`) | A read-only mount, or a volume that belongs to another account. | Give user `10001` (containers) or the service account write access; the message names the owner and the account the server runs as. |
| `MiB free. Free some room or mount a larger volume.` (after `is full`) | Under 16 MiB free: the start is refused. | Free space or use a larger volume. |
| `MiB where uploads are refused (REPOMEMO_DISK_FLOOR_MB).` | A warning: under the disk floor, so uploads are refused. | Free space, or lower the floor. |
| `). SQLite's write-ahead log needs shared memory and locks such file systems do not give, so a database kept there can be corrupted; use a local volume for the database.` (after `is on a network file system`) | A warning: the database is on NFS, SMB, 9p or sshfs. | Keep the database on a local volume; the files and backups may be elsewhere. |

## The server will not start: the database

| You see | Cause | Remedy |
| --- | --- | --- |
| `) does not; it was written by a newer RepoMemo. Nothing was changed. Start the newer release, or restore a backup made before the upgrade.` | The database has a migration this program does not know. | Start the release that wrote it; to go back, restore a backup made before the upgrade into a new environment. |
| `or later, or restore a backup made before the upgrade (the upgrade guide, docs/DIRECT_UPGRADE.md).` (after `this database was written by RepoMemo`) | The marker beside the database names a newer release than this program. | The same: nothing was changed. See [DIRECT_UPGRADE.md](DIRECT_UPGRADE.md). |
| A failed migration | A migration did not apply. | Restore the backup you made before the upgrade ([DEPLOYMENT.md](DEPLOYMENT.md), § 5) and report it with the log. |

## The first-run setup

The setup page answers with these sentences.

| You see | Cause | Remedy |
| --- | --- | --- |
| `The setup code is not correct. It is printed on the server console when the server starts.` | The code typed is not the server's. | Read it from `docker compose logs server`, or use `REPOMEMO_SETUP_CODE` if you set one. Too many wrong tries are slowed down; wait and try again. |
| `This server already has its system administrator.` | Someone finished the first step. | Sign in; ask that person for an account or a reset link. |
| `Create the system administrator first.` | The second step was tried before the first. | Go back to the first step. |
| `Only the system administrator can finish setting up the server.` | The signed-in account is not the one created in setup. | Sign in as that administrator. |
| `This server is already set up.` | Setup is complete. | Use *System* instead of the setup pages. |
| `The onboarding is turned off on this server.` | The server runs with `REPOMEMO_SETUP_WIZARD=false`. | With it off there is no setup: the first account to register becomes a system administrator. Register it, or set the variable to `true` (its default) on a new server. |

## Questions

**Nobody can sign in after a restart.** `REPOMEMO_JWT_SECRET` changed; sessions are signed with it. Restore the old value.

**The AI provider keys say "missing" after a restore or a move.** `REPOMEMO_SECRET_KEY` differs, or `secret.key` was not carried over.
Enter the keys again.

**Which version is running?** `GET /health`, *System › Overview*, or `repomemo-server --version`. A desktop app of another release line says
so and asks to be updated.

**The desktop app shows a configuration error and nothing else.** Its `repomemo-desktop.json` is wrong in some way; the page names the file
and the reason, and nothing in the file is used until it is fixed ([DESKTOP_ADMIN.md](DESKTOP_ADMIN.md), [DESKTOP_FILES.md](DESKTOP_FILES.md)).

**A server behind our own certificate authority does not connect from the app.** Give the app the authority's certificate
(`desktop:package --certificate`); a browser needs it in its own store.

**How do I check an image or an archive I downloaded?** [VERIFYING_IMAGES.md](VERIFYING_IMAGES.md) and the release's `SHA256SUMS`.

**How do I get back to the previous version?** Restore the backup made before the upgrade into a new environment ([DEPLOYMENT.md](DEPLOYMENT.md), § 5);
a migration that already ran is not undone.
