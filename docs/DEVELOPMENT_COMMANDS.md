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

## Web Client Tests And Generated Types

```powershell
npm run test:web          # Vitest + Testing Library (apps/desktop/src/test)
npm run types:generate    # rewrite apps/desktop/src/generated/domain.ts from crates/domain
npm run types:check       # fail when that file is out of date (CI runs this)
```

After changing a data transfer object in `crates/domain/src/lib.rs`, run
`types:generate` and commit the result. `src/types.ts` re-exports the generated
types that match; only types the client shapes differently are written by hand.

CI (`.github/workflows/ci.yml`) runs on every push to `main` and every pull
request: the version check, Rust tests on Linux and Windows (the desktop crate
is left out), the web type check, the web client tests, the generated-types check, `npm audit --omit=dev`, `cargo deny`
(policy in `deny.toml`), `cargo fmt --all --check` and clippy with `-D warnings`.
Licenses and banned crates are checked as well; duplicate versions only warn.

## Check A Running Server

```powershell
node scripts/smoke-test.mjs https://memo.example.com
```

Why: health, readiness, security headers and the web app of a server you just installed; with `REPOMEMO_SMOKE_EMAIL` and `REPOMEMO_SMOKE_PASSWORD` it also reads the overview and metrics (and makes a backup with `REPOMEMO_SMOKE_BACKUP=1`).

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

Why: launches the Tauri desktop app with the React dev server and Rust backend command bridge.

This requires Rust/Cargo and the platform dependencies required by Tauri.

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

| Variable | Default | What it does |
|---|---|---|
| `REPOMEMO_SERVER_ADDR` | `127.0.0.1:3020` | Address and port the API listens on. The server speaks plain HTTP and has no connection or request timeouts of its own; put a TLS reverse proxy with header, body and idle timeouts in front of it before binding beyond loopback. |
| `REPOMEMO_SERVICE_NAME` | `repomemo-server` | Name reported by `/health`. |
| `REPOMEMO_SOFFICE` | found automatically | Path to LibreOffice's `soffice` for layout-accurate Office previews. Unset: looked up on `PATH` and in the usual install folder; without it, text, table and slide previews are used. |
| `REPOMEMO_ALLOWED_ORIGIN` | `http://127.0.0.1:3021` | Browser origins allowed to call the API. Comma-separated for several; `*` is refused. |
| `REPOMEMO_SECRET_KEY` | unset | Key (32+ characters) used to encrypt AI provider API keys at rest. When unset, a random key is created in `<data dir>/secret.key`; back that file up with the database, or the saved keys must be entered again. |
| `REPOMEMO_ALLOW_REGISTRATION` | `false` | `true` opens sign-up to anyone who can reach the server; by default people join with invitation links. The very first account can still register, so a new server gets its owner. Email addresses are not verified, so keep it closed on a server others can reach. |
| `REPOMEMO_PUBLIC_URL` | unset | The address people use, such as `https://memo.example.com`. With `https://` the server may listen beyond loopback and sends `Strict-Transport-Security`. |
| `REPOMEMO_TRUST_PROXY` | `false` | Read the client address from `X-Forwarded-For` / `X-Real-IP`, but only when the TCP peer is loopback or listed in `REPOMEMO_TRUSTED_PROXIES`; the rightmost address no trusted proxy added wins. Also accepted as proof of a TLS proxy for non-loopback binds. |
| `REPOMEMO_TRUSTED_PROXIES` | unset | Comma-separated proxy addresses besides loopback whose forwarding headers are believed. |
| `REPOMEMO_ALLOW_INSECURE_HTTP` | `false` | Lets the server listen beyond loopback over plain HTTP with no proxy in front. For a trusted network only; without it (and without an `https://` public URL or proxy trust) the server refuses to start. |
| `REPOMEMO_MAX_UPLOAD_MB` | `10` | Largest request body (uploads). |
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
| `REPOMEMO_BACKUP_KEEP` | `7` | Backups kept; older ones are deleted after a new one. Also a run-time setting. |
| `REPOMEMO_VECTOR_INDEX_MB` | `512` | Memory for in-memory vector indexes of meaning search, in MiB. A workspace of 50,000 passages with 768-dimensional vectors takes about 150 MiB. `0` turns the indexes off (the database is scanned). A workspace that does not fit is scanned. |
| `REPOMEMO_DOCUMENT_TIMEOUT_SECONDS` | `60` | How long reading one document may take before it is skipped. `0` for no limit. |
| `REPOMEMO_HEAVY_REQUESTS_PER_MINUTE` | `240` | Searches, figures, map, health and file queries per person per minute; `0` is unlimited. Also a run-time setting. |
| `REPOMEMO_BACKGROUND_AI_PER_DAY` | `2000` | Image descriptions and embedding batches the server makes on its own per workspace per day (UTC); `0` is unlimited. Also a run-time setting. |
| `REPOMEMO_ACCESS_TOKEN_TTL_MINUTES` | `60` | Access-token lifetime (5–1440). |
| `REPOMEMO_REFRESH_TOKEN_TTL_DAYS` | `30` | Refresh-token lifetime (1–365). |
| `REPOMEMO_AUTH_REQUESTS_PER_MINUTE` | `30` | Sign-in and registration requests per client address (session refreshes get four times as many); `0` turns the limit off. |
| `REPOMEMO_LOGIN_MAX_FAILURES` | `10` | Consecutive failed sign-ins (per account and address) before a temporary lockout; the account is also locked after five times as many from all addresses. A device that still holds one of the account's sessions gets through. `0` turns it off. |
| `REPOMEMO_LOGIN_LOCKOUT_MINUTES` | `15` | Length of that lockout. |
| `REPOMEMO_AI_REQUESTS_PER_HOUR` | `120` | AI requests (Ask, overviews, summaries, assistant AI actions) per user per hour; `0` is unlimited. |
| `REPOMEMO_AI_ALLOWED_HOSTS` | `127.0.0.1,localhost,::1,openrouter.ai` | Comma-separated hosts AI providers may be configured at, such as `127.0.0.1,localhost,.openrouter.ai,llm.internal` (a leading dot allows subdomains). `*` allows every host. Link-local and cloud-metadata addresses are always refused, and names are resolved and checked at connect time. |
| `REPOMEMO_SMTP_ALLOWED_HOSTS` | unset | Mail servers allowed to be at private addresses, comma separated: a name or address, `.example.com` for a domain, or `*`. The mail server is set up in System › Settings › Mail; without this it must resolve to public addresses (link-local and cloud-metadata addresses are never allowed). |
| `REPOMEMO_SMTP_CA_FILE` | unset | A PEM file of certificate authorities trusted for the mail server besides the public ones, for a relay with a private certificate. |
| `REPOMEMO_REPO_ROOTS` | unset | Folders repository links must sit inside, separated like `PATH` (`;` on Windows, `:` elsewhere). **Unset turns repository links off.** Network shares are refused unless a root is on one. |
| `REPOMEMO_GIT_SAFE_DIRECTORIES` | unset | Further folders git may read although another account owns them, separated like `PATH`. The repository folders are included. The server's git ignores the account's own `git config`. |
| `REPOMEMO_SOFFICE_WRAPPER` | unset | A command LibreOffice is started through, split on spaces, with `{work}` (the conversion folder) and `{profile}` (its profile folder) replaced, for example `bwrap --ro-bind / / --bind {work} {work} --bind {profile} {profile} --unshare-net --die-with-parent`. Use it to run conversions without network and without the data folder. |
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

