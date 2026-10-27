# Deploying RepoMemo: the guide for a new administrator

One page from "I have a machine" to "a team is using a server I can back up and upgrade" (V0.4.62). It chooses between the two ways to
install, says what to size for, walks the first hour, and then backups and upgrades. The details live in the pages it links; this one says
what to do in what order.

## 1. Choose: containers or a direct install

| | **Containers** (preferred) | **Direct install** |
| --- | --- | --- |
| What you need | Docker with Compose v2, `curl`, `tar` | a Linux (systemd) or Windows machine, nothing else |
| What you get | the server, the web client, an optional TLS front door and an optional converter for Office previews, each in its own locked-down container; healthy-checked; one command to start | the server as a service, serving the web client itself, behind a front door you choose |
| How it is installed | [QUICK_START.md](QUICK_START.md): one fetched, checksummed script, then `init.sh` | an archive from the release, unpacked, and its installer script |
| How it is upgraded | `deploy/upgrade.sh --version X.Y.Z` (backup, pull, wait until healthy, the old version back if not) | the new archive's installer; the old release stays for a way back ([DIRECT_UPGRADE.md](DIRECT_UPGRADE.md)) |
| Choose it when | you already run containers, or want the least to maintain | you cannot run containers, or want the server under your own service manager |

If in doubt, choose containers. Both run the same server and keep the same data; a direct install can move to containers later by
restoring a backup.

## 2. Size it

The server is light for a team. As a start:

- **Memory:** 1 GB for the server, plus `REPOMEMO_DOCUMENT_MEMORY_MB` (1,024 by default) for each document being read at once; 64 MB each
  for the web image and the TLS edge; 1 GB for the converter while it makes an Office preview.
- **Disk:** the stored files, plus the database (roughly a tenth of the text), plus the cached previews, plus **each backup's new files**.
  Keep the free-disk floor (`REPOMEMO_DISK_FLOOR_MB`, 1,024 by default) above the size of one backup. A workspace is limited to 20 GB by
  default (`REPOMEMO_WORKSPACE_STORAGE_MB`), and one file to 10 MB (`REPOMEMO_MAX_UPLOAD_MB`).
- **CPU:** two cores serve a team comfortably; indexing and previews are what use more.
- **Network:** one address people can reach, with `https://`. Open 80 and 443 (and nothing else) to people.

The limits that protect the disk and the server, and how to change them: [the production guide](../mindmap/technical/operations.md), § 5.

## 3. The first hour

### With containers

```bash
curl -fsSL https://github.com/repomemo/repomemo/releases/latest/download/quickstart.sh | sh -s -- --dir repomemo
cd repomemo
sh deploy/init.sh --host memo.example.com          # a public name: Let's Encrypt gets the certificate (--no-edge behind your own proxy)
docker compose pull && docker compose up -d --wait
docker compose logs server                         # the first-run setup code
```

Keep a copy of `REPOMEMO_SECRET_KEY` from `.env` somewhere safe: without it the stored AI provider keys and repository tokens must be
entered again. The page [QUICK_START.md](QUICK_START.md) has the careful way to do the first command.

### As a direct install

1. Download the archive for your system and `SHA256SUMS` from the release, and check it (`sha256sum --check --ignore-missing SHA256SUMS`).
2. Unpack it and run its installer: Linux `sudo sh service/install.sh --generate-secrets --start`; Windows, in an administrator PowerShell,
   `.\service\install.ps1 -GenerateSecrets -Start`. The installer makes the user, the folders and the settings file once, and is safe to
   run again.
3. Put a front door in front of it: [FRONT_DOORS.md](FRONT_DOORS.md) (Caddy, nginx, or the server alone on a trusted network) and set
   `REPOMEMO_PUBLIC_URL`, `REPOMEMO_ALLOWED_ORIGIN` and `REPOMEMO_TRUST_PROXY` as it says.

### Then, for both

1. Open the address and finish the **first-run setup**: the setup code is in the server's log, or the value of `REPOMEMO_SETUP_CODE` if you
   set one. You create the first system administrator.
2. Look: *System › Overview* names the version, the free disk and the last backup; `https://<host>/health` answers; a search returns
   results after you add a file.
3. **Make the first backup now** (§ 4) and copy it off the machine, before anyone depends on the server.
4. Optional: mail (for invitations and password resets), AI providers, Office previews (`COMPOSE_PROFILES=previews`), monitoring
   (`COMPOSE_PROFILES=monitoring`). Each is off until you turn it on.
5. For the desktop app: [DESKTOP_FILES.md](DESKTOP_FILES.md) and the section *Hand a team the desktop app* of
   [DEVELOPMENT_COMMANDS.md](DEVELOPMENT_COMMANDS.md).

## 4. Backups

- **What to rely on:** the server's own backups: a consistent snapshot of the database and the stored files, made while it runs. *System ›
  Jobs & maintenance › Back up now*, `repomemo-console backup create --yes`, and automatically every `REPOMEMO_BACKUP_INTERVAL_HOURS`
  (keeping `REPOMEMO_BACKUP_KEEP`, 7 by default).
- **Take them off the machine.** Set `REPOMEMO_BACKUP_COPY_DIR` to another disk or a mounted share and the server copies each backup there
  and checks the copy; or copy the backup folder yourself.
- **Containers also have the volumes' scripts:** `deploy/backup-volumes.sh` archives the backups and the key volume (and with `--all`, the
  database and files, with the stack stopped), and `deploy/restore-volumes.sh` fills the empty volumes of another stack from them.
- **Seal them** with `REPOMEMO_BACKUP_KEY` if the copies leave your control, and keep that key apart from them.
- **Try a restore** once, into a new environment, before you need it: [the production guide](../mindmap/technical/operations.md), § 6.

## 5. Upgrading

1. **Back up** and copy the backup off the machine.
2. Read the *Upgrading* part of each release's entry in [CHANGELOG.md](../CHANGELOG.md) for each version you skip.
3. **Containers:** `deploy/upgrade.sh --version X.Y.Z` with a token of the `system` scope (or `--no-backup` if you have one of your own). It makes a
   backup, pulls the images, restarts and waits until every service is healthy, and puts the old version back if they are not. Pin
   `REPOMEMO_VERSION` to an `X.Y.Z`. **Direct:** [DIRECT_UPGRADE.md](DIRECT_UPGRADE.md).
4. Look: *System › Overview* shows the new version; a search returns results.
5. **Going back:** a migration the new version already ran is not undone; the backup is the way back. The server refuses to start on a
   database written by a newer release, naming both, and changes nothing.

An app and a server must be the same release line: a window of a different line asks to be updated or reloaded.

## 6. When something is wrong

Start with *System › Overview* and *System › Logs*, then `/health/ready`; [the production guide](../mindmap/technical/operations.md), § 10, lists
the usual causes. Images can be checked before you run them: [VERIFYING_IMAGES.md](VERIFYING_IMAGES.md).
