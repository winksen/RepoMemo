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

If this is a fresh checkout, create the local Tauri icon first:

```powershell
npm run icons
```

Why: Tauri's Windows resource build requires `apps/desktop/src-tauri/icons/icon.ico`.

```powershell
cargo check
```

Why: verifies Rust crates, Tauri command wiring, SQLx migrations, and compile-time Rust dependencies.

This requires Rust/Cargo to be installed and available on `PATH`.
See `docs/WINDOWS_SETUP.md` if PowerShell reports `cargo` as not recognized.

## Versions And Continuous Integration

The product version lives in several files (Cargo, npm, Tauri). Set them all at
once, and check that they agree (CI runs the check on every push):

```powershell
npm run version:set -- 0.1.67
npm run version:check
```

Record what changed in `CHANGELOG.md` under the new version.

## Run It In Containers

The server and a Caddy front door (TLS and the web app) as two containers, with the data in a volume:

```powershell
Copy-Item .env.example .env   # set REPOMEMO_JWT_SECRET and REPOMEMO_SECRET_KEY (32+ random characters each), and REPOMEMO_HOST
docker compose up -d --build
docker compose logs server    # the first-run setup code
```

Why: this is the way to run RepoMemo for a team without building anything by hand; the production guide ("Running it in containers") explains the volumes, the second backup copy, upgrading and using your own proxy. `npm run container:check` compares the container files with each other and with the server's variables without starting Docker, asserts the git release of the server image, and refuses a base image or CI action that is not pinned (a digest or a commit).

## Web Client Tests And Generated Types

```powershell
npm run test:web          # Vitest + Testing Library (apps/desktop/src/test)
npm run types:generate    # rewrite apps/desktop/src/generated/domain.ts from crates/domain
npm run types:check       # fail when that file is out of date (CI runs this)
npm run styles:check      # stylesheet size and order, and no new bare element style, raw colour, raw layer or raw control height (CI runs this)
npm run a11y:check        # accessibility rules and colour contrast, reading files only (CI runs this)
npm run migrations:check  # no released database migration was edited or removed, every one is locked (CI runs this)
npm run docs:check        # routes, settings, versions, releases and links of the documents agree with the code (CI runs this)
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
`REPOMEMO_PERF_FILES` and `REPOMEMO_PERF_ISSUES`, only in tests.

| Variable | Default | What it does |
|---|---|---|
| `REPOMEMO_SERVER_ADDR` | `127.0.0.1:3020` | Address and port the API listens on. The server speaks plain HTTP and has no connection or request timeouts of its own; put a TLS reverse proxy with header, body and idle timeouts in front of it before binding beyond loopback. |
| `REPOMEMO_SERVICE_NAME` | `repomemo-server` | Name reported by `/health`. |
| `REPOMEMO_OFFICE_PREVIEWS` | `false` | Turns on layout-accurate Office previews through LibreOffice (V0.2.137). Off: nothing is converted and LibreOffice is never started, even when it is installed; text, table and slide previews are used. On: LibreOffice must report 26.2.5 or later on the 26.2 line, or 26.8.0 or later; an older or unreadable release is refused, named in System › Overview and the server check, and nothing is converted. |
| `REPOMEMO_SOFFICE` | found automatically | Path to LibreOffice's `soffice` for layout-accurate Office previews, used when `REPOMEMO_OFFICE_PREVIEWS` is on. Unset: looked up on `PATH` and in the usual install folder. |
| `REPOMEMO_ALLOWED_ORIGIN` | `http://127.0.0.1:3021` | Browser origins allowed to call the API. Comma-separated for several; `*` is refused. |
| `REPOMEMO_SECRET_KEY` | unset | Key (32+ characters) used to encrypt AI provider API keys at rest. When unset, a random key is created in `<data dir>/secret.key`; back that file up with the database, or the saved keys must be entered again. |
| `REPOMEMO_ALLOW_REGISTRATION` | `false` | `true` opens sign-up to anyone who can reach the server; by default people join with invitation links. The very first account can still register, so a new server gets its owner. Email addresses are not verified, so keep it closed on a server others can reach. |
| `REPOMEMO_PUBLIC_URL` | unset | The address people use, such as `https://memo.example.com`. With `https://` the server may listen beyond loopback and sends `Strict-Transport-Security`. |
| `REPOMEMO_TRUST_PROXY` | `false` | Read the client address from `X-Forwarded-For` / `X-Real-IP`, but only when the TCP peer is loopback or listed in `REPOMEMO_TRUSTED_PROXIES`; the rightmost address no trusted proxy added wins. Also accepted as proof of a TLS proxy for non-loopback binds. |
| `REPOMEMO_TRUSTED_PROXIES` | unset | Comma-separated proxy addresses besides loopback whose forwarding headers are believed. |
| `REPOMEMO_ALLOW_INSECURE_HTTP` | `false` | Lets the server listen beyond loopback over plain HTTP with no proxy in front. For a trusted network only; without it (and without an `https://` public URL or proxy trust) the server refuses to start. |
| `REPOMEMO_MAX_UPLOAD_MB` | `10` | Largest request body (uploads). |
| `REPOMEMO_MAX_IMPORT_MB` | `2048` | Largest workspace archive an import accepts (1–1,048,576). The archive is received into a temporary file in `<data dir>/imports/`, not into memory, so the disk needs that much room while it is read; a proxy in front must allow a body this large on `POST /v1/workspaces/import`. |
| `REPOMEMO_REQUEST_TIMEOUT_SECONDS` | `120` | How long a request may take before it is answered `408`. Live event streams are not limited. `0` turns it off. |
| `REPOMEMO_HEADER_TIMEOUT_SECONDS` | `30` | How long a client has to send its request headers; a slower one is disconnected. `0` turns it off. |
| `REPOMEMO_MAX_EVENT_STREAMS_PER_USER` | `10` | Live event streams one account may hold open (one per open workspace page or System jobs page). |
| `REPOMEMO_INDEX_CONCURRENCY` | `2` | Artifacts indexed at once (1–16). Indexing may call a vision provider. |
| `REPOMEMO_EMBEDDING_CONCURRENCY` | `1` | Workspaces embedded at once (1–8). Raise it only for a provider that can take it. |
| `REPOMEMO_DB_READ_CONNECTIONS` | `4` | Read-only database connections (1–64). Writes always use one. |
| `REPOMEMO_WORKSPACE_STORAGE_MB` | `20480` | Evidence one workspace may hold, in MiB; uploads and notes beyond it answer `413`. `0` is unlimited. Also a run-time setting. |
| `REPOMEMO_UPLOADS_PER_HOUR` | `600` | Files and new versions one person may upload per hour; `0` is unlimited. Also a run-time setting. |
| `REPOMEMO_WORKSPACES_PER_PERSON` | `25` | Workspaces one person may own; `0` is unlimited. Also a run-time setting. |
| `REPOMEMO_DISK_FLOOR_MB` | `1024` | Uploads answer `507` while less than this is free on the disk of the data folder; `0` turns the check off. Also a run-time setting. |
| `REPOMEMO_BACKUP_INTERVAL_HOURS` | `0` | The server backs up by itself when the newest backup is older than this (checked by the maintenance loop); `0` is off. Also a run-time setting. |
| `REPOMEMO_BACKUP_COPY_DIR` | unset | A folder on another disk or a mounted share. After each backup the server copies it there and checks the copy (SHA-256 of every file, totals against the manifest); a failure shows in System › Backups and is retried. It must not be inside the backups folder. |
| `REPOMEMO_BACKUP_KEY` | unset | Passphrase (16+ characters) that seals every new backup with AES-256-GCM; restoring a sealed backup needs the same value, and a wrong one is refused before anything is written. Keep it apart from the backups: it is stored nowhere in them. |
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
| `REPOMEMO_LOG_FORMAT` | `text` | Console log format: `text`, `compact` or `json` (one JSON object per line, for log collectors). |
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

