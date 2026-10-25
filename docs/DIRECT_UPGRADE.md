# Upgrading and rolling back a direct install

For a server installed from the archive ([operations guide](../mindmap/technical/operations.md), the installers in the archive's
`service/` folder). Containers have their own script, `deploy/upgrade.sh`.

**Always begin with a backup.** A release can change the database in a way an older release cannot read, so the backup is the way back
across an upgrade, and the only one.

## Upgrading

1. **Back up.** In the web app: System > Backups > Make a backup, and wait for it to finish; copy the backup folder
   (`<data>/workspace-data/main/backups/<name>`) to another machine. If you seal backups (`REPOMEMO_BACKUP_KEY`), keep that key apart
   from them. Or, from a shell: `curl -X POST -H "Authorization: Bearer $TOKEN" http://127.0.0.1:3020/v1/system/backups` with a token of
   the `system` scope.
2. **Download the new archive** and its `SHA256SUMS` from the release page, and check it:
   `sha256sum --check --ignore-missing SHA256SUMS`.
3. **Unpack it** in a new folder (not over the old one) and run its installer.
   - Linux: `sudo sh service/install.sh --start` (it makes the new release folder `/opt/repomemo-<version>`, points `/opt/repomemo` at it
     and restarts the service; your settings in `/etc/repomemo/repomemo.env` are not touched).
   - Windows, administrator PowerShell: `.\service\install.ps1 -Start` (the same with `C:\Program Files\RepoMemo\repomemo-<version>` and
     the junction `current`).
4. **Look.** `curl https://memo.example.com/health` names the version; System > Overview shows it too; sign in and open a workspace.
   The logs: `journalctl -u repomemo -n 50` (Linux), the files under `workspace-data\main\logs` (Windows).

Old releases stay next to the new one (`/opt/repomemo-<old>`), so the way back is a link, not a download.

## Rolling back

If the new release is wrong and you want the old one:

1. **Stop** the service: `sudo systemctl stop repomemo` (Windows: `Stop-Service RepoMemo`).
2. **Point the link at the old release.** Linux: `sudo ln -sfn /opt/repomemo-<old> /opt/repomemo`. Windows:
   `cmd /c rmdir "C:\Program Files\RepoMemo\current"; New-Item -ItemType Junction -Path "C:\Program Files\RepoMemo\current" -Target "C:\Program Files\RepoMemo\repomemo-<old>"`.
3. **Start** it. Two outcomes:
   - It starts: the new release made no change the old one cannot read (a patch release usually does not). Done.
   - **It refuses**, with a message like *this database was written by RepoMemo 0.4.36, which is newer than this program (0.4.20). Nothing
     was changed.* The new release changed the database. **Restore the backup you made in step 1 of the upgrade:** stop the service, move
     the data folder aside (`workspace-data` -> `workspace-data.new`), restore the backup into a new environment with the old release
     (System > Backups > Restore, or `workspace-data/main` replaced by the backup's contents), then start. What was written after the
     backup, in the new release, is in the folder you moved aside and is not in the restored copy.

That refusal is deliberate: an older program that opened a database it does not understand could damage it, so it stops first and
changes nothing. The check reads the file `repomemo.version` beside the database (the release that last opened it) and the migrations
the database has applied.

## What not to do

- Do not unpack the new archive over the running one.
- Do not delete `repomemo.version` to get past the refusal. Restore the backup instead.
- Do not run two releases on the same data folder.
