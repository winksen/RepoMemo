# RepoMemo security review — 2026-10-14 (V0.2.6)

> **Scope:** the code at `V0.2.6` (commit `4b10be6`): the server (`apps/server`), the core and storage crates, ingestion, indexer, retrieval, AI and git crates, the web client and the desktop shell (`apps/desktop`), the terminal console (`apps/console`) and CI. Also the programs the server drives (LibreOffice, git, a local Ollama), and the surfaces planned for 0.3.
> **Method:** a full static review of the code, done afresh rather than as a diff. It included a mechanical pass over every handler's extractor (13 public handlers, all intended). Advisories published since the third pass were researched (RustSec, the GitHub Advisory Database, LibreOffice, Ollama, git) and checked against `Cargo.lock` and `package-lock.json`. Nothing was run, built or attacked. `cargo deny` and `npm audit` were not run locally because the repository's rules keep that to CI. Line numbers are as of this commit.
> **Status:** open. **2 high, 7 medium, 9 low, 4 info**, none fixed yet. Proposed targets are in [§11](#11-proposed-order-of-work).
> **Previous review:** [2026-10-10](2026-10-10-security-review.md), with its second pass (§11) and third pass (§12). All of its findings stay closed. Two return here in a new form: **E1** grows out of M10 (LibreOffice) and **E2** out of H2 (provider URLs).

| ID | Severity | Finding | Proposed target |
|---|---|---|---|
| E1 | **High** | LibreOffice converts every Office upload, unsandboxed by default, and has public load-time file-read, SSRF and code-execution flaws (October 2026). The sandbox recipe in the guide leaves the data folder readable | Next release |
| E2 | **High** | AI provider URLs reach any port and path on this machine, so any workspace admin can drive a local Ollama's admin API, including its model downloader, which has its own 2026 SSRF advisories | Next release |
| E3 | Medium | Every account, invited viewers included, can create an organization and become its owner, which unlocks every owner-level feature | Decision, then a release |
| E4 | Medium | Any workspace admin can link any repository under `REPOMEMO_REPO_ROOTS`, including other teams' repositories | With E3 |
| E5 | Medium | A read-and-write personal access token can invite people, change roles, delete workspaces and repoint providers. People it invited keep their access after the token is revoked | One release |
| E6 | Medium | The live event stream sends every reader the activity feed that the API keeps for owners and admins: emails, invitations, other people's AI questions | One release |
| E7 | Medium | Document text is extracted on the async runtime, again on every artifact request, so a few page views of a slow PDF stall the server | One release |
| E8 | Medium | V0.2.6's background re-index starts one unbounded task per request, with no deduplication or quota | Fix of V0.2.6 |
| E9 | Medium | An organization role change re-adds the person to every workspace of the organization, undoing a workspace owner's removals and viewer downgrades | One release |
| E10 | Low | Protocol-relative and backslash links in Markdown skip the host label and open in the same tab. The desktop app's Markdown has no link rules at all | Ride-along |
| E11 | Low | Per-address limits count each IPv6 address separately, and password hashing has no global cap | Ride-along |
| E12 | Low | Anyone can forge lines in the console's text logs through the sign-in email field | Ride-along |
| E13 | Low | Email addresses and display names accept control characters and line breaks. Mail delivery (V0.2.9) will put them into headers | Before V0.2.9 |
| E14 | Low | The refresh cookie has no prefix and is shared across the site, so a sibling subdomain can plant a session | Ride-along |
| E15 | Low | Revoking a personal access token does not end the event streams it opened | Ride-along |
| E16 | Low | *Open in Office* opens teammates' uploads, macro-enabled `.xlsm` included, as trusted intranet files | Ride-along |
| E17 | Low | GitHub Actions are pinned to moving tags | Ride-along |
| E18 | Low | git child processes inherit the server's secrets in their environment | Before V0.2.19 |
| E19 | Info | Rendered previews are cached server-wide by content, which tells a user whether someone else has the same Office file | Documented |
| E20 | Info | Readers see server folder paths in repository details, job errors and indexing failures | Documented |
| E21 | Info | Only `secret.key` is owner-only. The database, stored files and backups keep the folder's inherited permissions | Guide |
| E22 | Info | Dependencies: every library advisory published since the third pass is already fixed in the lock files. Vite 5 remains a dev-only exception | Tracking |

---

## 1. Summary

The protections that V0.2.0 promised still hold in the code. SQL is bound everywhere, every route has the right extractor, sessions are checked against their version on every request, the refresh cookie needs the web app's own header, file downloads cannot run as pages, and repository links re-check the real repository root. The setup and platform codes are long, compared in constant time and locked out per address.

The new risks come from three directions:

1. **Programs the server drives.** LibreOffice and a local Ollama are trusted with input that strangers can shape. LibreOffice published six load-time flaws on 2026-10-05, including reading local files, fetching URLs and running a remote Java class. RepoMemo hands every uploaded Office file to whatever LibreOffice it finds, and shows the result back to the uploader as the preview (E1). Provider URLs accept any path on loopback, and Ollama's model downloader is a known server-side weapon in 2026 (E2).
2. **Who is an owner.** Registration is closed, but any invited person can create an organization and own its workspaces (E3). Owner-level features were designed for trusted administrators: provider URLs (E2), repository links (E4), invitations. Write tokens also carry most of these powers (E5).
3. **Availability and data minimisation in newer code.** Document parsing blocks the async runtime (E7). The V0.2.6 background re-index has no bound (E8). The event stream sends readers what the API keeps from them (E6).

| Severity | Count | IDs |
|---|---:|---|
| Critical | 0 | — |
| High | 2 | E1, E2 |
| Medium | 7 | E3–E9 |
| Low | 9 | E10–E18 |
| Info | 4 | E19–E22 |

**Suggested first patch set:** E1 (a minimum LibreOffice version, and a sandbox that hides the data folder), E2 (provider URLs limited to a host, port and empty path, with private hosts set up by system administrators), E8 (bound the V0.2.6 tasks), then decide E3.

---

## 2. Threat model: what changed

The actors of the [first review](2026-10-10-security-review.md#2-threat-model) still apply. Since then:

| Actor | Before | Now |
|---|---|---|
| Anonymous network user | Could register (open by default) | Cannot register (closed by default). Reaches sign-in, link previews, file links, setup and platform codes, health |
| **Invited person, any role** | — | Can create an organization and becomes owner of its workspaces (E3). That makes them a workspace admin: provider URLs (E2), repository links (E4), invitations |
| Holder of a leaked write token | Reached what the owner could, except account security | The same, which includes granting access to new people who outlive the token (E5) |
| **Content from outside** | Indirect prompt injection; parser bugs | Also LibreOffice's load-time features (data links, media playlists, package URLs, Java drivers) reached by any `.docx`, `.xlsx` or `.pptx` an uploader controls (E1) |
| **Local services** | Not considered | A local Ollama (the documented setup) listens on loopback with no authentication. Its admin API (`/api/pull`, `/api/create`) is reachable through provider URLs (E2) |

---

## 3. High

### E1. LibreOffice converts every Office upload, unsandboxed by default, and has public load-time file-read, SSRF and code-execution flaws

**Where:**
- `apps/server/src/lib.rs:315–318`: the converter is used whenever LibreOffice is found (`Converter::detect`, `conversion.rs:68`).
- `apps/server/src/artifacts.rs:537`: every upload and new version starts a conversion.
- `apps/server/src/conversion.rs:134–161`: no wrapper unless the operator sets `REPOMEMO_SOFFICE_WRAPPER`.
- `conversion.rs:310`: `{work}` sits inside the data folder.
- `conversion.rs:111`: the version is only displayed, never checked.
- [operations.md §8](../technical/operations.md#8-office-previews-and-the-sandbox): the recommended `bwrap` line.

**What happens:** LibreOffice's [advisories](https://www.libreoffice.org/about-us/security/advisories/) of 2026-09-21 and 2026-10-05 list flaws triggered **when a document is loaded**, which is exactly what a headless conversion does. All are fixed in 26.2.5 and 26.8.0:

| CVE | What a crafted document does on load |
|---|---|
| CVE-2026-63277 | Loads a Java driver from a remote location through a data link and runs it (when Java is available) |
| CVE-2026-63267 | Reads a local file into the document and makes GET requests (CSV data provider) |
| CVE-2026-63268 | Reads local files through an SQL data link |
| CVE-2026-63269 | Reads local files and makes GET requests through media playlists |
| CVE-2026-63266 | Writes a file through an embedded Firebird database |
| CVE-2026-63270, -63278 | Expands environment variables and INI values into URLs it then fetches |
| CVE-2026-63272–63276, -63279 | Memory-safety bugs in WMF, PDF, CFF and PICT import |

The scrubbed environment from M10 blunts the environment leaks, and `--unshare-net` in the guide blunts the fetches. Neither stops the main path, though. **A file read lands in the converted PDF, and the uploader reads that PDF as the preview.** No network is needed. The guide's recipe starts with `--ro-bind / /`, so inside the sandbox the whole data folder is still readable: `secret.key`, the database, every workspace's stored files and every other preview. LibreOffice also picks the import filter from the content, not the name, so an ODF spreadsheet renamed `.xlsx` reaches the ODF features. On Windows there is no sandbox at all.

**Attack:** any member with write access uploads `budget.xlsx`, a spreadsheet whose data link reads `workspace-data/<env>/secret.key` (or the database). The preview of the file, which they are entitled to see, shows the key. With it, every stored provider API key decrypts. If the installed LibreOffice has Java, the same upload runs code as the server's account. A distribution's LibreOffice (for example Ubuntu 24.04's 24.2 line) is affected until the distribution ships the fixes.

**Fix:**
- **Refuse to convert with an affected version.** Parse the probed version, and turn previews off below 26.2.5 (or the distribution's patched build, which the operator can confirm with a setting). Report that in the server checks as an error, not information.
- **Make conversion an explicit choice.** Convert only when `REPOMEMO_SOFFICE` is set, or when a sandbox is configured, instead of whenever LibreOffice happens to be installed.
- **Hide the data folder from the sandbox.** Move `{work}` and `{profile}` to a dedicated folder outside the environment. Correct the recipe so it binds only what LibreOffice needs (`/usr`, `/etc/fonts`, `/tmp`, `{work}`, `{profile}`), or adds `--tmpfs` over `workspace-data`. Add a `{data}` placeholder so the wrapper can mask it.
- **Preconfigure the dedicated profile** so loading cannot reach out: Java off, external links blocked (`BlockUntrustedRefererLinks`), links never updated, no external data refresh.
- **Check the container before converting:** an `.xlsx`, `.docx` or `.pptx` must be OOXML, not ODF.
- **On Windows,** run conversions as a separate low-privilege account that cannot read the data folder, or leave previews off.

### E2. AI provider URLs reach any port and path on this machine, including a local Ollama's admin API

**Where:**
- `crates/ai/src/lib.rs:38`: the built-in host list allows `127.0.0.1`, `localhost` and `::1` on any port.
- `lib.rs:85–128`: the URL check looks at the scheme, user info and host only. Path, query and port are not checked.
- `lib.rs:322–324` and `463`: endpoints are built as `base_url + "/api/generate"` and the like.
- `lib.rs:356–370`: the embedding call, made in the background by the embedding queue.
- `apps/server/src/ai.rs:115`: workspace admins save providers.

**What happens:** a provider's base URL may carry a path and a query. With `http://127.0.0.1:11434/api/pull?` as the base URL, the "generate" request becomes a POST to `/api/pull` whose JSON body carries `"model": <whatever the admin typed>`. The suffix is swallowed as the query. Ollama ignores the extra fields and **downloads that model**. The embedding queue makes the same call on its own after the next indexing, with no click needed. Any other loopback HTTP service can be posted to the same way. Connection errors, HTTP status codes and timings from the provider test also map which loopback ports are open.

Ollama ships with no authentication on its port. Its model downloader has 2026 SSRF advisories ([CVE-2026-5530](https://vuln.today/cve/CVE-2026-5530), and [CVE-2026-85180](https://app.opencve.io/cve/CVE-2026-85180), which redirects blob downloads to arbitrary hosts in 0.30.0–0.33.2). Attackers were [seen using it as a server-side weapon](https://labs.zenity.io/p/threat-actors-are-using-ollama-s-model-downloader-as-a-server-side-weapon) in spring 2026.

**Attack:** an invited viewer creates their own organization and workspace (E3) and adds an embedding provider of type Ollama at `http://127.0.0.1:11434/api/pull?`, with the model `registry.attacker.example/x/huge:latest`. The server's Ollama pulls tens of gigabytes until the disk floor stops uploads for everyone. On an affected Ollama version, the pull's redirects also reach internal hosts. A pulled model then serves every workspace that names it.

**Fix:**
- **Validate the URL's shape.** Refuse a query, a fragment or user info. For Ollama, refuse any path. For OpenRouter-type providers, allow only the documented `/api/v1` path.
- **Name ports in the allowlist.** Make the built-in loopback entry `127.0.0.1:11434` (Ollama's port), and let `REPOMEMO_AI_ALLOWED_HOSTS` entries carry ports.
- **Keep private providers with system administrators.** Only they should save a provider on a loopback or private address, as a server-wide provider that workspaces pick from. Workspace admins keep public providers (OpenRouter) with their own key.
- **Report less from the provider test:** "reachable" or "not reachable", without status codes.
- **Update the guide:** keep Ollama on loopback at 0.34.2 or later.

---

## 4. Medium

### E3. Every account can create an organization and become its owner

**Where:** `apps/server/src/organizations.rs:3` (`create_organization`, any authenticated caller), `crates/storage/src/organizations.rs:134`. Only `REPOMEMO_WORKSPACES_PER_PERSON` (25) limits what follows.

**What happens:** closing registration (M1) assumed that accounts belong to trusted people. Every invitation still creates an account that can own organizations: a viewer invited to one workspace creates "Mine", becomes its owner, and is now a workspace admin. Owner and admin features were built for trusted administrators, and several reach beyond the workspace: provider URLs (E2), repository links (E4), storage allowances and background AI budget (each workspace gets its own).

**Fix:** decide who may create organizations. A run-time setting is proposed: system administrators only by default, or also app administrators, or everyone (today's behaviour). The setup wizard should ask. Whatever is decided, E2 and E4 should not depend on it.

### E4. Any workspace admin can link any repository under the configured roots

**Where:** `apps/server/src/repositories.rs:362–368` (workspace admin), `crates/api/src/repo_policy.rs:76–108` (one list of roots for the whole server).

**What happens:** `REPOMEMO_REPO_ROOTS` says where repositories may be, not who may read which. When one folder holds several teams' checkouts, any workspace admin can link any of them, and with E3 that means any account. The repository root is re-checked after `git rev-parse` (`repo_sync.rs:328`), so a parent repository cannot be reached. That part is sound.

**Fix:** tie roots to organizations (for example `REPOMEMO_REPO_ROOTS` entries scoped to an organization, set by system administrators). Alternatively, make linking a request that a system administrator approves. Decision 11 for remote repositories should settle local and remote links the same way.

### E5. A read-and-write personal access token can grant access, and what it grants outlives it

**Where:** these handlers take `AuthenticatedSubject` (a write token is accepted):
- `access.rs:132` and `:199` (create invitations)
- `organizations.rs:61` and `:165` (member roles)
- `workspaces.rs:94` (delete a workspace)
- `ai.rs:115` and `ai_access.rs:145` (providers, AI policy)
- `repositories.rs:362` (link a repository)

Only tokens, passwords, system roles, settings, ownership transfer and the environment need a session.

**What happens:** write tokens are made for scripts and CI, where they leak. With the owner's write token, an attacker creates an invitation for an address they control. Invitations allow registration even when it is closed, so the attacker gets an account in the workspace. When the token is found and revoked, or the owner resets their password (which revokes tokens, N5), the attacker's account stays. A system administrator's write token is an administrator of every workspace (`access_control.rs:30–38`).

**Fix:** require a signed-in session, or a new `admin` token scope, for creating invitations, changing members and roles, deleting a workspace, saving providers and the AI policy, and linking repositories. When a token is revoked, list the invitations it created so they can be withdrawn.

### E6. The live event stream sends every reader the activity feed that is kept for owners and admins

**Where:** `apps/server/src/streams.rs:24–29` (any reader may subscribe), `events.rs:193–210` (every activity row is published whole, with actor and summary), against `workspaces.rs:305–317` (`GET …/activity` and the calendar need an administrator). The [functional mindmap](../FUNCTIONAL_MINDMAP.md) lists the activity feed as owner and admin only.

**What happens:** a viewer who opens the workspace receives every activity event as it happens, readable in the browser's network panel. That includes:
- the actor's email;
- "Invited bob@partner.com as admin.";
- "Set Alice to viewer.";
- "Asked: \<a colleague's question to the AI\>";
- provider and repository changes.

Viewers cannot list members' emails through the API (`organizations.rs:156` needs an administrator), but the stream gives them out.

**Fix:** send non-administrators only what the client needs to refresh (`action`, `subject_type`, `subject_id`), without summary or actor, or open a separate stream for administrators. The stream watcher already knows the subscriber's role.

### E7. Document text is extracted on the async runtime, on every artifact request

**Where:**
- `crates/api/src/lib.rs:335–348`: `get_artifact` extracts the whole text of a Word, Excel, PowerPoint, PDF, OneNote or Outlook file each time it is called.
- `crates/ingestion/src/budget.rs:45–98`: the reading runs on its own thread, but the caller blocks on `recv_timeout` for up to 60 seconds.
- `crates/api/src/lib.rs:1124` and `crates/indexer/src/lib.rs:53`: indexing parses on the runtime as well.
- `apps/server/src/artifacts.rs`: `core.get_artifact` is called just to read the workspace id in nine handlers (`get_artifact`, lifecycle and its history, title, delete, move, re-index, new version, supersede target).

**What happens:** the server runs one runtime worker per CPU core (`#[tokio::main]`). Each page view of a document makes several of these calls, and each blocks a worker until the reading ends. A writer uploads a PDF crafted to parse slowly, and a handful of readers open it. Every worker is then busy, so all requests, health checks included, wait up to a minute. After four readings overrun, every document reading on the server is refused until those threads end (R2-4), which may be never. Extraction also runs **before** the authorization check in these handlers.

**Fix:**
- In handlers that only need the workspace, use `storage.get_artifact_summary`, and check the role first.
- Store the extracted text at indexing time and serve the preview from it.
- Run `extract_document_text` and `index_artifact` inside `spawn_blocking`.
- The subprocess readings planned for V0.2.44 (P10) remain the real bound.

### E8. V0.2.6's background re-index starts one unbounded task per request

**Where:** `apps/server/src/artifacts.rs:821–861` (`tokio::spawn` per `POST /v1/artifacts/{a}/index`, any writer). The workspace-wide re-index refuses a second run with `409`, but the per-file one has no such check.

**What happens:** the request now answers `202` at once, so the request timeout no longer paces it. A script with write access can queue thousands of re-indexes of the same large file in seconds. Each re-extracts and re-chunks the file (E7) and asks for embeddings again. The automatic queue's concurrency limit (`REPOMEMO_INDEX_CONCURRENCY`) is bypassed.

**Fix:** send manual re-indexes through the existing `IndexQueue`, with one pending job per file (answer `409` with the running job), and count them against the per-person heavy-request allowance.

### E9. An organization role change undoes a workspace owner's removals and downgrades

**Where:** `crates/storage/src/organizations.rs:279–285`: `upsert_organization_member` inserts the person into **every** workspace of the organization, overwriting their role (`member`, or `admin` for an organization admin). `remove_workspace_member` (`:591`) deletes only that workspace's row.

**What happens:** the owner of "HR — confidential" removes Bob, who is in the organization, from that workspace, or makes him a viewer. Later an organization administrator changes Bob's organization role, re-saves it, or Bob accepts an organization invitation for a higher role. Bob is back in "HR — confidential" as a member, and nobody is told. The administrator who does this does not even need access to that workspace.

**Fix:** propagate an organization role only into workspaces where the person already has a row, and only upward. Or record a workspace-level exclusion that propagation respects. Add a test: a person removed from one workspace stays removed after their organization role changes.

---

## 5. Low

| ID | Finding | Where | Fix |
|---|---|---|---|
| E10 | **Markdown links that look relative leave the site unlabelled.** `[design doc](//evil.example/login)` and `\\evil.example\…` pass react-markdown's protocol check as relative links, so `SafeMarkdown` shows no host and opens them in the same tab, a phishing path for prompt-injected AI answers and READMEs (L9). The desktop app renders Ask answers, summaries and cards with plain `ReactMarkdown` and none of these rules. Its policy blocks remote images, but links can still navigate the window | `components/SafeMarkdown.tsx:44–56`; `App.tsx:2305, 2439, 2523` | Resolve `href` against the page (`new URL(href, location.href)`) and treat any other origin as external. Use `SafeMarkdown` in the desktop app |
| E11 | **IPv6 rotation defeats per-address limits, and hashing has no global cap.** Rate limits and lockouts key on the full address. One host owns a /64 of IPv6 addresses, so sign-in, registration, preview and code limits stop applying. Each sign-in attempt hashes with Argon2 (about 19 MiB) on the blocking pool (up to 512 threads), so a flood can exhaust memory before the account-wide bar applies | `security.rs:315–341`; `auth.rs:417–441` | Key IPv6 clients by /64. Run password hashing behind a semaphore (for example twice the core count) and answer `429` when it is full |
| E12 | **Log forging in text logs.** The sign-in email is logged with `%email` before any validation (`auth.rs:161, 194`), so a line break in it writes a fake line into the console output, which journald or a log collector stores. The daily files are JSON lines and are safe | `auth.rs`; `logs.rs:298` (text is the default console format) | Log user input with `?` (escaped), or strip control characters in one helper. Recommend JSON console output for services |
| E13 | **Addresses and names are barely validated.** `normalize_email` checks only for an `@` and the length, so spaces, CR/LF, quotes and look-alike letters pass. Display names are only length-checked. This is harmless now, but V0.2.9–V0.2.15 will place both in mail headers (header injection) and V0.2.10 in invitation mails | `crates/storage/src/lib.rs:1337–1351` | Before V0.2.9: a strict address syntax (no whitespace or control characters, a bounded local part, IDNA for the domain) and display names without control or bidirectional-override characters. Re-check stored values when sending |
| E14 | **Session planting through the cookie.** `repomemo_refresh` has no `__Secure-` prefix, and `SameSite=Strict` treats every origin of the site as one. Whoever controls a sibling subdomain can set the cookie with their own refresh token, and the victim's web app then works in the attacker's account, uploading there | `web_session.rs:70–76` | Use `__Secure-repomemo_refresh` when the public address is HTTPS. Have the web app check, after a refresh, that the account is the one it signed in as |
| E15 | **A revoked token's streams stay open.** The stream watcher checks the session version and membership, not whether the token still exists. A personal access token's stream lives until the token would have expired (up to 365 days) | `streams.rs:167–195` | Re-check the token in the watcher (store its id on the subject), as is done for sessions |
| E16 | ***Open in Office* trusts uploads.** The link opens another person's upload in desktop Word or Excel. Served from an intranet name, it is not opened in Protected View, and `.xlsm` (macro-enabled) is offered like any workbook | `DocumentViewer.tsx:137–141`; `lib/documents.ts` | Offer *Open in* only for formats without macros, and say who uploaded the file. Download the others |
| E17 | **CI actions are pinned to tags** (`actions/checkout@v4`, `Swatinem/rust-cache@v2`, `EmbarkStudios/cargo-deny-action@v2`, `dtolnay/rust-toolchain@stable`). A moved tag runs new code in CI (the 2025 tj-actions compromise). Permissions are read-only, which limits the harm | `.github/workflows/ci.yml` | Pin to commit SHAs and let Dependabot update them |
| E18 | **git inherits the server's environment.** LibreOffice gets a scrubbed environment, but git children get `REPOMEMO_JWT_SECRET` and `REPOMEMO_SECRET_KEY`. No read path today runs repository-controlled programs (`core.fsmonitor` is overridden, no transport is allowed), but V0.2.20 adds fetches, credential helpers and SSH | `crates/git/src/lib.rs:311–345` | Reuse `is_secret_variable` for git (`env_remove`), or start git with a cleared environment plus `PATH`, `HOME` and the needed `GIT_*` variables |

---

## 6. Informational

- **E19. One server-wide preview cache.** Rendered PDFs are cached by content hash (`conversion.rs:179, 237–252`). Someone who uploads a document already uploaded elsewhere on the server sees `rendered-preview/status` answer `ready` at once, which confirms that the exact file exists somewhere. Key the cache by workspace, or always report a first conversion as in progress.
- **E20. Server paths reach readers.** A repository's `root_path`, job errors (`format!("{error:#}")`, `crates/api/src/lib.rs:417`, `repo_sync.rs:377`) and indexing failures can name folders on the server. Fine for administrators; for viewers, a short reason is enough.
- **E21. Data folder permissions.** L5 made `secret.key` owner-only (`secrets.rs:172`). The database (password hashes, encrypted keys, every workspace's text), stored files and backups follow the folder's inherited permissions. On a shared host, restrict the whole environment folder. The guide says so for Windows only.
- **E22. Dependencies.** See §7: every advisory found for the libraries is fixed in the lock files. The open items are the dev-only Vite 5 and the system programs (E1, E2, git for V0.2.20).

---

## 7. Research: dependencies and system programs

Advisories published since the third pass (2026-10-14 back to June 2026), checked against the lock files:

| Component | In use | Advisory | Fixed in | Status |
|---|---|---|---|---|
| `h2` | 0.4.20 | [RUSTSEC-2026-0258](https://rustsec.org/advisories/) unbounded empty DATA frames (2026-08-18) | 0.4.16 | ✔ fixed |
| `rustls` | 0.23.45 | [RUSTSEC-2026-0285](https://rustsec.org/advisories/) TLS 1.3 handshake messages across encryption levels (2026-09-14) | 0.23.45 | ✔ fixed (exactly at the fix; keep `cargo deny` blocking) |
| `lopdf` (via `pdf-extract`) | 0.42.0 | [RUSTSEC-2026-0187](https://rustsec.org/advisories/RUSTSEC-2026-0187) unbounded recursion, a **stack overflow that aborts the process** and cannot be caught (2026-06-21) | 0.42.0 | ✔ fixed |
| `quick-xml` | 0.41.0, 0.42.0 | RUSTSEC-2026-0194, -0195 (quadratic attributes, namespace memory) | 0.41.0 | ✔ fixed in V0.1.71 |
| `@tiptap/core` | 3.31.4 | [GHSA-j95f-988m-3j2f](https://github.com/advisories/GHSA-j95f-988m-3j2f) Markdown parser ReDoS; [GHSA-cp6q-959q-f8rh](https://github.com/advisories/GHSA-cp6q-959q-f8rh) prototype pollution to XSS (2026-08-26) | 3.30.5 / 3.30.4 | ✔ fixed. The ReDoS mattered: notes are loaded with `contentType: "markdown"` |
| `vite` / `esbuild` (dev) | 5.4.21 / 0.21.5 | [GHSA-67mh-4wv8-2f99](https://advisories.gitlab.com/npm/esbuild/GHSA-67mh-4wv8-2f99/) and the 0.17–0.28 pair; esbuild dev-server path escape on Windows | esbuild 0.28.1; the Vite 5 line gets no fix | Dev server only (bound to 127.0.0.1:3021). Plan the move to a current Vite |
| `rsa` (via `jsonwebtoken`) | 0.9.10 | RUSTSEC-2023-0071 | none | Ignored in `deny.toml` with reason (HS256 only); revisit 2027-01-15 |
| **LibreOffice** | operator's | [2026-09-21 and 2026-10-05 advisories](https://www.libreoffice.org/about-us/security/advisories/) (load-time file read, SSRF, file write, Java RCE, memory bugs) | 26.2.5 / 26.8.0 | ✖ **E1**: no minimum version is enforced |
| **Ollama** | operator's | [CVE-2026-5530](https://vuln.today/cve/CVE-2026-5530) pull SSRF; [CVE-2026-85180](https://app.opencve.io/cve/CVE-2026-85180) redirect SSRF (0.30.0–0.33.2) | 0.34.2 | ✖ **E2**: reachable through provider URLs |
| **git** | operator's | [CVE-2025-48384](https://dgl.cx/2025/07/git-clone-submodule-cve-2025-48384) (in CISA KEV): CR in submodule paths, RCE on recursive clone | 2.43.7 … 2.50.1 | Not reachable today (no clones, no submodules). Required before V0.2.20 |

No 2026 advisory turned up for `hyper`, `tokio`, `tower-http`, `axum`, `sqlx`, `zip`, `calamine`, `mail-parser`, `react-markdown` or `@tanstack/react-query`. That is not proof: CI's `cargo deny` and `npm audit` remain the check of record.

**Lesson from `lopdf`:** a stack overflow or an out-of-memory abort in **any** thread ends the whole server. The document readers' time budget does not help there. Subprocess readings (P10, V0.2.44) are the only real containment, and they would also cover the next parser bug of this kind.

---

## 8. Security requirements for the 0.3 surfaces

The roadmap's rule is that the security of a new outside connection ships with it. These are the requirements, from the code as it is and from the known attack classes, so each release can be checked against them.

| Track | Must have |
|---|---|
| **B. Mail** (V0.2.9–V0.2.15) | **Headers:** E13 first; reject CR/LF in every address, name and subject, and build messages with a library that encodes headers. **SMTP host:** checked through the same resolver rules as providers. **TLS:** required (implicit or STARTTLS) with certificate checks and no plaintext fallback. **Links:** built from `REPOMEMO_PUBLIC_URL`, never from the request's `Host` (reset-link poisoning). **Abuse:** per-sender and per-recipient rate limits, so invitations and mentions cannot be used to flood a mailbox. **Forgot password:** the same answer and the same time for every address (hash a dummy when there is no account); a one-time link; a newer link replaces older ones; an email change ends pending links |
| **C. Accounts** (V0.2.16–V0.2.18) | **Deactivation** ends sessions, tokens, open streams (E15), pending file links and the invitations the account sent. **Anonymization** also covers names in activity summaries and in log files, or documents what remains. **Registration:** the same answer for known addresses (M4 residual); its response time must not differ either |
| **D. Remote repositories** (V0.2.19–V0.2.26) | **git version:** refuse to clone below a patched git (2.50.1 or later). **Clone safety:** bare partial clones, never a checkout, `--no-recurse-submodules`; `protocol.allow=never` plus only `https` and `ssh` allowed; `transfer.fsckObjects=true`. **URLs:** refuse hosts starting with `-`, `ext::`, `file://` and local paths. **Credentials:** through `GIT_ASKPASS` or a helper on stdin, never in the URL or arguments; environment scrubbed (E18). **SSH:** `-o BatchMode=yes -o StrictHostKeyChecking=yes -o UserKnownHostsFile=<pinned> -o ProxyCommand=none`. **Network:** git resolves names itself, so the allowed-host check is advisory; enforce it with an egress rule or a proxy. **Webhooks:** HMAC-SHA256 in constant time, a delivery id and time window against replay, a body size limit and a rate limit, and the payload only *triggers* a sync. **Who links:** settle E4 together with decision 11 |
| **E. Issues and pull requests** (V0.2.27–V0.2.31) | **Tokens:** encrypted like provider keys, the narrowest read-only scopes. **Self-hosted servers:** GitHub Enterprise and GitLab URLs go through the outbound rules (SSRF). **Content:** issue and review bodies are untrusted, so render them with `SafeMarkdown` (E10 fixed first) and expect prompt injection in Ask and the assistant. **Webhooks:** verify `X-Hub-Signature-256` and `X-Gitlab-Token` in constant time |
| **F. Search and assistant** (V0.2.32–V0.2.39) | **Cross-workspace search:** each workspace's role **and** AI policy checked per result, and never a cache shared across workspaces. **Assistant memory:** earlier turns can carry injected instructions; keep the closed capability router, and never add a capability that writes without the person confirming. **Mention suggestions:** members only, rate-limited, never other organizations' people |
| **G. Portability** (V0.2.40–V0.2.46) | **Import** is fully untrusted input: no path traversal (zip-slip), no symlinks, size and ratio limits against zip bombs, fresh ids for everything, every reference re-validated inside the new workspace, quotas applied. **Export:** owners or admins only, audited, and nothing outside the workspace. **Encrypted backups:** AES-256-GCM per chunk with unique nonces, and the chunk index and a final-chunk flag in the associated data, so chunks cannot be truncated or reordered. **Readings in a subprocess:** memory and time limits, also covering aborts (§7). **Container:** non-root, read-only root filesystem, LibreOffice in its own container without network and without Java (E1) |
| **I. One client** (V0.2.49–V0.2.51) | **Credentials:** the session in the operating system's credential store. **Navigation:** the window stays on the server's origin; other origins get no IPC (Tauri capabilities with no `remote` URLs). **Updates:** the signing key is never in the bundle (M7's lesson). **Retiring the local edition** removes its unsafe Markdown (E10) |

---

## 9. Checked and found sound

- **Public surface:** a mechanical pass over every handler found 13 without an authenticated caller, all intended: sign-in, registration, refresh, sign-out, the invitation and reset previews and completion, file links, the setup code and administrator, the platform menu, and health.
- **Sessions:**
  - Access tokens are HS256 with a checked issuer, and the session version is compared on every request.
  - File-link tokens have their own issuer and cannot be used as sessions.
  - Refresh tokens rotate, and reuse revokes them all.
  - The refresh cookie is only read for requests carrying `X-RepoMemo-Client: web`, which a cross-site form cannot send.
- **System area:** reading needs a system or app administrator; changes need a system administrator plus a session or `system` token; the console goes through the same handlers.
- **Setup and platform codes:** 60 bits from `OsRng` with no modulo bias, constant-time comparison, a lockout per address, and printed outside the log files.
- **Environments:** names validated, symbolic links refused, every folder directly inside `workspace-data/`.
- **Invitations and resets:** acceptance re-checks the inviter's rights and never lowers a role. Reset links are single-use, end every session and revoke tokens. Previews are rate-limited.
- **Storage:** every query binds its values; `IN` lists bind each value; every listing limit is clamped. Memory-card citations are checked against the card's workspace; folders, bulk actions and supersede targets against the request's workspace.
- **Files:**
  - Downloads are attachments with `nosniff` and `CSP: sandbox`, and the content type comes from the name.
  - Upload names lose their folders and control characters.
  - ZIP and compound-file reading is capped at 16 MiB per member, and every reading is time-boxed.
  - Avatars are checked byte by byte and served sandboxed.
- **Repositories:** the policy is checked before the disk is touched and again on git's real root, so a parent repository is refused. Branch names cannot be options. `core.fsmonitor` is forced off. The same answer is given for every refusal outside the roots.
- **AI:** redirects are not followed; response bodies are capped; a stored key is only sent to the address it was saved for; provider bodies stay in the log; the assistant cannot write.
- **Web client:**
  - The content security policy has no inline script, `object-src 'none'` and `base-uri 'none'`.
  - `SafeMarkdown` blocks images, and react-markdown neutralises `javascript:` links.
  - The query cache is cleared when a session ends.
  - Live events send the token in a header, never in the URL.
  - Image previews open only raster formats.
- **Operations:** metric labels are escaped; log files are JSON lines; mentions notify only members; health actions act only on the finding's own files.

---

## 10. What is done well (additions since the first review)

The first review's list ([§8](2026-10-10-security-review.md#8-what-is-already-done-well)) still holds. Added since:
- System changes are split from reading, and System changes need a dedicated token scope.
- Refresh tokens are out of script reach, behind a strict content security policy.
- Outbound AI calls go through a resolver that checks addresses at connect time.
- Git runs isolated from the account's configuration.
- Request, header and event-stream limits.
- Rate limits on heavy reads, and a daily allowance for background AI.
- Quotas and a disk floor.
- Backups that restore only into a new folder.
- An upgrade test, and blocking dependency audits in CI.

The habit of re-checking rights at the moment of use is consistent throughout: invitations re-check the inviter, streams re-check membership, and repository syncs re-check the policy.

---

## 11. Proposed order of work

The 0.2.x rule is one change per release, with small fixes riding along. A proposal:

1. **Next release, E1:** a LibreOffice minimum version, opt-in conversion, the data folder hidden, Java and external links off in the profile, and the corrected recipe in the guide.
2. **Next release, E2:** the provider URL shape, ports in the allowlist, private providers set by system administrators.
3. **Ride along with the next release, E8:** manual re-indexes through the queue, one per file.
4. **Decision 15** (who creates organizations), then **E3**. **E4** follows from the same decision and decision 11.
5. **E5:** an `admin` scope, or a session, for access-granting routes.
6. **E6:** role-filtered events.
7. **E7:** document work off the runtime, with stored text.
8. **E9:** organization propagation that respects workspace decisions.
9. **Ride-alongs:** E10, E11, E12, E14, E15, E16, E17. **E13 before V0.2.9**, **E18 before V0.2.19**.
10. **Documentation:** E19–E21 in the guide and the auth page. Track Vite in the dependency policy.

The fourth security pass planned for V0.2.52 (R8) should take §8 as its checklist.
