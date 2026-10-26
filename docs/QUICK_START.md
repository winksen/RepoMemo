# Quick start: RepoMemo in containers, from a release

For a machine with Docker (Compose v2), `curl` and `tar`, and nothing else: no compiler, no checkout. You get the released images from the
registry, never a build.

## The command

```bash
curl -fsSL https://github.com/repomemo/repomemo/releases/latest/download/quickstart.sh | sh -s -- --dir repomemo
```

It finds the latest release, downloads that release's `SHA256SUMS` and `repomemo-compose-<version>.tar.gz`, checks the archive against
the sums, and only then unpacks it (a failed check unpacks nothing). It starts nothing and writes nothing outside the folder. The folder
holds the compose file, `.env.example` (its `REPOMEMO_VERSION` set to the release you fetched, so the install stays on it), the init and
upgrade scripts and the backup scripts.

To read the script before it runs, and to pin a release rather than take the latest:

```bash
curl -fsSLO https://github.com/repomemo/repomemo/releases/download/v0.4.41/quickstart.sh
curl -fsSLO https://github.com/repomemo/repomemo/releases/download/v0.4.41/SHA256SUMS
sha256sum --check --ignore-missing SHA256SUMS      # shasum -a 256 on macOS
less quickstart.sh
sh quickstart.sh --version 0.4.41 --dir repomemo
```

The checksum file comes from the same place as the files, so it proves the download is whole, not who made it. For that, verify the
images (`docs/VERIFYING_IMAGES.md`).

## Then the init

```bash
cd repomemo
sh deploy/init.sh --host memo.example.com        # a public name: Let's Encrypt gets the certificate
# or: sh deploy/init.sh --no-edge                # behind a proxy of your own
docker compose pull && docker compose up -d --wait
docker compose logs server                       # the first-run setup code
```

`init.sh` makes `.env` with strong secrets (readable by you alone); keep a copy of `REPOMEMO_SECRET_KEY` somewhere safe. Open
`https://memo.example.com` (or `http://127.0.0.1:8080` without the edge) and finish the first-run setup. On Windows,
`deploy\init.ps1` does the same from PowerShell; fetch the archive by hand from the release page and unpack it.

## Afterwards

- **Upgrading:** `deploy/upgrade.sh --version X.Y.Z` makes a backup, pulls the new images and starts them, or puts the old ones back.
- **Backups and restores:** `deploy/backup-volumes.sh` and `deploy/restore-volumes.sh`.
- **A mirror or your own registry:** set `REPOMEMO_REGISTRY` in `.env`.

The options of the script: `--version`, `--dir` (it must be empty or new), `--repo OWNER/NAME`, and `--base-url` for a mirror of the
release files.
