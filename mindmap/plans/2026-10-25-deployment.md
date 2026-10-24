# Preparation for deployment, on the server and on the client: the plan for the 0.4.x line

> **Status:** proposed on 2026-10-25 and opened by `V0.4.0`. The maintainer chose the theme (decision 33, replacing the earlier proposal of single sign-on). Decisions 34–40 below are proposals to confirm; nothing past `V0.4.0` is built.
> **What it asks for:** the repository can **deploy RepoMemo directly on a server or in containers**, containers being the preferred path and the one the tests focus on, with **separate containers for the API** (Rust, with its database and file storage) **and for the web client** (React); and the **desktop app resumes** as the web client's own UI and components, talking to the API directly like the web client does, installable, an administrator being able to send a team an executable that already carries the address of their server, **Windows first**, then Linux and macOS.
> **Releases:** 67 single-purpose releases in eight tracks, [RELEASES_0.4.md](../RELEASES_0.4.md); the tables are also in [the roadmap](../ROADMAP.md#-the-04x-line-preparation-for-deployment). **Set aside:** [the single-sign-on plan](2026-10-24-single-sign-on.md), kept as the candidate for a later line.
> **Related:** [operations guide](../technical/operations.md) · [clean-install record](../technical/clean-install.md) · [seventh security review](../security/2026-10-24-0.3-security-review.md)

## 1. Where deployment stands at V0.3.27

| Part | Today | What is missing for the theme |
|---|---|---|
| The API | `repomemo-server`: one Rust process, SQLite in WAL mode, stored files and backups in one data folder (`REPOMEMO_SERVER_DATA_DIR`), configured by about 80 environment variables ([DEVELOPMENT_COMMANDS.md](../../docs/DEVELOPMENT_COMMANDS.md)); `/health` and `/health/ready`; metrics at `/v1/system/metrics` | Secrets only as plain variables; no way to check a configuration without starting; no build information in `/health`; the folder cannot be split across volumes; no clean-stop guarantee that is tested |
| The image | A `Dockerfile` with stages `web`, `build`, `server`, `converter` (LibreOffice, no network) and `proxy`; every base pinned by digest; the server unprivileged (uid 10001) with a read-only root, no capabilities and a health check by `curl`; built natively for amd64 and arm64 in CI (V0.3.23) | Nothing is **published** (no registry, no tags); the build is not cached; the image carries `curl` and `socat`; no labels; a root-owned volume is not handled; no scan |
| The web client | The Vite build, served by Caddy in the `proxy` stage together with a reverse proxy to the API and TLS; the API address is a **build-time** variable (empty: same origin), and the content security policy is written at build time from it | The web client cannot be its own image with a runtime address; cross-origin use (web and API on two addresses) is not a supported shape; the server cannot serve the web itself |
| Compose | `docker-compose.yml`: `server`, optional `converter`, `proxy`; one `repomemo-data` volume; secrets from `.env`; one CI job that brings it up and asks `/health/ready` (V0.3.23) | No second compose shape; one volume for everything; no scripted setup, upgrade or backup; CI never uses the stack, never upgrades it, never restores a backup in it |
| A direct install | A binary a person builds with `cargo build --release` and an environment they write | No release archive, no service file, no Windows service, no binaries built by CI |
| The desktop app | Tauri: `App.tsx` chooses `SharedWebApp` in a browser and `DesktopApp` in the window; the same React bundle, the same components; a connect screen asks for the server address, which `connection.rs` checks, keeps in `server.json` and allows the window to call; the refresh token is in the credential store; links open in the system browser | No build outside the maintainer's machine (the scripts say `npm.cmd`), no installer for anyone else, no preset address, no file an administrator can send, a download is a blob link with no native save, no notification, deep link or update |

## 2. The shape this line aims at

```
                      browsers                  desktop app (Windows, then Linux and macOS)
                         |                                     |
                         |   one built web client (React)      |   the same bundle in a window,
                         v                                     |   the server address preset
   +---------------- optional edge (Caddy: TLS) ---------------+   or read from a file
                         |                                     |
              +----------+-----------+                         |
              |  repomemo-web        |  static app, /config.json at start,
              |  (plain HTTP)        |  forwards /v1 and /health to the API (same-origin mode)
              +----------+-----------+                         |
                         | /v1  (or the API's own address, cross-origin mode) <----+
                         v
              +----------------------+       volumes: database | files | backups
              |  repomemo-api        |------ (one or three mounts)
              |  Rust + SQLite       |
              +----------+-----------+
                         | (optional, no network)
              +----------+-----------+
              |  repomemo-converter  |   LibreOffice previews
              +----------------------+
```

Three images, published for amd64 and arm64: **`repomemo-api`** (the server, its database and files), **`repomemo-web`** (the client and nothing else) and the optional **`repomemo-converter`**. TLS is a separate, optional piece (`edge`) so that the two main images speak plain HTTP and fit behind any front door. The same server binary is also released as an **archive** for a plain Linux or Windows server, and it can serve the web client itself for a single-process install.

## 3. Decisions to confirm

| # | Decision | Proposal |
|---|---|---|
| 33 | **The theme of V0.4.0** | **Preparation for deployment**, the server and the client, containers first. Single sign-on stays the candidate for the line after |
| 34 | **Numbering** | `V0.4.0` opens the line (this release: the plan, the index, the version). `V0.4.1`–`V0.4.67` are built one by one, committed as `Update V0.4.N`. When the last is done and the maintainer's tests of a clean container install pass, the next minor is cut, as `V0.3.0` was cut after 0.2.x |
| 35 | **Containers are the supported path** | A container install is what is documented first, tested in CI and recorded in the clean-install page. A direct install is supported with a release archive and a guide, tested less (its unit files and the archive are checked, the machines are not) |
| 36 | **Three images, two main** | `repomemo-api` and `repomemo-web` are separate: each builds, starts, is configured and is published on its own. The converter stays optional. TLS is not in either: an `edge` profile adds it |
| 37 | **Same origin by default, cross-origin supported** | By default the web image forwards `/v1` to the API, so one address serves both and the session cookie stays simple. The web on one address and the API on another is supported through runtime configuration and a stated cookie and origin policy, and gets its own security pass |
| 38 | **Registry and tags** | GitHub's container registry, `X.Y.Z`, `X.Y` and `latest` for each image, multi-architecture, pushed only by a tag build, with an SBOM and an attestation. Anyone who forks the repository publishes under their own name |
| 39 | **One desktop client** | The desktop app is the web client in a window: **no desktop-only screen** except the window's own chrome and the connect screen, which is built from the web client's own components. Platform differences live in one seam (`lib/desktop.ts` and the transport) and a check keeps it that way |
| 40 | **How an administrator gives a team the app** | The same executable plus a small configuration file (`repomemo-desktop.json`: the server, a name, whether it is locked), or an installer packaged for one server by a command. Signing and automatic updates are separate, later releases that need the maintainer's certificates and keys; until then the app is unsigned and the guide says what Windows will show |

## 4. The tracks

| Track | Releases | What it settles |
|---|---|---|
| **U. Configuration and secrets** | V0.4.1–V0.4.6 | A container needs secrets as files, a way to check a configuration without starting, build information, the option to put the database and the files on different volumes, a check of the volume at start and a stop that does not hurt the data |
| **V. The server image** | V0.4.7–V0.4.13 | A cached and smaller build, labels, a volume that belongs to root, an API-only compose file, the converter on its own, a scan |
| **W. The web image and the topology** | V0.4.14–V0.4.21 | The client as its own image with an address read when it starts, the two modes (same origin, cross origin), TLS as a separate piece, samples for other front doors, the server able to serve the client, a version check on the client's side |
| **X. Compose and running containers** | V0.4.22–V0.4.30 | The compose shape, scripts that make the `.env` and upgrade safely, backups, **the tests that matter**: smoke over the whole stack and upgrade over named volumes, healthchecks, logs, metrics |
| **Y. Direct install** | V0.4.31–V0.4.36 | The release archive, a hardened systemd unit, a Windows service, front doors, binaries built by CI, a start that refuses a database from the future |
| **Z. Registry and release pipeline** | V0.4.37–V0.4.42 | Images pushed by a tag, provenance, release notes from the changelog, compose on released images, a quick start, pinning |
| **AA. The desktop app on the web client** | V0.4.43–V0.4.61 | Parity audit, one transport, one look, a preset server and a file, the admin packager, **Windows** build and installer, files, notifications, deep links, certificates, updates, signing, then **Linux** and **macOS** |
| **AB. Documents, security and closing** | V0.4.62–V0.4.67 | The deployment guide, the desktop administrator's guide, troubleshooting, a security pass, the clean-install record, the release checks |

### Order and dependencies

- **U before V and W:** the image work relies on secrets from files, the check and the build information.
- **W (V0.4.14–V0.4.16) before X:** compose needs the two images apart.
- **X's tests (V0.4.26, V0.4.27) as early as the stack allows,** since containers are what the line is tested through.
- **Y and Z can run beside W–X:** they share the binary and the checks, not the containers' shape.
- **AA starts with its audit (V0.4.43) and can begin once V0.4.21 (the version check) exists,** because the desktop app needs it; Windows (V0.4.46–V0.4.51) before the rest of the desktop work; Linux and macOS (V0.4.59–V0.4.61) last.
- **Security passes** for the new surfaces come with them (V0.4.17, V0.4.38, V0.4.57) and once more at V0.4.65.

## 5. The desktop app in detail

**What stays as it is.** The bundle is one: `App.tsx` already picks the same `SharedWebApp` in both places, so the pages, the components and the styles are the web client's. The shell is small (`connection.rs`: choose and remember a server, keep the session in the credential store, open external links, allow the window to call that server and no other). Nothing in the window can leave the bundled client.

**What this line changes.**

1. **Parity as a rule** (V0.4.43, V0.4.44, V0.4.45, V0.4.58): the differences are listed, the transport is one seam, the connect screen is built from the shared components and tokens, and `desktop:check` fails a desktop-only screen or stylesheet that is not on a short list with a reason.
2. **The administrator's file** (V0.4.46–V0.4.48): `repomemo-desktop.json` beside the executable (or in the app's configuration folder) holds the server address, a name for the window, whether the address is locked and a support link. It is checked by the same rules as a typed address (`https` unless the server is on the person's own network, no login or path in it). A packaged installer carries the file; a plain executable reads the one next to it, so an administrator can send *one executable and one small file*. `npm run desktop:package` makes the installer for a server without the administrator having to know Rust, from the CI build.
3. **Windows first** (V0.4.49–V0.4.52, V0.4.57): a `windows-latest` job builds the installer (per person, no administrator rights, a silent switch for IT) and a portable executable; the WebView2 runtime is fetched when missing or carried for offline machines; the app's identity is finished (icon, version details, one instance, window place); files are saved through a native dialog; code signing is used when the maintainer supplies a certificate, and the SmartScreen warning is documented when not.
4. **Beyond the minimum** (V0.4.53–V0.4.56): native notifications, `repomemo://` links, servers with their own certificate authority (a bundle trusted for that server only), signed updates.
5. **Linux and macOS** (V0.4.59–V0.4.61): the same preset and file; the scripts that say `npm.cmd` are made portable; AppImage and `.deb`; a universal `.dmg`, signed and notarized when the secrets exist; a page for administrators saying where each system keeps the configuration, the session and the logs.

**Not decided here:** a mobile app; an app store; a second UI for the desktop.

## 6. How the line is tested

The sessions that write the code build nothing and start no server, so every container test is **written and compiled with its release and run by CI** (and first by the maintainer, who watches the first run):

| Test | Release | What it proves |
|---|---|---|
| Stack up and ready | exists (V0.3.23), extended by V0.4.28 | Every service turns healthy |
| **Smoke over the stack** | V0.4.26 | First-run setup, a workspace, an upload, indexing, search, a backup, an export and an import all work *in the containers* |
| **Upgrade over named volumes** | V0.4.27 | The previous release's data survives the new images; a migration that loses data fails the job |
| **Restore into a second stack** | V0.4.25 | A backup made in one stack is a working stack in another |
| Each image on its own | V0.4.11, V0.4.15 | The API runs without the web and the web without the API |
| Two origins | V0.4.17 | Sign-in, refresh and live events across two addresses, and a refused third |
| Scan | V0.4.13 | No high finding in any image |
| Archives and unit files | V0.4.31, V0.4.32, V0.4.35 | The archive's layout, the unit's syntax, the three binaries |
| Desktop builds | V0.4.49, V0.4.59, V0.4.60 | The installers are produced and open the app |
| Configuration and parity checks | V0.4.47, V0.4.58 | The file's rules, and no desktop-only screens |

Unit and web tests accompany each release as before. The maintainer's record of a container install goes in the clean-install page (V0.4.66).

## 7. Risks and how the plan meets them

| Risk | What the plan does |
|---|---|
| **Cross-origin sign-in** puts a cookie on a different site, and a wrong policy is a session leak | It is a separate release with its own tests (V0.4.17), the default stays same-origin, and the deployment pass (V0.4.65) reads it again |
| **A runtime `config.json`** could point the client at another API | It is served by the web image from its own environment, never from the request; the content security policy is built from the same value at start; the desktop app takes its address from the shell, not from the page |
| **Secrets in files** can be world-readable | `--check-config` (V0.4.2) and the volume check (V0.4.5) warn about permissions and about a secret in both a variable and a file |
| **Splitting the data folder** can make a backup incomplete | The backup, restore and `--check-config` know the three folders (V0.4.4), and restoring into three mounts is tested |
| **A migration loses data** in a container upgrade | The upgrade test (V0.4.27) is the guard, and the server refuses a database from a newer version (V0.4.36) |
| **A published image is a promise** | Provenance and scanning (V0.4.13, V0.4.38), tags only from a tag build, pins checked by `container:check` (V0.4.42) |
| **The desktop file is a way to send a team to the wrong server** | Only the administrator's own file is read; it is validated like a typed address; a locked app shows the address it is locked to |
| **An unsigned installer is distrusted** by Windows | The guide says so, the signed path exists when the certificate does (V0.4.57), and per-user install avoids an administrator prompt |
| **WebView differences** (downloads, certificates, notifications) | Each is its own release with a native path; the parity audit lists them first |
| **CI cost and time** (images, three platforms) | Cached layers (V0.4.7), jobs on tags and on changes to the files that matter, and the long jobs kept out of the required checks until they are steady |
| **Things only the maintainer can supply:** a registry setting, a signing certificate, an Apple account, an update key | Each release that needs one says so and ships working without it |

## 8. Left out, on purpose

Kubernetes manifests and a Helm chart; PostgreSQL and a scale-out server; a hosted service; single sign-on (the [other plan](2026-10-24-single-sign-on.md), kept); a mobile app; installers for package managers (winget, Homebrew, apt) and app stores; and any change to what RepoMemo stores or how it searches.

## 9. What the maintainer decides now

1. **Decisions 33–40**, each as proposed or changed.
2. **The numbering** (decision 34): `V0.4.1` onward under `V0.4.0`, as proposed.
3. **What to supply, and when:** the registry (GHCR under which owner), a Windows signing certificate, an Apple developer account, an update signing key; none blocks a release before the one that needs it (V0.4.38, V0.4.56, V0.4.57, V0.4.60).
