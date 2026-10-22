# RepoMemo security and backend review — 2026-10-21 (V0.2.129, the fifth pass)

> **Scope:** the whole server (`apps/server`, the crates), the web client where it meets the API, CI and the container files, at `V0.2.129` (commit `f1550cd`). Three questions: which dependencies carry known advisories; which findings of the [review of 2026-10-14](2026-10-14-security-review.md) are still open (nothing in the changelog or the roadmap tracked them to closure); and what a fresh pass over the backend finds for security, logic, hardening and speed.
> **Method:** `cargo deny --workspace check advisories`, `npm audit` (with and without development packages) and `osv-scanner` over `Cargo.lock` and `package-lock.json`, run on 2026-10-21; then a static reading of the code. `cargo update --dry-run` confirmed which Rust fixes are lockfile-only. **Nothing was built, started or attacked**, as this repository's rules ask. Line numbers are as of `f1550cd`.
> **Status:** open. **2 high, 9 medium, 15 low, 4 info**, fixed so far: S5-1 (V0.2.130), S5-2 (V0.2.131), S5-10 (V0.2.132), S5-3 (V0.2.133, V0.2.134), E17 (V0.2.135), S5-7 (V0.2.136), E1 in part (V0.2.137, V0.2.138), E19 (V0.2.138), E2 (V0.2.139), E5 and E15 (V0.2.140), E6 (V0.2.141), E9 (V0.2.142), E3 and E4 (V0.2.143), S5-4 and E14 (V0.2.144), E11 (V0.2.145), E12, E13 and E18 (V0.2.146), E10 and E16 (V0.2.147), S5-8 and E1 (V0.2.148), E7 (V0.2.149, V0.2.150); eleven performance findings beside them. The releases that close them are planned as tracks L and M of the [Road to V0.3.0](../ROADMAP.md#track-l-security-and-dependencies-v02130v02148).
> **Previous review:** [2026-10-16](2026-10-16-security-review.md) (the fourth pass), closed. Its surfaces were not re-read, apart from the container.

| ID | Severity | Finding | Status | Release |
|---|---|---|---|---|
| E1 | **High** | LibreOffice converts every Office upload as soon as it is installed: no opt-in, no minimum version, no locked profile. The container variant puts it inside the server's container, with the network and the data folder | Fixed in V0.2.137, V0.2.138 and V0.2.148 | V0.2.137, V0.2.138, V0.2.148 | V0.2.137, V0.2.138, V0.2.148 |
| E2 | **High** | AI provider addresses reach any port and path of this machine (the built-in list allows `127.0.0.1` and `localhost` with any port), and the provider test tells an open port from a closed one | Fixed in V0.2.139 | V0.2.139 |
| S5-1 | Medium | `rustls` 0.23.45, `rustls-webpki` 0.103.15 and `webpki-roots` 1.0.9 carry six advisories (a pre-authentication 1 GiB allocation, a key-exchange downgrade, distrusted intermediates accepted). CI's blocking dependency job fails on `main` today | Fixed in V0.2.130 | V0.2.130 |
| S5-4 | Medium | Two tabs that refresh at the same moment sign the person out **everywhere**: the second tab presents the cookie the first just rotated, which reads as token theft | Fixed in V0.2.144 | V0.2.144 |
| E3 | Medium | Every verified account can create an organization and become its owner | Fixed in V0.2.143 | V0.2.143 |
| E4 | Medium | Any workspace administrator can link any local repository under `REPOMEMO_REPO_ROOTS`, other teams' included | Fixed in V0.2.143 | V0.2.143 |
| E5 | Medium | A read-and-write personal access token can invite, change roles, delete workspaces, repoint AI providers, link repositories, set webhooks, export and import | Fixed in V0.2.140 | V0.2.140 |
| E6 | Medium | The live event stream sends every reader the activity feed the API keeps for owners and administrators | Fixed in V0.2.141 | V0.2.141 |
| E7 | Medium | Ten artifact handlers load the full artifact, which re-reads a document's text in a reader process **on an async worker thread**, only to find its workspace, **before** checking the caller's rights | Fixed in V0.2.149 (rights on the summary first) and V0.2.150 (the page's text from stored passages, read off the runtime) | V0.2.149, V0.2.150 |
| E9 | Medium | An organization role change re-adds the person to every workspace of the organization, undoing a workspace's own removals and downgrades | Fixed in V0.2.142 | V0.2.142 |
| S5-5 | Medium | Every authenticated JSON route reads a body up to the upload limit (10 MiB by default, up to 1 GiB) | New | V0.2.152 |
| E8 | Low | A single-file re-index starts one background task per request, with no deduplication (the whole-workspace pass answers `409` since V0.2.6) | Partly fixed | V0.2.151 |
| E10 | Low | `//host` and `\\host` links in Markdown are shown without their host and open in the same tab | Fixed in V0.2.147 | V0.2.147 |
| E11 | Low | Per-address limits count each IPv6 address apart; password hashing has no global cap; limiter maps grow without bound for an hour | Fixed in V0.2.145 | V0.2.145 |
| E12 | Low | The sign-in address is logged with `%` (`Display`), so a line break forges a line in the text console log | Fixed in V0.2.146 | V0.2.146 |
| E13 | Low | Addresses are checked for an `@` only; display names take control and bidirectional characters. Mail headers are safe since V0.2.9 (`compose` refuses control characters) | Fixed in V0.2.146 | V0.2.146 |
| E14 | Low | The refresh cookie has no `__Secure-` prefix | Fixed in V0.2.144 | V0.2.144 |
| E15 | Low | Revoking a personal access token does not end the event streams it opened (deactivation does) | Fixed in V0.2.140 | V0.2.140 |
| E16 | Low | *Open in Office* is offered for macro-enabled formats (`.xlsm`, `.docm`, `.pptm`) | Fixed in V0.2.147 | V0.2.147 |
| E17 | Low | GitHub Actions pinned to moving tags; container base images too | Fixed in V0.2.135 | V0.2.135 |
| E18 | Low | Local git commands inherit the server's secrets in their environment (remote ones start empty) | Fixed in V0.2.146 | V0.2.146 |
| S5-2 | Low | `zip` 8.6.0 (through `calamine`) has three extraction advisories. RepoMemo never calls `extract()`, so they are not reachable, but they fail CI | Fixed in V0.2.131 | V0.2.131 |
| S5-3 | Low | Ten advisories in development packages (Vite 5, Vitest 3 and `tinypool`, PostCSS, nanoid, browserslist, source-map-js, esbuild). The production bundle is clean | Fixed in V0.2.133 and V0.2.134 | V0.2.133, V0.2.134 |
| S5-7 | Low | The container's git (Debian bookworm, 2.39.5) is refused by the remote-clone check, so remote repositories cannot work in the official image | Fixed in V0.2.136 | V0.2.136 |
| S5-8 | Low | The server container runs with a writable root filesystem, every default capability and no `no-new-privileges` | Fixed in V0.2.148 | V0.2.148 |
| S5-9 | Low | Any workspace writer can cancel any job of the workspace, an administrator's whole-workspace re-index included | New | V0.2.151 |
| E19 | Info | Rendered previews are cached server-wide by content hash, which tells a user whether the same Office file exists elsewhere | Fixed in V0.2.138 | V0.2.138 |
| S5-10 | Info | `sqlx` links a TLS stack (`rustls`, `webpki-roots` 0.26) that SQLite never uses | Fixed in V0.2.132 | V0.2.132 |
| S5-11 | Info | Unmaintained crates with no fixed release: `ttf-parser` (through `pdf-extract`, on the server), and `zvariant`, `zbus_names`, `glib`, `proc-macro-error` (Tauri on Linux, desktop only) | Tracked | — |
| S5-12 | Info | The web app keeps its access token (60 minutes) in `sessionStorage`; the strict content security policy is what keeps a script from reading it | Accepted | — |

| Severity | Count | IDs |
|---|---:|---|
| Critical | 0 | — |
| High | 2 | E1, E2 |
| Medium | 9 | S5-1, S5-4, S5-5, E3, E4, E5, E6, E7, E9 |
| Low | 15 | E8, E10–E18, S5-2, S5-3, S5-7, S5-8, S5-9 |
| Info | 4 | E19, S5-10, S5-11, S5-12 |

E20 (server paths shown to readers) and E21 (folder permissions) of the 2026-10-14 review are documentation items; E22 is replaced by §1 below.

---

## 1. Dependencies

### Rust (`cargo deny --workspace check advisories`: **failed**)

| Crate | In use | Advisory | Fixed in | Reached by |
|---|---|---|---|---|
| `rustls` | 0.23.45 | RUSTSEC-2026-0346 (`Debug` shows TLS secrets), -0347 (data sent to the outer server after an ECH rejection), **-0348 (Brotli certificate decompression allocates up to 1 GiB before authentication)**, **-0349 (an unauthenticated ServerHello downgrades later connections to classical key exchange)** | 0.23.46 | Every outbound call: AI providers, SMTP, connectors, the desktop client |
| `rustls-webpki` | 0.103.15 | **RUSTSEC-2026-0382 (distrusted intermediates may be accepted)** | 0.103.16 | Same |
| `webpki-roots` | 1.0.9 (and the 0.26.11 shim) | RUSTSEC-2026-0381 (distrusted emSign intermediates in the bundled roots) | 1.0.10 | Same |
| `zip` | 8.6.0 | RUSTSEC-2026-0367, -0368, -0369 (SUID bits, symbolic links and permissions through `extract()`) | 9.0.0 | `calamine` 0.36 reads workbooks by entry; `extract()` is never called. `calamine` 0.37 needs `zip` 9, `quick-xml` 0.42 and Rust 1.88 |
| `rsa` | 0.9.10 | RUSTSEC-2023-0071 | none | Ignored in `deny.toml` (HS256 only), revisit 2027-01-15. `jsonwebtoken` 11 still links it with `rust_crypto` |

`cargo update -p rustls -p rustls-webpki -p webpki-roots@1.0.9 --dry-run` moves the first three with nothing else: a lockfile-only fix.

### npm

`npm audit --omit=dev`: **0 vulnerabilities.** With development packages: 10 (2 critical, 5 high, 3 moderate), all in the build and test tools:

| Package | In use | What | Fix |
|---|---|---|---|
| `vitest` 3.2.7, `tinypool` 1.1.1 | dev | Prototype-pollution gadgets to code execution in worker options; a mock redirect reads any file | Vitest 4.1.11 or later (major) |
| `vite` 5.4.21, `esbuild` 0.21.5 | dev | `server.fs.deny` bypass with Windows alternate paths, `.map` path traversal, `launch-editor` command and NTLM issues on Windows, any site reading the dev server | Vite 6.4.4 or later (major); the 5 line gets no fix |
| `postcss`, `nanoid`, `browserslist`, `source-map-js`, `baseline-browser-mapping` | dev, transitive | Source-map file disclosure, loops, memory growth | Patch releases inside the current ranges (`npm update`) |

The development server listens on `127.0.0.1:3021` only, and the maintainer develops on Windows, where three of the Vite issues apply.

---

## 2. Findings carried over from 2026-10-14

The 2026-10-14 review proposed an order of work (its §11) that the 0.2.x tracks did not take up. Each finding was looked up again in the code.

- **E1 is open.** `Converter::detect` (`apps/server/src/conversion.rs:67–71`) falls back to `find_soffice()`, so conversion is on wherever LibreOffice is installed. No version is compared with the fixed releases (the version is only shown in System › Overview). The profile under `previews/profile` is LibreOffice's default: Java, macros and link updates as shipped. The sandbox wrapper is optional. The container builds LibreOffice into the **server** image (`Dockerfile`, `WITH_LIBREOFFICE`), next to the database and `secret.key`, with the network.
- **E2 is open.** `check_provider_url_with` (`crates/ai/src/lib.rs:85–131`) checks the host only; `DEFAULT_ALLOWED_HOSTS` names `127.0.0.1`, `localhost` and `::1` with no port, and the path is kept. A workspace administrator can aim the server at any loopback service (Ollama's `/api/pull`, the server's own port, an admin console), and the provider test answers differently for a refused connection and an HTTP error.
- **E3 is open.** `create_organization` (`apps/server/src/organizations.rs:3–15`) only asks for a verified address.
- **E4 is open.** Local repository links are workspace-administrator actions under the server-wide roots.
- **E5 is open.** `AuthenticatedSubject` (`apps/server/src/access_control.rs:166–178`) admits a read-and-write token for every handler that does not ask for `SessionSubject`. The doc comment says invitations need a session, but `create_workspace_invitation` (`access.rs:136`), `upsert_workspace_member` (`organizations.rs:166`), `delete_workspace` (`workspaces.rs:95`), `save_workspace_ai_provider` (`ai.rs:115`), `update_ai_policy` (`ai_access.rs:145`), `connect_repository` (`repositories.rs:421`), the webhook routes (`webhooks.rs:405`), `export_workspace` and `import_workspace` all take `AuthenticatedSubject`.
- **E6 is open.** `workspace_events` (`streams.rs:24–62`) forwards `WorkspaceEvent::Activity` to anyone who can read the workspace; `list_workspace_activity` (`workspaces.rs:311`) requires an administrator.
- **E7 is open and wider than reported.** `RepoMemoCore::get_artifact` (`crates/api/src/lib.rs:390–402`) calls `extract_document_text` directly in an `async fn`; since V0.2.44 that starts a reader process and waits for it synchronously (`crates/ingestion/src/reader.rs:124–170`), holding a runtime worker for up to the reading timeout. It runs on every view of a document, and also in ten handlers that only need the workspace for their rights check, which they make **after** the reading (`artifacts.rs:386`, `607–612`, `690`, `720`, `752`, `772`, `813`, `859`, `881`, and `index_artifact` at `874`). The version routes already do it right: `readable_artifact_workspace` (`artifacts.rs:1230`) reads the summary and checks first.
- **E8 is partly fixed.** A second whole-workspace re-index is a `409` (V0.2.6). `index_artifact` (`artifacts.rs:874–905`) still spawns a task per request.
- **E9 is open.** `upsert_organization_member` (`crates/storage/src/organizations.rs:280–288`) writes the organization role into every workspace of the organization with `ON CONFLICT … DO UPDATE`, so a person a workspace owner removed comes back, and a viewer becomes a member.
- **E10 is open in the web client.** `externalHost` (`components/SafeMarkdown.tsx:18–26`) parses with `new URL(address)` and no base, so `//evil.example/x` throws, is treated as "not a web address" and rendered same-tab without its host. (The desktop part was closed in V0.2.51.)
- **E11 is open.** Keys are the full address (`security.rs:319–343`); `hash_password` and `verify_password` (`auth.rs:540–563`) go to the blocking pool with no cap; `RateLimiter` (`security.rs:60–100`) is pruned by the hourly maintenance only.
- **E12 is open.** `auth.rs:271` and `:304` log `email = %email`.
- **E13 is partly fixed.** `normalize_email` (`crates/storage/src/lib.rs:1356`) and `require_name` (`:1364`) are unchanged; mail composition refuses control characters, so headers are safe.
- **E14 is open.** `COOKIE_NAME = "repomemo_refresh"` (`web_session.rs:31`).
- **E15 is partly fixed.** `watch_event_stream_access` (`streams.rs:167–195`) re-checks the session version, deactivation and membership, not whether a personal access token still exists.
- **E16 is partly fixed.** Links open in view mode (`ms-excel:ofv|u|`, `lib/documents.ts:46–47`); `.xlsm` is still offered (`:9`).
- **E17 is open,** and the container adds four more moving references: `node:22-bookworm-slim`, `rust:1-bookworm`, `debian:bookworm-slim`, `caddy:2-alpine`.
- **E18 is open for local repositories.** `git_command` (`crates/git/src/lib.rs:488–500`) sets `GIT_*` variables on top of the inherited environment; remote commands clear it (V0.2.20).
- **E19 is open.** `pdf_path` and the render status are keyed by content hash alone (`conversion.rs`).

---

## 3. New findings

### S5-1. TLS advisories, and CI is red (medium)

See §1. The fixes are patch releases inside the current requirements. Until they land, the blocking `cargo deny` job fails every push.

### S5-4. Two tabs refreshing at once sign the person out everywhere (medium)

**Where:** `crates/storage/src/users.rs:177–205` (`consume_refresh_token`), `apps/desktop/src/lib/sharedApi.ts:190–230` (`refreshInFlight`).

The refresh cookie is one per browser; the de-duplication of refreshes is one per tab. A new tab with no access token in its `sessionStorage` refreshes at once, and every tab refreshes when its 60-minute token runs out. When two requests carry the same cookie, the first rotates it; the second finds it revoked and calls `revoke_user_refresh_tokens`, which ends every session of the account, the first tab's new one included. Reopening a browser with several RepoMemo tabs, or two tabs crossing the hour together, is enough. Reuse detection then fires on ordinary use, which also hides a real theft among the noise.

**Fix:** remember why a token was revoked (`rotated`, `signed_out`, `reuse`) and when. A token rotated in the last 30 seconds that comes back is answered `401` without revoking the family; only an older rotated token, or one revoked for another reason, counts as reuse. On the client, one refresh at a time across tabs (`navigator.locks`), and a tab that loses the race reads the session again instead of refreshing.

### S5-5. JSON bodies as large as the upload limit (medium)

**Where:** `apps/server/src/lib.rs:871`, `DefaultBodyLimit::max(config.max_upload_bytes)`.

V0.2.52 (S4-2) gave the anonymous endpoints 64 KiB. Every other route still buffers up to `REPOMEMO_MAX_UPLOAD_MB` (default 10, up to 1,024) before its handler runs. A viewer can post 1 GiB to a comment or a search when the operator has raised the limit for large uploads.

**Fix:** 1 MiB by default; the upload, new-version and import routes name the upload limit themselves (the avatar route already has its own).

### S5-7. The container's git is refused for remote repositories (low, a logic bug)

**Where:** `Dockerfile` (`debian:bookworm-slim`, `apt-get install git`), `crates/git/src/remote.rs:313–338` (`is_patched_git`, whose test lists `2.39.5` as unpatched).

Debian 12 ships git 2.39.5 with the CVE-2025-48384 fix backported, but its version number is below every release the check accepts. `require_patched_git` therefore refuses every remote clone in the official image. **Fix:** a base whose git the check accepts (Debian 13 ships 2.47.3), checked by `npm run container:check`.

### S5-8. The container runs with more than it needs (low)

`docker-compose.yml` sets no `read_only`, `cap_drop`, `security_opt: no-new-privileges` or `tmpfs`. With E1's LibreOffice in the same image, a document that escapes LibreOffice reaches the database, the key file and the network. **Fix:** a read-only root with the data volume and a `tmpfs` for `/tmp`, `cap_drop: [ALL]`, `no-new-privileges`, and LibreOffice in its own container with no network that sees only its work folder.

### S5-9. Any writer can cancel any job (low)

`cancel_job` (`artifacts.rs:1005–1016`) asks for `require_workspace_write`. A member can stop an administrator's whole-workspace re-index or a repository sync. **Fix:** the person who started the job, or an administrator.

### S5-2, S5-3, S5-10, S5-11, S5-12

See §1 and the table. S5-10 is a smaller surface for free: SQLite needs no TLS, and the advisory of `webpki-roots` 0.26 came through it.

---

## 4. Backend performance

Read with the V0.1.74 targets in mind (search under 300 ms, the dashboard under 500 ms, map and health under 2 s, at 50,000 passages) and the 100,000-file repository of V0.2.54. None of these was measured here.

| ID | Finding | Where | Release |
|---|---|---|---|
| P12 | Rights checks read whole documents (E7): ten handlers extract a document's text in a reader process, on a runtime thread, to learn its workspace | `artifacts.rs` (see E7) | V0.2.149 |
| P13 | The artifact page extracts the text again on every view, although indexing stored it as passages | `crates/api/src/lib.rs:390–402` | V0.2.150 |
| P14 | Uploads are held whole in memory, then copied (`body.to_vec()`), then hashed | `artifacts.rs:545`, `:1201` | V0.2.153 |
| P15 | Every workspace page loads **every** file of the workspace: `GET …/artifacts` reads all rows plus two whole-workspace maps and pages in memory (`PageQuery::apply`), and the workspace page, evidence, memory, tasks, the artifact page and the command palette ask for the full list, again after live events. On a 100,000-file repository that is tens of megabytes per refresh | `artifacts.rs:17–48`, `requests.rs:433–458`; `web/workspace/data.ts:40–44`, `WorkspaceDetail.tsx:86`, `CommandPalette.tsx:60` | V0.2.154, V0.2.155 |
| P16 | A repository sync with nothing new lists the whole tree, reads up to 20,000 commits for last authors, reads `CODEOWNERS` and loads every stored file before finding nothing to do (the risk V0.2.54 named) | `crates/api/src/repo_sync.rs:434–500` | V0.2.156 |
| P17 | The last-author pass reads the history from the head every sync instead of from the previous commit | `repo_sync.rs:465–472` | V0.2.157 |
| P18 | Search across workspaces runs up to 30 workspaces one after another, embedding the query again in each | `apps/server/src/retrieval.rs:29–110`; `crates/api/src/lib.rs:668` | V0.2.158 |
| P19 | SQLite runs with its defaults: a 2 MiB page cache per connection, no memory-mapped reads, temporary tables on disk | `crates/storage/src/lib.rs:695–718` | V0.2.159 |
| P20 | No response compression outside the container (Caddy compresses there; the nginx recipe and direct use do not) | `lib.rs` (no `CompressionLayer`) | V0.2.160 |
| P21 | Nothing sheds load: past the read pool and the writer, requests queue until their timeout | `lib.rs:847–855` | V0.2.161 |
| E8 | One task per single-file re-index request | `artifacts.rs:874–905` | V0.2.151 |

---

## 5. Checked and found sound

- **SQL** binds every value; the only `format!` fragments are constant column lists and joins (`access.rs`, `collaboration.rs`, `mentions.rs:138`, `repo.rs`).
- **The 0.2.x routes** check rights the right way round: versions and comparison (`readable_artifact_workspace`), mentions, a file at a commit, the review queue, jobs, saved searches, mention suggestions (members only, writers only), cross-workspace search (role and AI policy per workspace).
- **Webhooks** verify the signature before remembering a delivery id, so an unsigned caller cannot fill or poison that memory.
- **Log files** for a day are parsed as a date before any path is built; backup names are a closed character set.
- **The JWT secret** must be 32 characters and not repetitive; access tokens live 5 to 1,440 minutes; personal access tokens record their last use at most once per interval, so a token does not write on every request.
- **Request telemetry** keeps at most 500 clients.
- **Indexes** cover the hot lookups (activity, tasks, comments, notifications, memberships by user, chunks and embeddings by workspace).

---

## 6. For the next pass

- Run `cargo deny check` and `npm audit` after V0.2.130 to V0.2.134; they should be clean apart from the documented ignore.
- Try S5-4 with two tabs and a 5-minute access token; try E2 against a local Ollama; try E7 by opening a large PDF's page as a non-member.
- Run the four performance commands of [technical/performance.md](../technical/performance.md) before track M and after it, and record both.
