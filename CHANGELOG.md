# Changelog

What changed in each release, newest first, with what to do when upgrading. Versions follow [semantic versioning](https://semver.org): `0.2.0` is the next minor release (see the [roadmap](mindmap/ROADMAP.md#-road-to-v020)); the 0.2.x releases after it lead to `0.3.0` ([Road to V0.3.0](mindmap/ROADMAP.md#-road-to-v030), proposed). Releases before V0.1.63 are summarized in the roadmap's *Delivered* tables.

## V0.2.3 (2026-10-14)

The remaining web pages, on the query cache.

### Changed
- **No page loads its own data any more.** The dashboard, profile (with the tasks assigned to you), notifications, evidence detail (file, comments, lifecycle and its history, chunks) and memory card pages read their data through the cache that the workspace page already used. A page you opened before shows what it had at once and refreshes it in the background; details and personal pages always refetch when they open.
- **Live events refresh them.** The evidence page marks its file, comments or lifecycle stale when someone else changes them or indexing finishes, through the cache instead of its own fetches. The memory card page now listens to the workspace stream too: another person editing the card, or evidence changing (which can make a card *need review*), refreshes it once per burst, and a dropped connection refreshes it on reconnect.
- Entries of the evidence and memory card pages live under their workspace in the cache, so a full reload of the workspace page refreshes them with everything else, and the file list and member list are shared with the workspace page instead of fetched again.

### Added
- `staleTime` on `useServerState`, `accountKeys` for what belongs to a person rather than a workspace, and two tests of the cache (32 web tests).

### Upgrading
- Nothing to do.

## V0.2.2 (2026-10-14)

The workspace page, split.

### Changed
- **One component per section.** `SharedWorkspaceDetail` (1,089 lines) is now the shell (320 lines: live events, loading and the layout) and `src/web/workspace/` holds `OverviewSection`, `EvidenceSection`, `RetrievalSection`, `MemorySection`, `TasksSection`, `PeopleSection` and `SettingsSection` (the largest, evidence, is 371 lines). Sections read their server data through the hooks of `workspace/data.ts`, which share the query cache, and receive what every section needs (who is looking, what they may do, the busy flag, a reload) as one `WorkspacePage` object.
- Nothing changes on screen. Filters, drafts, results and the open folder are still there when a person leaves a section and comes back (`sectionState`); they start clean in another workspace, as the page already reset its folder there.
- Tasks and People read the member list when they open (it used to be fetched ahead for every section); the page still reads it ahead, so the list is there at once.

### Added
- Tests of the section state (30 web tests).

### Upgrading
- Nothing to do.

## V0.2.1 (2026-10-14)

Operations: the production guide and the server checks, corrected against the code.

### Changed
- **Server checks say what is missing.** The setup wizard's checks gain *Public address* (HTTPS, which turns HSTS and the `Secure` cookie on) and *Automatic backups*, and *Client addresses* warns when the address is HTTPS but `REPOMEMO_TRUST_PROXY` is not set, because every person would then share the proxy's address for limits and lockouts. Advice appears only where something is missing. The LibreOffice hint names `REPOMEMO_SOFFICE`.
- **The production guide** lists the checks, adds `REPOMEMO_BACKUP_INTERVAL_HOURS` to the settings that must be right, and drops the "to be followed in V0.1.76" status. Every variable, default, route and metric it names was compared with the code: no other difference was found.

### Upgrading
- Nothing to do. A server without HTTPS or backups shows two new warnings in the setup checks.
- The clean-machine install (R6) itself is still the maintainer's: anything it finds becomes the next fix.

## V0.2.0 (2026-10-14)

**Team-ready.** The first minor release: a server a team can install, expose and trust with its knowledge. It is V0.1.76 with the version number changed; no feature was added after the candidate and the candidate's checks found nothing to fix.

What 0.2.0 promises, and the release that delivered each part:

- **Secure by default** (V0.1.64–V0.1.69, V0.1.76): registration is closed unless an operator opens it, invitations onboard people, refresh tokens live in an `HttpOnly` cookie, the browser runs under a strict content security policy, the server only reaches hosts it is told about, and three security passes are closed in writing.
- **Checked on every commit** (V0.1.66, V0.1.71): CI, one version source, formatting, lints and dependency audits.
- **Deployable and recoverable** (V0.1.75): a production guide, backups that restore into a new environment, quotas and a disk floor, OpenMetrics.
- **Memory stays true** (V0.1.72, V0.1.73): cards that say when their evidence changed, search that can put trusted evidence first, symbol search, ownership transfer.
- **Changeable** (V0.1.70, V0.1.71): the web client and the server are split into modules with tests.
- **Fast enough to trust** (V0.1.74, V0.1.76): measured at 50,000 passages in a release build.
- **Upgradable** (V0.1.76): environments from V0.1.40 and later upgrade in place; see [UPGRADING.md](UPGRADING.md).

### Upgrading
- Nothing beyond [UPGRADING.md](UPGRADING.md). Entries V0.1.66–V0.1.76 below are the detailed release notes of this version.
- The version is `0.2.0` in `Cargo.toml`, both `package.json` files, `tauri.conf.json` and the lock files (`npm run version:check`). The tag `v0.2.0` is created by the maintainer after the clean-machine install (R6); until then the build is the release candidate.

## V0.1.76 (2026-10-14)

Release candidate for 0.2.0. **Feature freeze:** from here to 0.2.0 only fixes from this candidate go in.

### Security
- **Third security pass** over everything changed since V0.1.65 ([review §12](mindmap/security/2026-10-10-security-review.md#12-third-pass-2026-10-14-release-candidate-v0176)): no high and no medium finding. Two low findings are fixed: **repository syncs obey the storage allowance and the disk floor** (a linked repository could grow a workspace past them), and **the key file in a backup is owner-only on Windows**. Four low-impact items are accepted in writing.
- The review's status table is closed: every finding is fixed, mitigated or accepted.

### Added
- **An upgrade test.** The schemas of V0.1.40, V0.1.52 and V0.1.65 are rebuilt from the migration files, filled with data, and opened by the current engine: migrations apply, data and full-text search survive, and the integrity and foreign key checks pass. It runs with `cargo test`, so CI runs it.
- **Performance runs** for the map and health (`cargo test -p repomemo-api --release at_scale -- --ignored --nocapture`) next to the storage one. Results at 50,000 passages, release build: meaning search p95 112 ms (first search, which builds the index, 6.0 s), keyword search p95 63 ms, figures p95 6 ms, file query 1.4 ms, knowledge map 0.5 s and health 0.4 s on first load.
- **Documentation:** [UPGRADING.md](UPGRADING.md), the generated [API reference](mindmap/technical/api-reference.md) (`node scripts/list-routes.mjs`) and the [authentication and authorization](mindmap/technical/auth-and-authorization.md) page.
- `node scripts/smoke-test.mjs <address>` checks a running server the way a clean install should behave (health, headers, the web app, and with an administrator's credentials the overview, metrics and a backup).

### Upgrading
- Nothing to do beyond [UPGRADING.md](UPGRADING.md). Repository syncs are refused, with the reason in the repository's activity, while the workspace is at its storage allowance or the disk is under its floor.

## V0.1.75 (2026-10-14)

Operations: what an operator needs before trusting the server.

### Added
- **Backup and restore.** System › Jobs & maintenance › **Back up now**, the console (`backup list`, `backup create --yes`, `backup restore <backup> <environment> --yes`, `backup delete <backup> --yes`) and `GET/POST /v1/system/backups`, `DELETE /v1/system/backups/{name}`, `POST /v1/system/backups/{name}/restore`. A backup is a folder in `<data folder>/backups/` with a consistent snapshot of the database (`VACUUM INTO`, taken while the server runs), the stored files, the key file and a manifest; each backup links the files the one before it had, so only new files cost space. A restore never touches the running environment: it copies a backup into a **new environment folder**, opens it to prove it works and says how many workspaces, files and people it holds; attach it from the environment menu.
- **Automatic backups.** `REPOMEMO_BACKUP_INTERVAL_HOURS` (0, off, by default; also a setting) makes the maintenance loop back up by itself when the newest backup is older, and `REPOMEMO_BACKUP_KEEP` (7) bounds how many are kept.
- **Quotas that protect the disk.** Storage per workspace (`REPOMEMO_WORKSPACE_STORAGE_MB`, 20,480 MiB: `413 workspace_full`), uploads per person per hour (`REPOMEMO_UPLOADS_PER_HOUR`, 600: `429`), workspaces owned per person (`REPOMEMO_WORKSPACES_PER_PERSON`, 25: `409 workspace_limit`) and a free-disk floor (`REPOMEMO_DISK_FLOOR_MB`, 1,024 MiB: uploads answer `507 disk_low`). All are settings in System › Settings (group *Limits*); `0` turns one off. System › Overview shows free disk space and the allowances.
- **Metrics.** `GET /v1/system/metrics` in the OpenMetrics text format: version, uptime, requests and errors, accounts, workspaces, files, passages, vectors, jobs by status, database and log size, free disk, queues, vector index memory, abandoned document readings, backups and the time of the last one. It needs a system administrator's session or a personal access token with the `system` scope.
- **A production guide**, [mindmap/technical/operations.md](mindmap/technical/operations.md): the settings that must be right, nginx and Caddy configurations with timeouts and event streams, systemd and Windows service definitions, readiness checks, limits, backups and the restore drill, monitoring with alerts, the LibreOffice sandbox, upgrades and a troubleshooting table.

### Upgrading
- Nothing changes in the database. Uploads over a workspace's 20 GiB allowance, or while less than 1 GiB is free, are refused from now on: raise the settings or set them to `0` for the old behaviour.
- Backups are kept next to the data, so copy the `backups` folder to another machine (the guide shows how). They hold `secret.key`: protect them like the data.

## V0.1.74 (2026-10-14)

Performance and robustness.

### Added
- **A dashboard in one request.** `GET /v1/dashboard` returns the figures of every workspace the person can open (files, active tasks, memory cards). The dashboard page used to ask for the full metrics of each workspace.
- **Limits on heavy requests.** Search, symbol search, figures, the knowledge map, Workspace Health and the file query each allow a person `REPOMEMO_HEAVY_REQUESTS_PER_MINUTE` requests a minute (240 by default, `0` is unlimited; also a setting in System › Settings). Beyond it the answer is `429` with `Retry-After`.
- **A daily allowance for background AI.** Image descriptions and embedding batches the server starts on its own are counted per workspace per day (`REPOMEMO_BACKGROUND_AI_PER_DAY`, 2,000 by default, `0` is unlimited; also a setting). A workspace that used it up pauses until midnight UTC: embedding resumes by itself, an image waits for the next indexing retry, and the job says why.
- **A time budget for document readers.** Reading a Word, Excel, PowerPoint, PDF, OneNote or Outlook file stops waiting after `REPOMEMO_DOCUMENT_TIMEOUT_SECONDS` (60 by default, `0` for no limit); the file is reported as failed with the reason.
- **`REPOMEMO_VECTOR_INDEX_MB`** (512 by default, `0` turns it off) sets the memory for in-memory vector indexes. System › Overview lists it with the document time limit.
- A performance run for operators: `cargo test -p repomemo-storage --release performance_at_scale -- --ignored --nocapture` builds a workspace of `REPOMEMO_PERF_PASSAGES` passages (50,000 by default) and prints search, figures and query timings.

### Changed
- **Meaning search no longer scans the database.** The vectors of a workspace are kept in memory as one matrix, built on the first search, kept current by the embedding queue and checked against the database on every search. A workspace whose vectors do not fit `REPOMEMO_VECTOR_INDEX_MB` is scanned as before.
- **Figures and the file query are computed by the database.** `GET …/metrics` uses SQL aggregates instead of loading every file; `POST …/artifacts/query` filters, counts and pages in SQL (`X-Total-Count` when paged). Text matching ignores case for ASCII letters only (it used to ignore case for every script).
- **The knowledge map and Workspace Health are kept while a workspace is unchanged.** They are recomputed when files, passages, vectors, memory cards, links, evidence states, health decisions or providers change, and at the latest after ten minutes.
- File links for "Open in Office" are valid for **60 seconds** (they were 5 minutes) and belong to the client that first used them (see Security).

### Security
- **Heavy endpoints are rate limited and cached, parsers have a time budget (M11).** A reading that overruns is abandoned, not killed: it ends by itself, and new readings are refused while four abandoned ones are still running. There is no memory cap on a reading.
- **Background AI is counted (M12).** See the daily allowance above.
- **File links are not reusable (L1).** A link works for 60 seconds, for the first client that fetches it (the same client may repeat the request for 30 seconds, as Office apps do), and its path is replaced by `/v1/shared-files/[link]/name` in request logs.
- **`secret.key` is owner-only on Windows (L5).** The folder's inherited permissions are removed and only the account running the server keeps access (`icacls`); on Unix an older file with wider permissions is set to `0600`. Failing to do so is logged, and `REPOMEMO_SECRET_KEY` avoids the file altogether.

### Upgrading
- Links to open a file in Office that were issued before the upgrade stop working; open the file again.
- A backup or service account other than the server's own can no longer read `secret.key` on Windows. Give it access, or keep the key in `REPOMEMO_SECRET_KEY`.
- Scripts that make more than 240 searches or reports a minute as one person need a higher `REPOMEMO_HEAVY_REQUESTS_PER_MINUTE`.
- A workspace that embeds more than 2,000 batches (about 64,000 passages) in one day pauses until the next day; raise `REPOMEMO_BACKGROUND_AI_PER_DAY` or set `0`.

## V0.1.73 (2026-10-14)

Evidence, ownership and repositories, phase 1b.

### Added
- **Ownership transfer.** An owner can hand a workspace (People) or an organization (Organization administration) to another member with **Make owner**; the previous owner stays on as an administrator. Recorded in the workspace activity and the System audit. `POST /v1/workspaces/{ws}/transfer-ownership` and `POST /v1/organizations/{org}/transfer-ownership`, from a signed-in session.
- **Folders can be moved** (`POST /v1/workspaces/{ws}/folders/{f}/move`, menu *Move to folder…*), not into themselves or below themselves, within the five levels, and not onto a name that is taken.
- **Several files at once.** *Select files* in the Evidence ledger moves, marks verified, needs review or outdated, or deletes the chosen files together (`POST /v1/workspaces/{ws}/artifacts/bulk`, up to 200). Files that cannot be handled (repository files, files already gone) are reported and do not stop the others.
- **New version of a file** (`POST /v1/artifacts/{a}/versions`, *New version* on the file page): the content is replaced in place, so comments, folder, state and the memory cards that cite it stay; passages whose text did not change keep their identity and their links. It must be the same kind of file.
- **Repository renames with edits.** A file renamed and edited in one commit stays the same artifact (git's rename detection, `-M50%`), with its comments, memory links and history.
- **The commit on results and citations.** Search results, citations and Ask answers for a repository file say which commit their text is from.
- **Console:** `users reset-link <email> --yes`, `tokens list [--user email]`, and `logs --follow` (also `--after <sequence>`).

### Security
- **Nobody can leave an organization while owning one of its workspaces (N8):** removing such a member answers `409` and names the workspaces.

### Upgrading
- No data is changed by this release. A workspace that an earlier removal left without an owner stays as it is: ownership can only be handed over by an owner.

## V0.1.72 (2026-10-14)

Memory and search you can trust.

### Added
- **Memory cards say when their evidence moved.** A card whose cited passage was changed or removed, whose cited file was marked outdated or superseded, or whose cited file was updated after the card was written shows **Needs review** in the list and a panel with the reasons on the card, where it can be confirmed as it is. Workspace Health has a new detector, *Stale memory* (create task, dismiss).
- **Evidence of a card can be edited.** `PUT /v1/memory-cards/{id}` takes `citations`; the edit form lists what the card cites with remove and add.
- **Search by trust.** Search takes an evidence state, a folder (and the folders below it) and an updated-between date range. Outdated and superseded evidence is hidden unless asked for, in Search, Ask and the assistant alike, and verified evidence ranks a little higher (10 %, never above 1).
- **Code symbols on the web.** `GET /v1/workspaces/{ws}/symbols?q=` and a *Code symbols* box on the Retrieval page; the assistant answers "where is parse_config defined" and "find the function …" from the same table.
- **Live Map and Health.** Both pages refresh themselves, once per burst, when evidence, memory or repositories change or a job finishes.

### Changed
- `SearchRequest` has `lifecycle_statuses`, `folder_id`, `modified_after` and `modified_before` (all optional); `MemoryCardSummary.needs_review` and `MemoryCardDetail.review` are new fields.

### Upgrading
- Search no longer returns passages of files marked outdated or superseded by default. Ask for them with the evidence state *Outdated and superseded* or *Every state*.

## V0.1.71 (2026-10-14)

Server foundation: the base the product work after it builds on. Behaviour changes are limited to the items below.

### Changed
- **The server is split by area.** `apps/server/src/lib.rs` (9,000 lines with its tests) is the router and state, with the handlers in `auth`, `access_control`, `profile`, `organizations`, `workspaces`, `artifacts`, `collaboration`, `retrieval`, `ai`, `streams`, `requests`, `errors` and `config`, and its tests in `tests/`. No module is over 1,500 lines.
- **The storage crate is split by area.** `crates/storage/src/lib.rs` (6,200 lines) keeps the engine, the row types and the helpers; its methods live in `users`, `organizations`, `activity`, `collaboration`, `artifacts`, `search`, `memory`, `jobs` and `providers`.
- **Typed errors.** What a person caused (a record that does not exist, input that is not acceptable, a name that is taken) is created with `not_found!`, `invalid!` and `conflict!` in `repomemo-domain` and carries its kind. The HTTP layer answers from the kind, no longer from the words of the message, and every other failure is a generic 500 with the detail in the log. Handlers that returned storage errors as 400 (token creation, folders) now answer with the right status (review L6).
- **One writer connection and a pool of readers.** Writes and transactions go through a single SQLite connection (writers queue instead of failing with "database is locked"); reads use their own read-only connections (`REPOMEMO_DB_READ_CONNECTIONS`, default 4) and never wait for a write.
- Indexing and embedding concurrency are settings: `REPOMEMO_INDEX_CONCURRENCY` (default 2) and `REPOMEMO_EMBEDDING_CONCURRENCY` (default 1). System › Overview lists them.
- **The code base is formatted with rustfmt and clean under clippy** (`-D warnings`), and CI fails otherwise. License and duplicate-crate checks run in CI as well.
- `apps/worker`, a placeholder that did nothing, is removed.
- **Dependencies.** `quick-xml` 0.41 in the document readers closes two advisories about hostile XML (RUSTSEC-2026-0194, -0195), and `cargo update` brings `anyhow`, `rustls` and the rest to their patched releases, so `cargo deny` is clean for the whole workspace. One advisory is ignored with its reason in `deny.toml`: `rsa` is linked by `jsonwebtoken` but unused, since tokens are HS256.

### Security
- **Request timeouts (N7).** Every route except the live event streams answers within `REPOMEMO_REQUEST_TIMEOUT_SECONDS` (default 120; `408` after that). A client has `REPOMEMO_HEADER_TIMEOUT_SECONDS` (default 30) to send its request headers, which ends slow-header connections. One account can hold `REPOMEMO_MAX_EVENT_STREAMS_PER_USER` live streams (default 10; `429` after that). Part of M11.

### Upgrading
- A request that takes longer than two minutes (a very large upload over a slow link) needs a higher `REPOMEMO_REQUEST_TIMEOUT_SECONDS`.
- Scripts that matched error texts of the API should use status codes and the `error.code` field.

## V0.1.70 (2026-10-14)

Web client foundation: the base every later product release builds on. No change in what the app does.

### Changed
- **`SharedWebApp.tsx` is split by page.** The 2,800-line file is now the app shell (about 340 lines) plus modules under `src/web/`: the dashboard and workspace home, profile, notifications, organization pages, workspace detail, artifact and memory card pages, shared constants, formatting helpers and the router.
- **A typed router** (`src/web/router.ts`): `parseRoute` turns an address into a `Route` (workspace and section, artifact, memory card, System page, not found) and `routePath` builds it back, instead of matching `pathname.split("/")` by hand.
- **Server state lives in a cache** (TanStack Query, `src/lib/queryClient.ts`). A workspace page shows cached data at once and refreshes it in the background. Live events and mutations mark only what they touched as stale (ledger, memory cards, tasks, figures), so one change no longer refetches the whole page; a stream resync refetches everything once.
- **Heavy panels load on demand:** the System area, the rich note editor, the knowledge map, the document viewer, workspace health and the repository pages are separate bundles fetched when first shown.
- **Types from the Rust source.** `npm run types:generate` writes `src/generated/domain.ts` from `crates/domain`; `src/types.ts` re-exports the 70 types that match and keeps the 28 the client shapes differently. CI fails when the generated file is out of date (`npm run types:check`).
- The local desktop app is labelled **Local edition**: it is frozen, with security fixes only (roadmap decision 3).

### Added
- **Web client tests** (Vitest and Testing Library, `npm run test:web`, in CI): session renewal through the cookie, live event parsing, Markdown safety (no images, link targets), image preview rules, the upload file filter, routes, and the query cache.


Host exposure (bundle S3): the last two high findings.

### Security
- **AI providers only where the operator allows (H2).** Without `REPOMEMO_AI_ALLOWED_HOSTS`, a provider may sit only on this machine (`127.0.0.1`, `localhost`, `::1`) or on `openrouter.ai`. List your own hosts to allow others, or `*` to allow every host (link-local and cloud-metadata addresses stay refused). Names are resolved by the server and every address is checked at connect time, so a name cannot be re-pointed at the server's own network after it was accepted; with the built-in list a name must resolve to a public address.
- **Provider error bodies are not echoed (H2).** What a provider answers with stays in the server log (System › Logs, AI category); the message shown has the status and a hint only.
- **Repository links are off until the operator names folders (H3).** A server without `REPOMEMO_REPO_ROOTS` refuses every link and sync with one message; outside the configured folders the answer is the same whether or not the path exists.
- **Git runs isolated (L8).** On a server, git ignores the system and global configuration, allows no transport protocol and does not recurse into submodules. The repository folders, and folders in `REPOMEMO_GIT_SAFE_DIRECTORIES`, are exempt from git's ownership check.
- **LibreOffice runs without the server's secrets (M10).** Every `REPOMEMO_*` variable and anything named like a key, token or password is removed from its environment, and `REPOMEMO_SOFFICE_WRAPPER` runs it inside a sandbox command of the operator's (no network, no data folder). System › Overview and the server check show its version and whether it is sandboxed.

### Upgrading
- Provider URLs outside this machine and OpenRouter stop working until listed in `REPOMEMO_AI_ALLOWED_HOSTS`. A provider on another machine in your network must be listed by name.
- Repository links stop working (and linked repositories stop syncing) until `REPOMEMO_REPO_ROOTS` names the folders that hold them.
- Git no longer reads the service account's `git config --global`: a `safe.directory` set there is ignored. Use `REPOMEMO_GIT_SAFE_DIRECTORIES`. Git 2.32 or later is needed for the isolation to be complete.

## V0.1.68 (2026-10-14)

Browser hardening (bundle S4).

### Security
- **The refresh token is out of reach of scripts (M2).** The web app's refresh token travels in an `HttpOnly; SameSite=Strict` cookie (`repomemo_refresh`, path `/v1/auth`, `Secure` when `REPOMEMO_PUBLIC_URL` is HTTPS) instead of `localStorage`. Only requests with `X-RepoMemo-Client: web` are affected; the console and scripts still use the token in the response body. The web app and the API must share a site (one host behind the proxy; different ports are fine). Tokens kept by an earlier version are exchanged once on the next visit.
- **A content security policy for the built web app (M2).** `npm run build` puts it in `index.html` and writes the header value, with `frame-ancestors 'none'`, to `dist/content-security-policy.txt` for the host to send: scripts from the app only, requests to itself and the API, images from itself, `blob:` and `data:`, frames only for `blob:` previews, no plugins.
- **A strict policy for the desktop shell (M6)** in `tauri.conf.json`: its own files and the local IPC only, no remote connections.
- **No remote images in Markdown (M5).** AI answers, notes, memory cards and repository summaries show an image as a note with its text and address instead of fetching it, so a prompt-injected image cannot send workspace content out in a URL.
- **External links show where they go (L9).** The target host is shown next to the link, which opens in a new tab with `noopener noreferrer`.
- **Invitation and reset secrets leave the address bar (L3).** The link page reads the fragment once and removes it from the address and the history entry.

### Changed
- The API allows credentialed cross-origin calls from the origins in `REPOMEMO_ALLOWED_ORIGIN` (needed for the cookie when the web app is on another port).

### Upgrading
- Serve the web app and the API from one host (a reverse proxy routing `/v1` and `/health` to the server), or at least one site; a cookie-less setup across different sites can no longer renew sessions.
- Send the header from `dist/content-security-policy.txt` from the host that serves the web app, and rebuild the web app with `VITE_REPOMEMO_API_URL` set so the policy names your API.

## V0.1.67 (2026-10-14)

Safe defaults and identity (bundle S2).

### Security
- **Registration is closed by default (M1).** `REPOMEMO_ALLOW_REGISTRATION` now defaults to `false`; people join with invitation links, and the first account of a new server can still register. `/health` no longer says whether registration is open (I2).
- **Adding someone no longer reveals whether they have an account (M4).** The member forms add only people who are already in the organization (or workspace); anyone else gets the same `409 invitation_required` answer, whether or not the address has an account, and the web app offers an invitation link. Nobody is added to an organization or workspace without accepting.
- **App administrators read, system administrators change (M8).** App administrators keep the System pages, jobs and maintenance. Settings, roles, other people's sessions, lockouts, reset links, raw log files and detaching the environment need a system administrator, and workspace activity records are hidden from app administrators in the logs.
- **A `system` scope for personal access tokens (N2 residual).** Only administrators can create one. Changes in the System area (sign people out, unlock, run maintenance, console `--yes`) need a signed-in session or a `system` token; read-and-write tokens no longer reach them. Migration 0024.
- **Transport checks (M9).** The server refuses to listen beyond loopback unless `REPOMEMO_PUBLIC_URL` is `https://…`, `REPOMEMO_TRUST_PROXY` is on, or `REPOMEMO_ALLOW_INSECURE_HTTP` says the network is trusted. An HTTPS public address turns on `Strict-Transport-Security`. `repomemo-console` warns when it talks plain `http://` to another machine.
- **Client addresses behind a proxy (M3).** `X-Forwarded-For` counts only when the TCP peer is loopback or listed in `REPOMEMO_TRUSTED_PROXIES`, and is read from the right, so a client cannot choose its own address.
- **Lockouts no longer lock owners out (L10).** A sign-in that carries a session this browser still holds gets past a lockout other people caused.
- **The account hears about reset links (L4):** a notification when an administrator creates one and another when it is used.
- **Weak passwords refused (L7):** the most common long passwords, passwords of fewer than five different characters, and passwords containing the email name.

### Changed
- The member forms say "is not in this organization yet. Create an invite link for them below." instead of asking whether an account exists.

### Upgrading
- A server that relied on open registration must set `REPOMEMO_ALLOW_REGISTRATION=true` (or turn it on in System › Settings). A value saved in System › Settings is kept as it was.
- A server bound beyond loopback over plain HTTP stops at startup until it has `REPOMEMO_PUBLIC_URL`, `REPOMEMO_TRUST_PROXY` or `REPOMEMO_ALLOW_INSECURE_HTTP=true`.
- Scripts that added existing accounts straight to a workspace must create an invitation instead.
- Scripts that ran System changes with a read-and-write token need a `system` token.

## V0.1.66 (2026-10-13)

Checkable releases: the first step of the road to V0.2.0.

### Added
- **Continuous integration** (`.github/workflows/ci.yml`) on every push to `main` and every pull request. Blocking: versions agree, Rust tests on Linux and Windows, the web type check, and security advisories for the server's dependencies (`cargo deny`, policy in `deny.toml`) and the web client's production dependencies (`npm audit`). Reported but not yet blocking: rustfmt and clippy, licenses and duplicate crates, and advisories that only reach the desktop shell.
- **One version source.** `npm run version:set -- <version>` (`scripts/set-version.mjs`) sets `Cargo.toml`, `Cargo.lock`, both `package.json` files, `package-lock.json` and `tauri.conf.json` together; `npm run version:check` (also in CI) fails when they disagree.
- **Performance timings.** Search, Ask, workspace metrics, the artifact query, the knowledge map and health checks record how long they took and how much they handled (`timing` target), with the workspace. Turn on **System › Settings › Logging › Performance** at *debug* to see them in System › Logs, filterable by workspace. New setting: `log_performance_level`.
- This changelog.

### Changed
- The version reported by System › Overview and the console is correct again: it had stayed at 0.1.60 since V0.1.60.
- `PRODUCT.md` describes the self-hosted team product instead of the desktop-only one. `docs/IMPLEMENTATION_TRACKER.md` is retired in favour of the roadmap and this changelog.

## V0.1.65 (2026-10-13)

The security second pass ([review §11](mindmap/security/2026-10-10-security-review.md#11-second-pass-2026-10-13)).

### Security
- **Stored XSS through image previews closed (N1, high).** Stored and served content types now come from the file name, never from the uploader (also I3); HTML, scripts and unknown types are served as plain text. Image previews open in a tab of their own only for raster images.
- Run-time settings changes need a signed-in session; personal access tokens can still run dry runs (N2).
- A workspace or organization can no longer lose its owner through the member list or an organization role change (N3).
- Someone downgraded to viewer can no longer edit or delete their earlier comments or tasks; saved searches are deleted by their author or an administrator (N4).
- Using a password reset link also revokes the account's personal access tokens (N5).
- `POST /v1/workspaces/{ws}/index` (synchronous, whole workspace) is for administrators (N6).

### Upgrading
- Scripts that changed settings with a personal access token must use a signed-in console (`repomemo-console` with a password) instead.

## V0.1.64 (2026-10-13)

Security ship-blockers for onboarding (bundle S1).

### Security
- Adding someone to one workspace, directly or by invitation, gives that workspace only; their organization shows on their dashboard as a guest entry (H1).
- Registering an address listed in `REPOMEMO_SYSTEM_ADMIN_EMAILS` grants nothing; only accounts that exist at startup are promoted (H4).
- What personal access tokens may reach is marked on the handlers instead of a path list (L2).
- Invitation and reset links carry their secret in the URL fragment (`/invite#…`, `/reset-password#…`) and the web app sends no referrer; older path links still open (L3).
- Only `VITE_` and `TAURI_ENV_` environment variables reach the web bundle (M7).

### Upgrading
- People added to a single workspace before V0.1.64 kept the organization membership they received then, and with it every workspace of the organization. Review organization members once and remove anyone who should only see one workspace.

## V0.1.63 (2026-10-13)

Onboarding without an account (migration 0023).

### Added
- **Invitation links** for one email address and role, single-use, valid 7 days, withdrawable, usable even when registration is closed.
- **Password reset links** issued in System › Users, one-time, valid 24 hours.
- **Personal access tokens** (`rmp_…`, read-only or read and write, 7–365 days) for scripts, CI and `repomemo-console` (`REPOMEMO_TOKEN`).
