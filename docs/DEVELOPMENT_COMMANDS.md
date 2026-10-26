# RepoMemo Development Commands

Run these from the repository root unless a command says otherwise.

## Install Dependencies

```powershell
npm.cmd install
```

Why: downloads the React, Vite, Tauri CLI, and UI icon dependencies declared in `package.json`.
Run this again whenever `package.json` changes, such as after adding a Tauri plugin.

## Typecheck The Frontend

```powershell
npm.cmd run typecheck
```

Why: verifies the TypeScript React shell and Tauri command client types.

## Build The Frontend

```powershell
npm.cmd run build
```

Why: verifies the production Vite bundle that Tauri will load in packaged builds.

## Run The Web Preview Shell

```powershell
npm.cmd run web:dev
```

Why: starts the React UI in browser-preview mode. This does not launch the native Tauri shell, but it lets you inspect the interface quickly.

## Verify The Rust Workspace

The icon set is committed (V0.4.51: made from the logo, `apps/desktop/public/RM-logo.svg`). Make it again only when the logo changes:

```powershell
npm run icons
```

Why: Tauri's own icon command draws every size the installer, Windows, macOS and Linux use from the logo; the Windows resource build needs `apps/desktop/src-tauri/icons/icon.ico`.

```powershell
cargo check
```

Why: verifies Rust crates, Tauri command wiring, SQLx migrations, and compile-time Rust dependencies.

This requires Rust/Cargo to be installed and available on `PATH`.
See `docs/WINDOWS_SETUP.md` if PowerShell reports `cargo` as not recognized.

## Package The Desktop App For One Server

```powershell
$env:REPOMEMO_PRESET_SERVER = "https://memo.example.com"
npm run tauri:build --workspace @repomemo/desktop
```

Why: the address is compiled into the app (V0.4.46). An app packaged this way opens on its server's sign-in page with no address field,
ignores any address saved by an earlier run and cannot be pointed elsewhere (*Change server* is gone and the shell refuses it). The
address is checked by the rules of a typed one (`https://`, or `http://` on your own network, nothing after the host), and a bad one
stops the app at start with a message that names it. Without the variable, the app asks for an address as before.

## Configure The Desktop App With A File

The same executable can talk to different servers if it is given a file (V0.4.47): `repomemo-desktop.json`, beside the executable or in
the app's config folder (`%APPDATA%\com.repomemo.desktop` on Windows, `~/.config/com.repomemo.desktop` on Linux,
`~/Library/Application Support/com.repomemo.desktop` on macOS); the one beside the executable wins. A sample is
`deploy/desktop/repomemo-desktop.example.json`:

```json
{
  "server": "https://memo.example.com",
  "name": "Acme Memo",
  "locked": true,
  "support": "https://intranet.example.com/help/memo"
}
```

- `server`: the server's address, checked by the rules of a typed one. With `locked` true (the default when there is a server) it
  overrides what a person typed, no connect screen is shown and *Change server* is gone; with `"locked": false` it only fills in the
  connect screen's field. An address compiled into the app (`REPOMEMO_PRESET_SERVER`) wins over the file's.
- `name`: what the window and the page are called (one line, up to 60 characters).
- `support`: an `http(s)` address shown as a *Help* link on the connect screen and in the account area.
- Nothing else is accepted. A file that is not valid JSON, names a field the app does not know, or holds a value that fails its rule is
  **named and not obeyed**: the app shows the file and the reason in place of everything else and connects to nothing, so a mistake
  cannot be gone around by typing an address.

## Hand A Team The Desktop App

```powershell
npm run desktop:package -- --server https://memo.example.com --name "Acme Memo" --support https://intranet.example.com/help/memo
```

Why: one command from an administrator's keyboard to the files a team needs, in `dist-desktop/` (V0.4.48): `RepoMemo-Acme-Memo-portable.zip`
(the program, `repomemo-desktop.json` and a readme; unpack it anywhere), `RepoMemo-Acme-Memo-setup.exe` (the per-person installer, when one is
there), the same `repomemo-desktop.json` for IT to place in `%APPDATA%\com.repomemo.desktop\`, and `SHA256SUMS`. The address, the name and the
help link are checked by the rules the app checks them by, and a mistake stops the command before anything is written. `--open` leaves the
server unlocked (the person may change it).

- **With Rust and the Tauri CLI installed,** the command builds the app with the address compiled in (the executable alone then talks to
  that server and no other), as the section above does, and packs the result.
- **Without Rust,** point it at the app CI builds (the *Desktop Windows* workflow: run it by hand from the Actions page, or take the one of a tag, and download the artifact `desktop-windows-x64`; it holds `repomemo-desktop.exe`, the per-person installer `RepoMemo_<version>_x64-setup.exe`, `RepoMemo_<version>_x64-offline.exe` which carries the WebView2 runtime for machines with no internet, and `SHA256SUMS`): `npm run desktop:package -- --server ... --name ... --from C:\path\to\artifact`, where
  the folder holds `repomemo-desktop.exe` (and `*-setup.exe`). It builds nothing; it adds the configuration file and packs the handout.
  Without a compiled-in address the program obeys the file beside it, which the person could delete: for a team that must not reach
  another server, build with Rust.

`npm run desktop:test` tests the command (checks, handout, reproducible zip).

### The Windows installer

`RepoMemo_<version>_x64-setup.exe` (V0.4.50) installs **for the person who runs it**: no administrator rights, into
`%LOCALAPPDATA%\RepoMemo`, recorded under that person's own uninstall list, with a Start menu entry and a desktop shortcut.

- **Silent install for IT:** `RepoMemo_<version>_x64-setup.exe /S` (add `/NS` for no shortcuts, and `/D=C:\path` last to choose the
  folder). Put `repomemo-desktop.json` in `%APPDATA%\com.repomemo.desktop\` or beside the installed program.
- **Uninstall:** from *Apps* in Settings, or `%LOCALAPPDATA%\RepoMemo\uninstall.exe`. Interactively it asks whether to also **forget
  the server and the sign-in** saved on the computer (the saved server and the session in the Windows credential store; the
  administrator's file is left alone); the default is to keep them. `uninstall.exe /S` asks nothing and keeps them, so an update that
  removes and installs again keeps the person signed in; to forget on a silent removal, run `RepoMemo.exe --forget-session` first (it
  opens no window).

## Versions And Continuous Integration

The product version lives in several files (Cargo, npm, Tauri). Set them all at
once, and check that they agree (CI runs the check on every push):

```powershell
npm run version:set -- 0.1.67
npm run version:check
```

Record what changed in `CHANGELOG.md` under the new version. The page of a release is written from that entry, so nobody edits it
by hand: `npm run notes` prints it for the version in `package.json` (or `-- --version 0.4.39`), and `npm run notes:test` tests the
script. The release workflows use the same script when a `v*` tag is pushed.

## Run It In Containers

The server and a Caddy front door (TLS and the web app) as two containers, with the data in a volume:

```powershell
Copy-Item .env.example .env   # set REPOMEMO_JWT_SECRET and REPOMEMO_SECRET_KEY (32+ random characters each), and REPOMEMO_HOST
docker compose pull; docker compose up -d   # the released images; docker-compose.build.yml builds this checkout instead
docker compose logs server    # the first-run setup code
```

Why: this is the way to run RepoMemo for a team without building anything by hand; the production guide ("Running it in containers") explains the volumes, the second backup copy, upgrading and using your own proxy. `npm run container:check` compares the container files with each other and with the server's variables without starting Docker, asserts the git release of the server image, and refuses a base image or CI action that is not pinned (a digest or a commit).

## Web Client Tests And Generated Types

```powershell
npm run test:web          # Vitest + Testing Library (apps/desktop/src/test)
npm run types:generate    # rewrite apps/desktop/src/generated/domain.ts from crates/domain
npm run types:check       # fail when that file is out of date (CI runs this)
npm run styles:check      # stylesheet size and order, and no new bare element style, raw colour, raw layer or raw control height (CI runs this)
npm run a11y:check        # accessibility rules, toolbars, the dialogs' skip link, button sizes and colour contrast, reading files only (CI runs this)
npm run migrations:check  # no released database migration was edited or removed, every one is locked (CI runs this)
npm run docs:check        # routes, settings, versions, releases and links of the documents agree with the code (CI runs this)
npm run templates:check   # every workspace template file is valid, a changed one has a new version, and no template id is written in code (CI runs this; --base <ref> compares with another commit)
npm run templates:test    # the checker's own tests (Node's test runner; CI runs this)
```

**Performance runs** (release builds, `#[ignore]`d, they print timings): `cargo test -p repomemo-storage --release performance_at_scale -- --ignored --nocapture`, `cargo test -p repomemo-api --release map_and_health_at_scale -- --ignored --nocapture`, `cargo test -p repomemo-api --release repo_sync_at_scale -- --ignored --nocapture` (a repository of 100,000 files, `REPOMEMO_PERF_FILES`) and `cargo test -p repomemo-server --release ten_thousand_issues -- --ignored --nocapture` (10,000 issues through a connector, `REPOMEMO_PERF_ISSUES`). Targets, what each measures and where to record the figures: [mindmap/technical/performance.md](../mindmap/technical/performance.md).

A **database migration** that has shipped is never edited: a change goes into a new numbered file in `crates/storage/migrations/`, and `node scripts/check-migrations.mjs --lock` records it in `crates/storage/migrations.lock` with the release that ships it. `cargo test -p repomemo-storage an_environment_made_by_v0_2_0` runs the upgrade test (a 0.2.0 environment opened by the current code).

After changing a data transfer object in `crates/domain/src/lib.rs`, run
`types:generate` and commit the result. `src/types.ts` re-exports the generated
types that match; only types the client shapes differently are written by hand.

CI (`.github/workflows/ci.yml`) runs on every push to `main` and every pull
request: the version check, Rust tests on Linux and Windows (the desktop crate
is left out), the web type check, the web client tests, the accessibility check, the generated-types check, `npm audit --omit=dev`, `cargo deny`
(policy in `deny.toml`), `cargo fmt --all --check` and clippy with `-D warnings`.
Licenses and banned crates are checked as well; duplicate versions only warn.
Two jobs try the container for real: one writes a `.env` with throwaway secrets, runs
`docker compose up --build --detach --wait` and asks `/health/ready` through the proxy, and one builds the
server and proxy images for `linux/arm64` on an arm64 runner and checks their architecture (nothing is
published). Both build the Rust server in the image, so they take a while.

## Check A Running Server

```powershell
node scripts/smoke-test.mjs https://memo.example.com
```

Why: health, readiness, security headers, the body limit and the webhook answer, and the web app of a server you just installed; `REPOMEMO_SMOKE_MAIL` (an address) also sends a test mail and waits for it, and `REPOMEMO_SMOKE_REMOTE_REPO` with `REPOMEMO_SMOKE_WORKSPACE` (and `REPOMEMO_SMOKE_REMOTE_TOKEN`) checks that a remote repository can be linked (the run before the tag is [mindmap/technical/clean-install.md](../mindmap/technical/clean-install.md)); with `REPOMEMO_SMOKE_EMAIL` and `REPOMEMO_SMOKE_PASSWORD` it also reads the overview and metrics (and makes a backup with `REPOMEMO_SMOKE_BACKUP=1`).

## List The API Routes

```powershell
node scripts/list-routes.mjs
```

Why: every route with its authentication extractor and the guard calls in its handler; `--markdown` writes the table behind `mindmap/technical/api-reference.md`. A route marked `check` has no guard call, which is right only for a person's own data.

## Run Rust Tests

```powershell
cargo test
```

Why: runs unit tests for ingestion rules, content hashing, and future Rust core behavior.

## Run The Desktop App

```powershell
npm.cmd run dev
```

Why: launches the Tauri desktop app with the React dev server and the Rust shell.

On first start the app asks for the address of a RepoMemo server (run one with the next section: `http://127.0.0.1:3020` is accepted because it is on your own machine) and keeps it in `server.json` in the app's config folder. The session is kept in the system credential store (entry *RepoMemo*); *Change server* in the left rail forgets both and restarts the app. The window can only show the bundled client, and it can only call the server you chose.

This requires Rust/Cargo and the platform dependencies required by Tauri. The Rust tests of the shell (address rules, which pages the window may show) run with `cargo test -p repomemo-desktop`; CI leaves that crate out because it needs the system web view libraries.

## Run The Shared API

```powershell
$env:REPOMEMO_JWT_SECRET = 'replace-this-with-a-random-development-secret-of-at-least-32-characters'
cargo run -p repomemo-server
```

The API binds to `127.0.0.1:3020` by default. It stores server-owned
development data in an *environment* folder, always directly inside
`workspace-data/` (git-ignored, so new projects are never tracked). Start the
server with no `REPOMEMO_SERVER_DATA_DIR` and the web app shows a menu, unlocked
by the code printed on the console, to create `workspace-data/<name>/` or to
reroute to an existing one; the folder is verified first. The choice is
remembered in `workspace-data/.last-environment` and attached again at the next
start; detaching it in System › Settings forgets it.

To pin the server to one environment instead, set
`REPOMEMO_SERVER_DATA_DIR=main` (or `workspace-data/main`): every start attaches
that folder. The value is verified at startup, and anything outside
`workspace-data/`, or a folder that is neither empty nor a RepoMemo environment,
stops the server. A pinned server can still be detached and pointed at another
environment from the web app, but only until it restarts; the web app warns
about this and names the environment it will go back to. `$env:` values last
for the PowerShell window they were set in, so a restart from the same window
keeps the pin. The server
exposes JWT-protected workspace routes. Import the newest collection,
`docs/api/RepoMemo-Shared-API_v5.postman_collection.json` (the folder is
git-ignored, so it exists only in local checkouts), into Postman and run its
folders in order to exercise authentication, workspace setup, evidence,
indexing, retrieval, team memory, workspace membership management, and (new in
v5) invitation links, password reset links and personal access tokens. For the
membership folder, register the teammate first, then set its email in the
`memberEmail` collection variable.

## Open The Admin Console In A Terminal

With the API running, system and app administrators can open the same console
as System › Console in the web app, with its logo, history, Tab completion and
usage hints:

```powershell
npm run console
```

This is `cargo run -q -p repomemo-console`. It asks for an email and password
(hidden) and keeps the session in memory only; earlier lines are kept in
`~/.repomemo/console-history.txt`. With a command it runs it once and exits,
which suits scripts (`--json` prints the raw answer):

```powershell
npm run console -- status
npm run console -- jobs list --status failed --json
```

| Setting | Default | Meaning |
|---|---|---|
| `--server` or `REPOMEMO_SERVER_URL` | `http://127.0.0.1:3020` | The server to talk to. |
| `--email` or `REPOMEMO_EMAIL` | asked | The account to sign in with; the password is always asked for. |
| `REPOMEMO_TOKEN` | unset | A personal access token (`rmp_…`, created in the web app under Profile › API tokens) to use instead of signing in, for scripts. A read-only token runs lookups and dry runs only. |
| `NO_COLOR` | unset | Any value turns colours off. |

Exit codes: `0` done, `1` the server refused the command, `2` sign-in or
connection problem.

### Optional server settings

Every setting below has a safe default; set only what you need. Invalid values
stop the server at startup with a message naming the variable. System
administrators can change the registration, token, protection, AI-quota and
background settings at run time in **System › Settings**; those changes are
stored in the database and override the values below until reset.

Four more variables are set by the server itself for the helper it starts and are
not settings: `REPOMEMO_GIT_ASKPASS`, `REPOMEMO_GIT_USERNAME` and
`REPOMEMO_GIT_SECRET` (the credential handed to one git command, through the server's own
program acting as git's askpass) and `REPOMEMO_READER_MEMORY_MB` (the memory cap of a document
reader). Do not set them. The performance runs read `REPOMEMO_PERF_PASSAGES`,
`REPOMEMO_PERF_FILES`, `REPOMEMO_PERF_ISSUES` and `REPOMEMO_PERF_LISTED_FILES` (the files of the file-list case, 100,000), only in tests.

**Secrets from files (V0.4.1).** `REPOMEMO_JWT_SECRET`, `REPOMEMO_SECRET_KEY`, `REPOMEMO_BACKUP_KEY`, `REPOMEMO_BACKUP_KEY_PREVIOUS` and `REPOMEMO_SETUP_CODE` can each be given as a file instead: set the same name ending in `_FILE` (for example `REPOMEMO_JWT_SECRET_FILE=/run/secrets/jwt`) to the path of a file that holds the value, as Docker and Kubernetes secrets are mounted. White space and the line ending around the value are dropped. A variable and a file for the same secret are refused at start, and so is a file that cannot be read or is empty. The same rules and limits apply as for the variable.

**The web app's own configuration (V0.4.14).** The built web app asks the host that serves it for `/config.json` when it starts and reads two things from it: `apiUrl` (the API's address; empty means the page's own host; absent keeps the address it was built with, `VITE_REPOMEMO_API_URL`) and `productName` (the browser tab's title). A host with no such file, or the Vite development server, leaves the build's values. The build writes a default `config.json` beside the page; the compose file's proxy serves its own from `REPOMEMO_WEB_API_URL` and `REPOMEMO_PRODUCT_NAME` (compose settings, not the server's; no quote or backslash in either). The page's content security policy still allows only its own host and the address it was built for, so an API on another address needs the web image (V0.4.17), which adds the API's origin to the policy it sends when it starts.

**Health check (V0.4.8).** `repomemo-server --health` asks the server on this machine (the address in `REPOMEMO_SERVER_ADDR`, with an all-interfaces address asked on the loopback) for `/health/ready` and exits `0` when it answers 200, else `1`. It is the container's health check, so the image needs no `curl`.

**Checking a configuration (V0.4.2).** `repomemo-server --check-config` reads the environment as a start would, prints what it would use with the secrets hidden, checks that the data folder exists and is writable (or can be created), and exits without serving: `0` the configuration can start, `1` it is invalid, `2` the data folder cannot be used. In a container: `docker compose run --rm server --check-config`.

| Variable | Default | What it does |
|---|---|---|
| `REPOMEMO_SERVER_ADDR` | `127.0.0.1:3020` | Address and port the API listens on. The server speaks plain HTTP and has no connection or request timeouts of its own; put a TLS reverse proxy with header, body and idle timeouts in front of it before binding beyond loopback. |
| `REPOMEMO_SERVICE_NAME` | `repomemo-server` | Name reported by `/health`. |
| `REPOMEMO_BUILD_COMMIT`, `REPOMEMO_BUILD_DATE`, `REPOMEMO_IMAGE_TAG` | `unknown` | **Build-time only (V0.4.3), not read when the server runs:** set as build arguments of the Dockerfile (or as environment variables of `cargo build`), they are compiled in and reported by `/health` (`commit`, `built_at`, `image`) and by `repomemo-server --version`, next to the package `version`. The compose file passes the commit, the date and `REPOMEMO_VERSION` as the image tag; a build without them reports `unknown`. |
| `REPOMEMO_OFFICE_PREVIEWS` | `false` | Turns on layout-accurate Office previews through LibreOffice (V0.2.137). Off: nothing is converted and LibreOffice is never started, even when it is installed; text, table and slide previews are used. On: LibreOffice must report 26.2.5 or later on the 26.2 line, or 26.8.0 or later; an older or unreadable release is refused, named in System › Overview and the server check, and nothing is converted. |
| `REPOMEMO_SOFFICE` | found automatically | Path to LibreOffice's `soffice` for layout-accurate Office previews, used when `REPOMEMO_OFFICE_PREVIEWS` is on. Unset: looked up on `PATH` and in the usual install folder. |
| `REPOMEMO_SHUTDOWN_GRACE_SECONDS` | `30` | How long running requests get to finish once the server is asked to stop (SIGTERM or Ctrl+C), V0.4.6. Keep it under the time the service manager waits (`docker stop` waits 10 seconds unless `stop_grace_period` says more). |
| `REPOMEMO_DB_DIR` | unset: the data folder | Absolute path of the folder that holds the database file (`repomemo.sqlite` and its write-ahead log), so it can sit on its own volume (V0.4.4). Needs `REPOMEMO_SERVER_DATA_DIR`. A server with it set finds the same database again only with it set; moving the file is the operator's step (stop, move, set, start). |
| `REPOMEMO_BLOB_DIR` | unset: `<data folder>/blobs` | Absolute path of the folder that holds the stored files (V0.4.4). Files being received wait in `.incoming` inside it, so storing one stays a rename on one volume. Needs `REPOMEMO_SERVER_DATA_DIR`. |
| `REPOMEMO_BACKUP_DIR` | unset: `<data folder>/backups` | Absolute path of the folder backups are made and kept in (V0.4.4); its free space is the one System > Backups shows. Needs `REPOMEMO_SERVER_DATA_DIR`. |
| `REPOMEMO_ALLOWED_ORIGIN` | `http://127.0.0.1:3021` | Browser origins allowed to call the API. Comma-separated for several; `*` is refused. With a web app on another address (V0.4.17), list its origin here; credentials are allowed for the listed ones only. |
| `REPOMEMO_WEB_DIR` | unset | Absolute path of the built web app (`npm run build` makes `apps/desktop/dist`) for the server to serve itself (V0.4.20), for a single-process install: one program, one port. Off when unset. It must hold an `index.html`. Everything under `/v1/` and `/health` stays the API's; any other path is a file of the folder, or the page for a client route (a missing file with an extension is a 404), with the web image's headers and `Cache-Control` (hashed files under `/assets/` for a year, the rest `no-cache`) and the content security policy the build wrote. `/config.json` is made by the server: the API is this host. No compression of its own; a front door brings it. |
| `REPOMEMO_PRODUCT_NAME` | `RepoMemo` | The name `/config.json` gives the page (the browser tab's title) when the server serves the web app (V0.4.20). One line of 60 characters or fewer, no quote or backslash. |
| `REPOMEMO_COOKIE_SAMESITE` | `strict` | The sign-in cookie's `SameSite` attribute (V0.4.17). `strict`: the web app and the API on one site. `lax` or `none`: a web app on another address; then a request that presents the cookie from an origin not listed in `REPOMEMO_ALLOWED_ORIGIN` is refused with 403. `none` is accepted only with an `https://` `REPOMEMO_PUBLIC_URL`, since browsers take it only with `Secure` (and some block it as a third-party cookie, so prefer two subdomains of one domain with `lax`). |
| `REPOMEMO_SECRET_KEY` | unset | Key (32+ characters) used to encrypt AI provider API keys at rest. When unset, a random key is created in `<data dir>/secret.key`; back that file up with the database, or the saved keys must be entered again. |
| `REPOMEMO_ALLOW_REGISTRATION` | `false` | `true` opens sign-up to anyone who can reach the server; by default people join with invitation links. The very first account can still register, so a new server gets its owner. Email addresses are not verified, so keep it closed on a server others can reach. |
| `REPOMEMO_PUBLIC_URL` | unset | The address people use, such as `https://memo.example.com`. With `https://` the server may listen beyond loopback and sends `Strict-Transport-Security`. |
| `REPOMEMO_TRUST_PROXY` | `false` | Read the client address from `X-Forwarded-For` / `X-Real-IP`, but only when the TCP peer is loopback or listed in `REPOMEMO_TRUSTED_PROXIES`; the rightmost address no trusted proxy added wins. Also accepted as proof of a TLS proxy for non-loopback binds. |
| `REPOMEMO_TRUSTED_PROXIES` | unset | Comma-separated proxy addresses besides loopback whose forwarding headers are believed. |
| `REPOMEMO_ALLOW_INSECURE_HTTP` | `false` | Lets the server listen beyond loopback over plain HTTP with no proxy in front. For a trusted network only; without it (and without an `https://` public URL or proxy trust) the server refuses to start. |
| `REPOMEMO_MAX_UPLOAD_MB` | `10` | Largest file an upload or a new version accepts (1–1024). It is received into a temporary file in `<data dir>/tmp` and never held in memory. Every other request body stops at 1 MiB; an import has its own limit. |
| `REPOMEMO_UPLOAD_INFLIGHT_MB` | `512` | Uploads in flight at once, all senders together (1–1,048,576). An upload counts by the size it announces (the largest allowed when it announces none), and past the total a new one answers `503` with `Retry-After`. |
| `REPOMEMO_MAX_IMPORT_MB` | `2048` | Largest workspace archive an import accepts (1–1,048,576). The archive is received into a temporary file in `<data dir>/imports/`, not into memory, so the disk needs that much room while it is read; a proxy in front must allow a body this large on `POST /v1/workspaces/import`. |
| `REPOMEMO_REQUEST_TIMEOUT_SECONDS` | `120` | How long a request may take before it is answered `408`. Live event streams are not limited. `0` turns it off. |
| `REPOMEMO_HEADER_TIMEOUT_SECONDS` | `30` | How long a client has to send its request headers; a slower one is disconnected. `0` turns it off. |
| `REPOMEMO_MAX_EVENT_STREAMS_PER_USER` | `10` | Live event streams one account may hold open (one per open workspace page or System jobs page). |
| `REPOMEMO_INDEX_CONCURRENCY` | `2` | Artifacts indexed at once (1–16). Indexing may call a vision provider. |
| `REPOMEMO_EMBEDDING_CONCURRENCY` | `1` | Workspaces embedded at once (1–8). Raise it only for a provider that can take it. |
| `REPOMEMO_DB_READ_CONNECTIONS` | `4` | Read-only database connections (1–64). Writes always use one. |
| `REPOMEMO_MAX_IN_FLIGHT` | `64` (16 for each read connection, at least 32) | Requests served at once, all accounts together (0 for no limit). Past it a request is refused at once with `503` and `Retry-After`, instead of queuing until its timeout. Health checks, sign-in and the event streams are not counted. Shown in System › Overview; the count and the refusals are in the metrics. |
| `REPOMEMO_MAX_IN_FLIGHT_PER_ACCOUNT` | `32` (8 for each read connection, at least 16) | Requests served at once for one account or personal access token (0 for no limit). Past it a request is refused at once with `429` and `Retry-After`. Shown in System › Overview. |
| `REPOMEMO_SQLITE_CACHE_MB` | `32` | Page cache of each read connection, in MiB (0 keeps SQLite's own, about 2 MiB; at most 4,096). Four readers hold up to four times this. Shown in System › Overview. |
| `REPOMEMO_SQLITE_MMAP_MB` | `256` | How much of the database each read connection may map into memory, in MiB (0 turns it off; at most 65,536). It is address space shared with the page cache of the system, not memory the server holds. Shown in System › Overview. |
| `REPOMEMO_WORKSPACE_STORAGE_MB` | `20480` | Evidence one workspace may hold, in MiB; uploads and notes beyond it answer `413`. `0` is unlimited. Also a run-time setting. |
| `REPOMEMO_UPLOADS_PER_HOUR` | `600` | Files and new versions one person may upload per hour; `0` is unlimited. Also a run-time setting. |
| `REPOMEMO_WORKSPACES_PER_PERSON` | `25` | Workspaces one person may own; `0` is unlimited. Also a run-time setting. |
| `REPOMEMO_DISK_FLOOR_MB` | `1024` | Uploads answer `507` while less than this is free on the disk of the data folder; `0` turns the check off. Also a run-time setting. |
| `REPOMEMO_BACKUP_INTERVAL_HOURS` | `0` | The server backs up by itself when the newest backup is older than this (checked by the maintenance loop); `0` is off. Also a run-time setting. |
| `REPOMEMO_TEMPLATES_DIR` | unset | A folder of workspace template files (`*.json`) added to the built-in ones, read once when the server starts. Each file is held to the rules of every template ([docs/TEMPLATES.md](TEMPLATES.md)): at most 64 KiB, at most 64 files, nested at most 8 deep, a link is not followed, a file is named after its id, and an id cannot be one of a built-in template or `general`. A file that fails is left out and named in System › Overview, in the server check and in the log; it never stops the server. Templates from the folder carry a *Custom* tag in the picker. Restart to load a change. Every signed-in account can read every template, so a file holds no secrets or personal data; its `focus` line is part of the prompt sent to a workspace's AI provider. |
| `REPOMEMO_BACKUP_COPY_DIR` | unset | A folder on another disk or a mounted share. After each backup the server copies it there and checks the copy (SHA-256 of every file, totals against the manifest); a failure shows in System › Backups and is retried. It must not be inside the backups folder. |
| `REPOMEMO_BACKUP_KEY` | unset | Passphrase (16+ characters) that seals every new backup with AES-256-GCM; restoring a sealed backup needs the same value, and a wrong one is refused before anything is written. Keep it apart from the backups: it is stored nowhere in them. |
| `REPOMEMO_BACKUP_KEY_PREVIOUS` | unset | The passphrase before the last change of `REPOMEMO_BACKUP_KEY` (16+ characters, different from it, and only with it set). It opens the backups made with it, which then show *previous key* in System › Backups and can be sealed again with the new one (`backup reseal`, the *Seal again with the current key* action). Leave it unset once none is left. |
| `REPOMEMO_BACKUP_KEEP` | `7` | Backups kept; older ones are deleted after a new one. Also a run-time setting. |
| `REPOMEMO_VECTOR_INDEX_MB` | `512` | Memory for in-memory vector indexes of meaning search, in MiB. A workspace of 50,000 passages with 768-dimensional vectors takes about 150 MiB. `0` turns the indexes off (the database is scanned). A workspace that does not fit is scanned. |
| `REPOMEMO_DOCUMENT_TIMEOUT_SECONDS` | `60` | How long reading one document may take before its reader process is killed and the file skipped. `0` for no limit. |
| `REPOMEMO_DOCUMENT_MEMORY_MB` | `1024` | Memory one document reading may use before its reader process stops and the file is skipped. `0` for no limit. |
| `REPOMEMO_DOCUMENT_READER` | `process` | `process`: each reading runs in a child process (the server started with `--document-reader`, with an empty environment). `thread`: on a thread of the server, as before V0.2.44, with no memory limit and no way to kill an overrunning reading. |
| `REPOMEMO_HEAVY_REQUESTS_PER_MINUTE` | `240` | Searches, figures, map, health and file queries per person per minute; `0` is unlimited. Also a run-time setting. |
| `REPOMEMO_BACKGROUND_AI_PER_DAY` | `2000` | Image descriptions and embedding batches the server makes on its own per workspace per day (UTC); `0` is unlimited. Also a run-time setting. |
| `REPOMEMO_ACCESS_TOKEN_TTL_MINUTES` | `60` | Access-token lifetime (5–1440). |
| `REPOMEMO_REFRESH_TOKEN_TTL_DAYS` | `30` | Refresh-token lifetime (1–365). |
| `REPOMEMO_AUTH_REQUESTS_PER_MINUTE` | `30` | Sign-in and registration requests per client address (session refreshes get four times as many); `0` turns the limit off. |
| `REPOMEMO_LOGIN_MAX_FAILURES` | `10` | Consecutive failed sign-ins (per account and address) before a temporary lockout; the account is also locked after five times as many from all addresses. A device that still holds one of the account's sessions gets through. `0` turns it off. |
| `REPOMEMO_LOGIN_LOCKOUT_MINUTES` | `15` | Length of that lockout. |
| `REPOMEMO_AI_REQUESTS_PER_HOUR` | `120` | AI requests (Ask, overviews, summaries, assistant AI actions) per user per hour; `0` is unlimited. |
| `REPOMEMO_AI_ALLOWED_HOSTS` | `127.0.0.1:11434,localhost:11434,[::1]:11434,openrouter.ai:443` | Comma-separated hosts AI providers may be configured at, such as `127.0.0.1:11434,.openrouter.ai,llm.internal:8080` (a leading dot allows subdomains). An entry may end in `:port`, and then the provider's address must use it; an entry without a port allows any port. With the built-in default, an address is also a host and a port and nothing more: no path beyond the provider's own (`/api/v1` for OpenRouter, none for Ollama) and no query, so `http://localhost:11434/api/pull` or `http://127.0.0.1:3020` is refused (V0.2.139). A list you write yourself leaves paths and unlisted ports to you. `*` allows every host. Link-local and cloud-metadata addresses are always refused, and names are resolved and checked at connect time. |
| `REPOMEMO_SMTP_ALLOWED_HOSTS` | unset | Mail servers allowed to be at private addresses, comma separated: a name or address, `.example.com` for a domain, or `*`. The mail server is set up in System › Settings › Mail; without this it must resolve to public addresses (link-local and cloud-metadata addresses are never allowed). |
| `REPOMEMO_SMTP_CA_FILE` | unset | A PEM file of certificate authorities trusted for the mail server besides the public ones, for a relay with a private certificate. |
| `REPOMEMO_REPO_ROOTS` | unset | Folders repository links must sit inside, separated like `PATH` (`;` on Windows, `:` elsewhere). **Unset turns repository links off.** Network shares are refused unless a root is on one. Who may link a folder is a System setting (*Who can link a local repository*, system administrators by default; V0.2.143). |
| `REPOMEMO_GIT_ALLOWED_HOSTS` | unset | Hosts remote repository links **and trackers (connectors)** may use, comma separated (`api.github.com` for GitHub, `gitlab.com` or the self-hosted host for GitLab, or a GitHub Enterprise host): `github.com`, `git.example.com:2222`, `*.corp.example` or `*`. Only `https` and `ssh`. **Unset turns remote links off.** Link-local and cloud-metadata addresses are never allowed, and a private address must be named. git resolves names itself, so for private targets use an egress rule too. |
| `REPOMEMO_GIT_SAFE_DIRECTORIES` | unset | Further folders git may read although another account owns them, separated like `PATH`. The repository folders are included. The server's git ignores the account's own `git config`. |
| `REPOMEMO_PREVIEW_ROOT` | unset (the data folder) | The folder that holds the `previews` folder: converted PDFs, the conversions' work folders and LibreOffice's profile. Set it to a volume of its own when LibreOffice runs in another container that must see only that (V0.2.148; the compose file sets it to `/var/lib/repomemo-previews`). |
| `REPOMEMO_SOFFICE_WRAPPER` | unset | A command LibreOffice is started through, split on spaces, with `{work}` (the conversion folder) and `{profile}` (its profile folder) replaced, for example `bwrap --ro-bind / / --bind {work} {work} --bind {profile} {profile} --unshare-net --die-with-parent`. Use it to run conversions without network and without the data folder. |
| `REPOMEMO_REPO_CLONE_MAX_MB` | `2048` | The most the server's copy of one remote repository may grow to; a clone or fetch that goes over is stopped. `0` is no cap. Also in System › Settings › Limits. |
| `REPOMEMO_REPO_CLONE_TIMEOUT_MINUTES` | `30` | The longest one clone or fetch of a remote repository may run before it is stopped. Also in System › Settings › Limits. |
| `REPOMEMO_REPO_POLL_SECONDS` | `300` | How often linked repositories are checked for new commits and synced; `0` turns automatic syncing off. |
| `REPOMEMO_MAINTENANCE_INTERVAL_MINUTES` | `60` | Background maintenance (token and job cleanup, blob garbage collection, retries, database checkpoint); `0` turns it off. |
| `REPOMEMO_BLOB_GC` | `true` | Delete stored files no evidence references any more. |
| `REPOMEMO_JOB_RETENTION_DAYS` | `90` | Finished jobs kept this long; `0` keeps them forever. |
| `REPOMEMO_INDEX_RETRY_HOURS` | `6` | How often indexing that failed for good is retried. |
| `REPOMEMO_SYSTEM_ADMIN_EMAILS` | unset | Comma-separated account emails made **system administrators** at startup, if the account exists then. Registering a listed address later does **not** grant the role (addresses are not verified); the server logs a warning instead, and an administrator grants it in System › Users. The first account created on a new server becomes one automatically; use this to name one on an existing server. |
| `REPOMEMO_SYSTEM_AUDIT_RETENTION_DAYS` | `365` | System administration events kept this long; `0` keeps them forever. |
| `REPOMEMO_SETUP_WIZARD` | `true` | A brand-new server (no account yet) opens the **onboarding** in the web app to create its first system administrator. `false` skips it: the first account to register becomes system administrator, as before. |
| `REPOMEMO_SETUP_CODE` | generated | The one-time code the onboarding asks for (at least 12 letters or digits). When unset, the server makes one at each start until setup is done and prints it on its console, in a framed block, never in the log files. |
| `RUST_LOG` | unset | A raw log filter (tracing syntax). When set, it is used instead of the per-category levels until a system administrator chooses levels in **System › Settings › Logging**; resetting those brings it back. |
| `REPOMEMO_LOG_FORMAT` | `text` | Console log format: `text`, `compact` or `json` (one JSON object per line, for log collectors). With `json`, the lines written before the logging is up (a refused configuration, a data folder that cannot be used) and by the container's entrypoint are JSON objects of the same shape too (V0.4.29), so every line parses. The compose files set it, with `REPOMEMO_LOG_TO_FILE=false`. |
| `REPOMEMO_LOG_TO_FILE` | `true` | Keep logs in daily files `<data dir>/logs/repomemo-YYYY-MM-DD.jsonl` (JSON lines, UTC days), browsable and downloadable in **System › Logs**. |
| `REPOMEMO_LOG_RETENTION_DAYS` | `14` | Daily log files older than this are deleted by maintenance; `0` keeps them forever. |

`GET /health/ready` answers 200 when the database responds and 503 otherwise,
for load balancers and service managers. Security-relevant events (sign-ins,
lockouts, password changes, provider keys) are logged under the `audit`
tracing target.

Run the React web client in a second terminal:

```powershell
npm.cmd run web:dev
```

Open `http://127.0.0.1:3021`. The web client reads `VITE_REPOMEMO_API_URL`,
which defaults to `http://127.0.0.1:3020`; copy `apps/desktop/.env.example` to
`apps/desktop/.env.local` to override it.

The web client keeps its refresh token in an `HttpOnly; SameSite=Strict`
cookie, so the web app and the API must be on one site: `127.0.0.1:3021` and
`127.0.0.1:3020` are (ports do not matter), and so is one host behind a
reverse proxy. Do not mix `localhost` and `127.0.0.1`. `npm run build`
writes `apps/desktop/dist/content-security-policy.txt`: send its content as the
`Content-Security-Policy` header from whatever serves `dist/`, after building
with `VITE_REPOMEMO_API_URL` set to the API address people will use.

