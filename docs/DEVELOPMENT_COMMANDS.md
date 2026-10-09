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
reroute to an existing one; the folder is verified first. To skip the menu set
`REPOMEMO_SERVER_DATA_DIR=onboarding` (or `workspace-data/onboarding`): the value
is verified at startup, and anything outside `workspace-data/`, or a folder that
is neither empty nor a RepoMemo environment, stops the server. System
administrators can detach the environment from System › Settings. The server
exposes JWT-protected workspace routes. Import `docs/api/RepoMemo_Shared_API_v2.postman_collection.json` into
Postman and run its numbered folders in order to exercise authentication,
workspace setup, evidence, indexing, retrieval, team memory, and workspace
membership management. For the membership folder, register the teammate first,
then set its email in the `memberEmail` collection variable.

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
| `REPOMEMO_TOKEN` | unset | An access token to use instead of signing in, for scripts. |
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
| `REPOMEMO_ALLOWED_ORIGIN` | `http://127.0.0.1:3021` | Browser origins allowed to call the API. Comma-separated for several; `*` is refused. |
| `REPOMEMO_SECRET_KEY` | unset | Key (32+ characters) used to encrypt AI provider API keys at rest. When unset, a random key is created in `<data dir>/secret.key`; back that file up with the database, or the saved keys must be entered again. |
| `REPOMEMO_ALLOW_REGISTRATION` | `true` | `false` closes sign-up. The very first account can still register, so a new server gets its owner. |
| `REPOMEMO_TRUST_PROXY` | `false` | Read the client address from `X-Forwarded-For` / `X-Real-IP`. Only behind a reverse proxy that sets them. |
| `REPOMEMO_MAX_UPLOAD_MB` | `10` | Largest request body (uploads). |
| `REPOMEMO_ACCESS_TOKEN_TTL_MINUTES` | `60` | Access-token lifetime (5–1440). |
| `REPOMEMO_REFRESH_TOKEN_TTL_DAYS` | `30` | Refresh-token lifetime (1–365). |
| `REPOMEMO_AUTH_REQUESTS_PER_MINUTE` | `30` | Sign-in and registration requests per client address (session refreshes get four times as many); `0` turns the limit off. |
| `REPOMEMO_LOGIN_MAX_FAILURES` | `10` | Consecutive failed sign-ins (per account and address) before a temporary lockout; the account is also locked after five times as many from all addresses. `0` turns it off. |
| `REPOMEMO_LOGIN_LOCKOUT_MINUTES` | `15` | Length of that lockout. |
| `REPOMEMO_AI_REQUESTS_PER_HOUR` | `120` | AI requests (Ask, overviews, summaries, assistant AI actions) per user per hour; `0` is unlimited. |
| `REPOMEMO_AI_ALLOWED_HOSTS` | unset | Comma-separated hosts AI providers may be configured at, such as `127.0.0.1,localhost,.openrouter.ai` (a leading dot allows subdomains). Link-local and cloud-metadata addresses are always refused. |
| `REPOMEMO_REPO_ROOTS` | unset | Folders repository links must sit inside, separated like `PATH` (`;` on Windows, `:` elsewhere). Unset allows any local folder. Network shares are refused unless a root is on one. |
| `REPOMEMO_REPO_POLL_SECONDS` | `300` | How often linked repositories are checked for new commits and synced; `0` turns automatic syncing off. |
| `REPOMEMO_MAINTENANCE_INTERVAL_MINUTES` | `60` | Background maintenance (token and job cleanup, blob garbage collection, retries, database checkpoint); `0` turns it off. |
| `REPOMEMO_BLOB_GC` | `true` | Delete stored files no evidence references any more. |
| `REPOMEMO_JOB_RETENTION_DAYS` | `90` | Finished jobs kept this long; `0` keeps them forever. |
| `REPOMEMO_INDEX_RETRY_HOURS` | `6` | How often indexing that failed for good is retried. |
| `REPOMEMO_SYSTEM_ADMIN_EMAILS` | unset | Comma-separated account emails made **system administrators** at startup (and when they register). The first account created on a new server becomes one automatically; use this to name one on an existing server. |
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

Run the background worker foundation separately:

```powershell
cargo run -p repomemo-worker
```
