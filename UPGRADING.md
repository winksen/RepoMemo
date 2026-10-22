# Upgrading RepoMemo

How to move an existing server from any V0.1.x release (V0.1.40 and later) to **0.2.0**, and from there through the 0.2.x releases to **0.3.0**, and what you will notice. [CHANGELOG.md](CHANGELOG.md) has the detail of each release; this page is the checklist.

## Before

1. **Back up.** Back up from System › Jobs & maintenance, or `repomemo-console backup create --yes` (V0.1.75 and later). From an older version, stop the server and copy the whole environment folder (`workspace-data/<name>`, including `secret.key`).
2. **Copy the backup to another machine.** A backup sits in the data folder. If you seal backups (`REPOMEMO_BACKUP_KEY`, V0.2.42 and later), keep that passphrase apart from them: restoring needs it.
3. Note your environment variables. The ones that changed meaning are below.

## The upgrade

1. Stop the service (a graceful stop: SIGTERM, Ctrl-C or a normal service stop).
2. Replace the server binary and the web app (`dist/`).
3. Start it (an environment made by 0.2.0 is opened in place; this is tested in CI). Database migrations run on their own and the server refuses to start on a failed or newer one, so a failed upgrade leaves your data as it was: put the old binary back.
4. Open System › Overview and check the version, the free disk space and that a search returns results.

Environments from V0.1.40 and later upgrade in place. An older one: open it first with V0.1.40.

**Running in containers (V0.2.45 and later):** back up, fetch the new release and run `docker compose up -d --build`. The data volume is kept and migrations run at start; the [production guide](mindmap/technical/operations.md) has the rest.

## What you must do, by the version you come from

### From before V0.1.64

- People who were added to a single workspace kept an organization membership and so every workspace of it. Review organization members once and remove anyone who should only see one workspace.

### From before V0.1.67

- **Registration is closed by default.** A server that relied on open registration sets `REPOMEMO_ALLOW_REGISTRATION=true`, or people join with invitation links (preferred).
- A server bound beyond loopback over plain HTTP stops at startup until it has `REPOMEMO_PUBLIC_URL=https://…`, `REPOMEMO_TRUST_PROXY=true` or `REPOMEMO_ALLOW_INSECURE_HTTP=true`.
- **New addresses and names are stricter (V0.2.146).** An address given for a new account, invitation or address change must have one `@` and no white space, control or text-direction characters, and display, workspace and organization names must not contain control or text-direction characters. Existing accounts and names are untouched and still sign in. Local git commands no longer see the server's environment variables (only `PATH`, the home and temporary folders and the language settings): anything git needs for a local repository belongs in the repository's own configuration or `REPOMEMO_GIT_SAFE_DIRECTORIES`.
- **Uploads are received into a temporary file, and the bytes arriving at once are capped (V0.2.153).** An upload or a new version is written to `<data dir>/tmp/` while it is hashed and moved into the blob store, so the server holds a chunk of it in memory and not the file; the folder is emptied at start. A new setting, `REPOMEMO_UPLOAD_INFLIGHT_MB` (512), caps the uploads arriving at once, all senders together: past it an upload answers `503` with `Retry-After`, which a client retries. A file above `REPOMEMO_MAX_UPLOAD_MB` is refused with a JSON `413` (`payload_too_large`) as soon as its announced size or the bytes that arrived show it. Nothing to do unless a proxy or script gives up on a `503`.
- **Request bodies other than files stop at 1 MiB (V0.2.152).** `REPOMEMO_MAX_UPLOAD_MB` now limits only an upload and a new file version (a workspace import has `REPOMEMO_MAX_IMPORT_MB`); every other call (a comment, a note, a search, a setting) answers `413` above 1 MiB, whatever that setting is. A script that posts a note of more than 1 MiB as JSON uploads it as a file instead.
- **A job is cancelled by whoever started it, or an administrator (V0.2.151, migration 0040).** Before, any workspace writer could cancel any job of the workspace, an administrator's whole-workspace re-index included. A job now remembers its starter (new column, empty for earlier jobs); jobs that nobody started (a webhook or scheduled sync, a restart) and jobs of earlier versions are stopped by an owner or administrator only. A re-index of one file now goes through the indexing queue and asking again while it waits returns the same job.
- **The server container is locked down, and LibreOffice has a container of its own (V0.2.148).** The compose file now runs the server with a read-only root, no capabilities and no way to gain any, and the `REPOMEMO_WITH_LIBREOFFICE` build option is gone from it. Layout-accurate previews use the `converter` container (no network, no view of the data volume): set `COMPOSE_PROFILES=previews` and `REPOMEMO_OFFICE_PREVIEWS=true` in `.env`, then `docker compose up -d --build`. A server that put LibreOffice in its image keeps no previews until then. If you changed the compose file (extra mounts, a different user), keep the server writing only to its volumes and `/tmp`. Debian 13's LibreOffice may be older than the releases the server accepts; System › Overview says so.
- **Sign-in limits count an IPv6 client by its /64 (V0.2.145).** Everyone behind one IPv6 prefix now shares one budget for sign-ins, registrations and lockouts, as a whole prefix belongs to one site; IPv4 is unchanged. Password checks are capped at twice the cores at a time and answer `429` with `Retry-After` when the server is saturated.
- **Two tabs renewing together no longer sign a person out everywhere (V0.2.144, migration 0039), and the refresh cookie is renamed over HTTPS.** A token rotated less than 30 seconds ago and presented again is refused with `409` instead of ending every session of the account; reuse of an older one still does. The web app renews one tab at a time and asks again after a `409`. Over HTTPS (a `REPOMEMO_PUBLIC_URL` that starts with `https://`) the cookie is now `__Secure-repomemo_refresh`: a browser that holds the old one is read once, signed in and given the new name, so nobody is signed out. A script or proxy rule that names the cookie, or reads `409` from `POST /v1/auth/refresh` as an error, needs the new name or a retry.
- **Only system administrators create organizations and link local repositories, until you say otherwise (V0.2.143).** Before, any verified account could create an organization and become its owner, and any workspace administrator could link any folder under `REPOMEMO_REPO_ROOTS`, other teams' repositories included. Two System settings now decide (System › Settings › Accounts): *Who can create organizations* (system administrators by default, or every verified account) and *Who can link a local repository* (system administrators by default, or workspace administrators). Existing organizations, workspaces and links are untouched. A server where people are meant to start their own teams, or where workspace administrators link folders, sets the matching one after upgrading. Remote repositories are not affected, and the setup wizard asks both questions on a new server.
- **An organization role change no longer undoes a workspace's own choices (V0.2.142, migration 0038).** Before, changing someone's organization role wrote that role into every workspace of the organization, bringing back a person a workspace owner had removed and raising a viewer. Now it reaches only the workspaces whose access came from the organization. Roles that exist today stay: a role that matches the organization's is treated as coming from it, any other as the workspace's own. To make a workspace follow the organization again, remove the person from the workspace and change their organization role once more, or leave and rejoin the organization.
- **The live stream shows the activity feed to owners and administrators only (V0.2.141).** Other readers still get a `changed` event for every change, so their pages refresh as before, but it names only the kind of change: no actor, no summary, no time. A script that read the `activity` events of a workspace as a member now needs an owner or administrator account.
- **A read-and-write token no longer grants access or removes things (V0.2.140).** Inviting people, changing roles, adding or removing members, deleting or renaming a workspace or organization, changing AI providers or policy, linking repositories and connectors (with their tokens, deploy keys and webhooks), and exporting or importing a workspace need a browser session or a token with the new `admin` scope (Profile › API tokens; a short-lived one, for a script that needs it). Existing tokens keep their scope and lose these routes: a script that calls them gets `403` and the message names the scope. Content work (files, notes, tasks, comments, memory, searches) is unchanged, and so are `system` tokens for the System area. Revoking a token now ends the live streams it opened.
- Scripts that added existing accounts directly to a workspace create an invitation instead; scripts that made System changes with a read-and-write token need a token with the `system` scope.

### From before V0.1.68

- Serve the web app and the API from **one host** behind a reverse proxy (`/v1` and `/health` to the server). Send the header from `dist/content-security-policy.txt` and rebuild the web app with `VITE_REPOMEMO_API_URL` set to your host.

### From before V0.1.69

- Providers outside this machine and OpenRouter need `REPOMEMO_AI_ALLOWED_HOSTS`. From V0.2.139 the built-in list names Ollama's port (11434) and OpenRouter's (443) and allows no path beyond the provider's own: a provider saved with another port on this machine, or with a path such as `/api`, is refused when it is next saved or used. List the host (without a port to allow any, or with `:port`) in `REPOMEMO_AI_ALLOWED_HOSTS` to keep it. Repository links need `REPOMEMO_REPO_ROOTS` (they are off without it). `git config --global` of the service account is no longer read: use `REPOMEMO_GIT_SAFE_DIRECTORIES`.

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

### From 0.2.0 to 0.3.0 (the 0.2.x releases)

Every 0.2.x release upgrades in place from the one before it, and an environment made by 0.2.0 is opened by the current code (a test in CI does exactly that, and a lock keeps every released database migration from being edited). Nothing below is needed to start the server; it is what to set, change or know to use what the line added.

**New things that stay off until you turn them on**
- **Mail** (V0.2.9): System › Settings › Mail, over TLS, and `REPOMEMO_PUBLIC_URL` so links can be put in messages. A mail server on a private address needs `REPOMEMO_SMTP_ALLOWED_HOSTS`. It switches on invitations by mail, "Forgot your password?", mentions, assignments, verification and the daily digest. Since V0.2.52 one address is sent at most 12 messages an hour.
- **Remote repositories** (V0.2.19–V0.2.23) and **trackers** (GitHub and GitLab issues and pull requests, V0.2.27–V0.2.31): name their hosts in `REPOMEMO_GIT_ALLOWED_HOSTS` (both are off without it). git must be 2.50.1 or later (or a patched older release); `ssh` links need an `ssh` client on the `PATH`. Clones have a size and a time cap (`REPOMEMO_REPO_CLONE_MAX_MB`, `REPOMEMO_REPO_CLONE_TIMEOUT_MINUTES`) and count in a workspace's storage, so a workspace with large repositories may reach its allowance. Webhooks need the server to be reachable from the host; send a unique delivery header from your own sender (V0.2.52).
- **Encrypted backups** (V0.2.42): set `REPOMEMO_BACKUP_KEY` and keep the passphrase apart from the backups; without it a sealed backup cannot be restored. A **second copy** of each backup (V0.2.43): `REPOMEMO_BACKUP_COPY_DIR`.
- **Containers** (V0.2.45): `docker compose up`, with the data in a volume and Caddy in front; see the production guide.

**Behaviour that changed**
- **Layout-accurate Office previews are off until you turn them on** (V0.2.137). A server that has LibreOffice installed showed Word, Excel and PowerPoint files as PDF; after this release it shows extracted text and tables until `REPOMEMO_OFFICE_PREVIEWS=true` is set and the server restarted. It also needs LibreOffice 26.2.5 or later (26.8.0 or later on the next line): an older release is refused, and System › Overview and the server check say which and why. In the containers, see V0.2.148 below: LibreOffice runs in a container of its own.
- The two re-index endpoints answer `202` with a running job (V0.2.6): scripts that read the job in the answer poll `GET /v1/jobs/{id}` until it ends.
- Registering on an open server with mail set up answers `202` with no session and mails the owner of a known address (V0.2.18): a script signs in afterwards.
- Document files (Word, Excel, PowerPoint, PDF, OneNote, email) are read in a short-lived process of their own with a time and a memory limit (V0.2.44, `REPOMEMO_DOCUMENT_TIMEOUT_SECONDS`, `REPOMEMO_DOCUMENT_MEMORY_MB`). If readings fail with "could not be started", the server cannot start its own program file: check where it runs from, or set `REPOMEMO_DOCUMENT_READER=thread`.
- The anonymous endpoints (sign-in, registration, refresh, reset, invitations, setup) take bodies of 64 KiB, and the webhook receivers 1 MiB (V0.2.52). An import needs a proxy that lets `POST /v1/workspaces/import` carry a body as large as your biggest archive, unbuffered.
- Deactivating (V0.2.16) and anonymizing (V0.2.17) accounts exist. Log files and earlier backups keep a person's address until they age out.
- The desktop app is a window onto a server (V0.2.49); its local workspace is retired and goes in a 0.3.x release. Move any you need with *Move a workspace to a server* (V0.2.50). The server needs nothing.

**If you build the server yourself:** build from a checkout with LF line endings, as CI does: a build with other line endings computes other migration checksums and refuses an environment another build made. The desktop crate pulls `tauri-plugin-http` and `keyring`.

**A short path:** back up, replace the binary and the web app, start it, open System › Overview. Then, if you want any of the above, set it and restart. Rolling back is below; an environment already migrated by a newer release is refused by an older binary, so roll back with a backup.

## Rolling back

Stop, restore the backup into a new environment (`backup restore <backup> <new-name> --yes` on the new version, or copy the folder you saved by hand), point `REPOMEMO_SERVER_DATA_DIR` at it and start the old binary. Data written after the backup is not in it.
