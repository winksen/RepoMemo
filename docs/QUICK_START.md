# Quick start: RepoMemo in containers, from a release

For a machine with Docker (Compose v2), `curl` and `tar`, and nothing else: no compiler, no checkout. You get the released images from the
registry, never a build.

## Before you start

- **A host** that runs Linux containers: Docker Engine with Compose v2 on Linux, or Docker Desktop (Linux engine) on Windows and macOS.
- **`curl`, `tar` and `sh`.** On Windows, run the commands in Git Bash or WSL, or use `deployinit.ps1` (below).
- **A name and open ports for a public install:** a DNS name that points at the host, with ports 80 and 443 open, so Let's Encrypt can issue the
  certificate. Without the edge (`--no-edge`) the web image listens on `127.0.0.1:8080` only, for a proxy of your own.
- **A release to fetch.** The images and the archive come from a published release; see *What has been tested* at the end of this page.

### The images

`docker-compose.yml` pulls these from `REPOMEMO_REGISTRY` (default `ghcr.io/repomemo`), at the tag in `REPOMEMO_VERSION`.

| Image | Runs | When |
|---|---|---|
| `repomemo-server` | The API and the background jobs | always |
| `repomemo-web` | The web client | always |
| `repomemo-edge` | TLS and Let's Encrypt in front | the edge profile; `init.sh` turns it on unless `--no-edge` |
| `repomemo-converter` | Office previews | `COMPOSE_PROFILES=previews` |
| `prom/prometheus` (pinned by digest) | Metrics | `COMPOSE_PROFILES=monitoring` |

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

## First sign-in

The first start has no accounts. The server prints a **setup code** in its log (`docker compose logs server`), unless you set
`REPOMEMO_SETUP_CODE` in `.env` (12 or more letters or digits). Open the address; the setup has seven steps:

1. **Setup code:** paste the code from the log.
2. **System administrator:** name, email, and a password of at least 12 characters. This account runs the server; keep it for people who do.
3. **Check the server:** the page reports what it found and what to fix.
4. **Access** and 5. **Logging:** who may join, and what is recorded and for how long. The defaults are safe; change them later in *System*.
6. **First workspace:** an organization name and, optionally, a workspace name.
7. **Review and finish.**

You land signed in on the new organization. To add people, invite them from the organization (see the [user guide](USER_GUIDE.md#12-for-admins)).
`/health` says `setup_required` until the setup is done. If a step fails, find its sentence in [TROUBLESHOOTING.md](TROUBLESHOOTING.md#the-first-run-setup).

## Afterwards

- **Upgrading:** `deploy/upgrade.sh --version X.Y.Z` makes a backup, pulls the new images and starts them, or puts the old ones back.
- **Backups and restores:** `deploy/backup-volumes.sh` and `deploy/restore-volumes.sh`.
- **A mirror or your own registry:** set `REPOMEMO_REGISTRY` in `.env`.

The options of the script: `--version`, `--dir` (it must be empty or new), `--repo OWNER/NAME`, and `--base-url` for a mirror of the
release files.

## What has been tested

Honest status at V0.4.94. Read the [validation record](../mindmap/technical/release-validation-0.4.71.md) before relying on a path:

- **Run:** fetching and unpacking the Compose archive from a local mirror (a changed byte is rejected), `init.sh --no-edge`, starting from locally built images,
  the first-run setup, upload, indexing, search, backup and export, and restoring the volumes into a second stack. This was on Windows with
  Docker Desktop.
- **Not run yet:** the pull of released images, image verification, the Let's Encrypt edge, `upgrade.sh`, previews and monitoring, and the direct install.
  They need a published release; none has been tagged. They are written to work and are not yet shown to.
