# RepoMemo Roadmap

> **Current version:** `V0.2.21` (2026-10-15), after `V0.2.0` *Team-ready*, which waits for the clean-machine install (R6) before its tag · **Next minor after it:** `V0.3.0` *Connected* (proposed 2026-10-14)
> **Feature freeze (V0.1.76 to the `v0.2.0` tag):** only fixes from the clean install go into 0.2.0. The 0.2.x releases of [Road to V0.3.0](#-road-to-v030) follow the tag.
> **Related:** [Functional mindmap](FUNCTIONAL_MINDMAP.md) · [Technical mindmap](TECHNICAL_MINDMAP.md) · Release index [0.1 → 0.2](RELEASES_0.1.md) and [0.2 → 0.3](RELEASES_0.2.md) · [Security review 2026-10-10](security/2026-10-10-security-review.md) · [Security review 2026-10-14 (V0.2.6)](security/2026-10-14-security-review.md) · [Enhancement analysis 2026-10-13](analysis/2026-10-13-enhancement-analysis.md)

RepoMemo started as a local-first desktop workbench. It is now a **self-hosted team product**: a shared API server with a web client, organizations and roles, collaboration, git repositories as living sources, business documents, and optional AI grounded in cited evidence. A server can be installed and administered entirely from the browser (environment menu, first-run setup, System area, admin console).

Between V0.1.32 and V0.1.62 the work went into **depth and operability**. V0.1.63 added **onboarding without an account** (invitation links, reset links, personal access tokens). V0.1.64 and V0.1.65 fixed the most urgent security findings: bundle S1, and a second pass that included a stored XSS.

**V0.2.0 is proposed as the first *team-ready* release**: a server a team can install, expose and trust with its knowledge. That means secure by default, deployable and recoverable, checked on every commit, and keeping its core promise that memory stays true. The plan to get there is [Road to V0.2.0](#-road-to-v020); every release of it is built and only the clean-machine install (R6) remains before the tag.

**V0.3.0, *Connected*,** is proposed next: remote repositories, mail, issues and pull requests, an assistant that remembers the conversation, portable workspaces and one client, reached through 56 single-purpose releases (V0.2.1–V0.2.56). The plan is [Road to V0.3.0](#-road-to-v030).

> **Versions** are set in one place since V0.1.66 (`npm run version:set -- <version>`), checked in CI, and recorded in [CHANGELOG.md](../CHANGELOG.md).

---

## At a glance

```mermaid
timeline
  title RepoMemo delivery
  section Local memory loop
    May 2026 : V0.1.0 Desktop skeleton
    Jun 2026 : V0.1.2 Import and store : V0.1.3 Chunking and symbols : V0.1.4 Full-text search
    Jul 2026 : V0.1.6 AI providers : V0.1.8 Embeddings and Ask : V0.1.9 Image understanding
    Aug 2026 : V0.1.10 Memory cards
  section Shared team mode
    Aug 2026 : V0.1.12 Server foundation : V0.1.13 Accounts and shared evidence : V0.1.15 to V0.1.23 Admin, AI, collaboration, review
    Sep 2026 : V0.1.27 Organization admin : V0.1.30 Word documents : V0.1.32 Jobs API and live events
  section Depth and operability
    Oct 2026 : V0.1.40 Auto-indexing, notes, folders : V0.1.42 Business documents : V0.1.45 Assistant : V0.1.46 Semantic search
    Oct 2026 : V0.1.48 Knowledge map : V0.1.50 Health checks : V0.1.52 Repository sync : V0.1.51 to V0.1.57 Security hardening
    Oct 2026 : V0.1.58 System area : V0.1.59 Setup wizard : V0.1.60 Environments : V0.1.61 Admin console : V0.1.62 Live progress
    Oct 2026 : V0.1.63 Invitations and tokens : V0.1.64 Security S1 : V0.1.65 Security second pass
  section Road to V0.2.0
    Hardening : V0.1.66 Checkable releases : V0.1.67 Safe defaults : V0.1.68 Browser hardening : V0.1.69 Host exposure
    Foundations : V0.1.70 Web client foundation : V0.1.71 Server foundation
    Product : V0.1.72 Memory and search you can trust : V0.1.73 Evidence, ownership, repositories 1b
    Run it for real : V0.1.74 Performance and robustness : V0.1.75 Operations : V0.1.76 Release candidate : V0.2.0 Team-ready
  section Road to V0.3.0 (proposed, one change per release)
    V0.2.1 to V0.2.8 : After 0.2.0 (web residuals, jobs, console, incremental map)
    V0.2.9 to V0.2.18 : Mail, verification, notifications, accounts
    V0.2.19 to V0.2.31 : Remote repositories, pushes, issues and pull requests
    V0.2.32 to V0.2.39 : Assistant memory, cross-workspace search, palette, languages, review queue
    V0.2.40 to V0.2.48 : Export and import, encrypted backups, container, accessibility
    V0.2.49 to V0.2.56 : One client, release candidate : V0.3.0 Connected
```

| Stage | Status |
|---|---|
| Phase 1: Local memory loop (1A–1H) | ✅ Complete |
| Phase S: Shared team mode | ✅ Complete |
| Phase D: Depth and operability (V0.1.33–V0.1.65) | ✅ Complete |
| Security review 2026-10-10, second and third passes | ✅ Every finding fixed, mitigated or accepted in writing (V0.1.76); R2-4 and R2-5 scheduled for V0.2.8 |
| Road to V0.2.0 (V0.1.66–V0.1.76) | 🔄 Built and versioned `0.2.0`; R6 (clean install) and the tag are the maintainer's |
| Road to V0.3.0 (V0.2.1–V0.2.56, one change each) | 🔜 Proposed 2026-10-14; decisions 7–14 to agree |
| V0.4.0 and later: scale-out, single sign-on, hosted | 💭 Direction |

---

## ✅ Delivered

### Phase 1: Local memory loop (desktop)

The goal was to run the whole local loop with AI kept optional: *create workspace → import → inspect → search → ask with citations → save memory*.

| Phase | Version | Delivered |
|---|---|---|
| 1A Skeleton | V0.1.0 | Tauri + React shell, Rust workspace, SQLite schema, workspaces |
| 1B Import and store | V0.1.2 | File and folder import, content-addressed blob store, artifacts view, ADRs 0002–0004 |
| 1C Chunking | V0.1.3 | Markdown chunking by heading, 100-line windows for code and text, indexing jobs |
| 1E Code symbols | V0.1.3 | tree-sitter symbols for TS/TSX/JS, Python and Rust, file outlines |
| 1D Full-text search | V0.1.4 | SQLite FTS5, safe query building, filters, highlighted snippets |
| 1F AI provider layer | V0.1.6–0.1.7 | Ollama (local) and OpenRouter (cloud, with explicit acknowledgement), cited summaries |
| 1G Semantic search and Ask | V0.1.8 | Local embeddings, hybrid retrieval, cited answers, "insufficient context" guard |
| Image understanding | V0.1.9 | Vision-provider descriptions make images searchable |
| 1H Memory cards | V0.1.10 | Durable cards with evidence links, search and Markdown export |

The detailed specs are in [docs/phases/](../docs/phases).

### Phase S: Shared team mode (server + web)

| Milestone | Version | Delivered |
|---|---|---|
| Server foundation | V0.1.12 | `repomemo-server` (Axum), health and session routes, worker process boundary |
| Accounts and shared evidence | V0.1.13 | Registration and login (Argon2 + JWT), organizations, workspaces, text evidence, indexing, search, memory, and the **web client** |
| File uploads | V0.1.14 | Upload of evidence files up to 10 MiB |
| Workspace membership | V0.1.15 | Add, change and remove members, with role rules |
| Workspace admin and AI | V0.1.16 | Rename and delete, capabilities, per-workspace AI providers, AI overview, activity log |
| Ask in shared mode | V0.1.17 | Cited Q&A, provider test, memory search |
| Retrieval depth | V0.1.18–0.1.19 | Artifact query filters, retrieval facets (type, language, source) |
| Profiles and metrics | V0.1.20 | User profile, password change, "workspace pulse" metrics |
| Collaboration | V0.1.21 | Task board, evidence comments, assigned tasks, activity calendar |
| Review and notifications | V0.1.22 | Evidence lifecycle (verified, outdated, superseded) with history, notifications, @mentions |
| Workflow depth | V0.1.23 | Saved searches, task checklists |
| Web experience | V0.1.24–0.1.29 | Dashboard, organization switcher, notifications page, profile page, workspace sections, grid and list evidence views |
| Organization administration | V0.1.27 | Organization rename and member management, with access flowing to all org workspaces |
| Word documents | V0.1.30–0.1.31 | `.doc`/`.docx` text extraction, indexing, Documents section with preview |
| Jobs API and live events | V0.1.32 | Job kinds, cooperative cancel, job listing, **SSE event stream** for job and activity events |

### Phase D: Depth and operability (V0.1.33–V0.1.65)

| Milestone | Version | Delivered |
|---|---|---|
| Automatic and incremental indexing | V0.1.33–0.1.40 | Notes and uploads are indexed in the background with retries (30 s, 2 min, 10 min) and restart recovery. Failure reasons are stored and shown (migration 0012). Unchanged chunks keep their identity and embeddings. An `INDEXER_VERSION` bump refreshes older indexes. Code chunking follows declarations |
| Notes and folders | V0.1.40 | TipTap rich-text notes stored as Markdown, nested evidence folders up to 5 levels (migration 0013) |
| Business documents | V0.1.42–0.1.43 | Excel, PowerPoint, PDF, OneNote and Outlook text extraction and previews. Signed "Open in Office" links. Layout-accurate PDF previews through LibreOffice when installed |
| Workspace assistant | V0.1.45 | Closed capability router (find files, search, summarize, ask, overview) with stored per-user conversations (migration 0014) |
| Semantic search in shared mode | V0.1.46 | Embedding providers (Ollama, OpenRouter), a background embedding queue, hybrid FTS + vector search with rank fusion in Retrieval, Ask and the assistant, and an LLM rerank in Ask |
| Knowledge map | V0.1.48 | Indexing pipeline, coverage per kind, and a relation graph of similar files and memory-card citations |
| Workspace health | V0.1.50 | Deterministic checks (older versions, duplicates, removed symbols still mentioned, outdated references, failed indexing, unconnected files), each with actions and recorded outcomes (migration 0015) |
| Repository sync, phase 1 | V0.1.52–0.1.56 | Local git repositories as living sources (migration 0017). The branch is polled and synced when it moves. One evidence item per repository with a content overview, stack detection and an on-request cited AI summary |
| Security hardening | V0.1.51–0.1.57 | Rotating refresh tokens, session versions and "sign out everywhere", encrypted provider keys, sign-in throttling and lockout, workspace AI policy and per-user quotas, provider-URL and repository-root guards, security headers, typed 404s, blob garbage collection and hourly maintenance (migrations 0016, 0018) |
| System area | V0.1.58 | Overview, usage, users, run-time settings, logs (live and daily files, per-category levels), jobs and maintenance, audit trail (migration 0019) |
| Setup, avatars, app administrators | V0.1.59 | First-run setup wizard with a one-time code, profile pictures, app administrators without wildcard access (migrations 0020–0022) |
| Environment folders | V0.1.60 | Choose, create, verify and detach data environments under `workspace-data/` from the browser |
| Admin console | V0.1.61 | System › Console and the `repomemo-console` terminal client, with dry runs and audited changes |
| Daily-use polish and live progress | V0.1.62 | AI output rendered as Markdown with numbered citations; safe search highlights; several citations per memory card; **Save as memory** from Ask and the overview; several files or whole folders per upload (three at a time, with progress); **live progress** over a fetch-based event-stream reader (ledger, artifact page, repositories, tasks, memory, activity, a status strip with Stop) and `GET /v1/system/events` for System › Jobs, with polling as the fallback; the assistant docked beside every workspace section; the chosen environment remembered across restarts (`workspace-data/.last-environment`) unless one is pinned |
| Onboarding and API tokens | V0.1.63 | **Invitation links** for one email address and role, single-use, 7 days, withdrawable, usable even when registration is closed, re-checked against the inviter's rights on acceptance and never lowering a role; **password reset links** issued in System › Users (one-time, 24 hours, end every session, lift lockouts; app administrators cannot reset a system administrator); **personal access tokens** (`rmp_…`, read-only or read and write, 7–365 days, revocable, last use shown) for scripts, CI and `repomemo-console` (migration 0023) |
| Security ship-blockers (S1) | V0.1.64 | **H1:** adding someone to one workspace, directly or by invitation, gives that workspace only; their organization shows as a guest entry. **H4:** registering an address listed in `REPOMEMO_SYSTEM_ADMIN_EMAILS` grants nothing; only accounts that exist at startup are promoted. **L2:** token limits are marked on the handlers (`SessionSubject`, `ReadingSubject`) instead of a path denylist. **L3:** link secrets are in the URL fragment (`/invite#…`, `/reset-password#…`), with `no-referrer`; older path links are moved to the fragment. **M7:** only `VITE_` and `TAURI_ENV_` variables reach the bundle. People added before V0.1.64 keep the wider access they received; review organization members once |
| Security second pass | V0.1.65 | **N1 (high):** stored XSS through image previews closed; content types come from the file name, never the uploader (also I3), and previews open in a tab only for raster images. **N2:** run-time settings need a signed-in session. **N3:** owners keep their role, and organization changes never take a workspace from its owner. **N4:** viewers no longer change their old comments or tasks; saved searches are deleted by their author or an administrator. **N5:** a used reset link revokes personal access tokens. **N6:** the synchronous whole-workspace re-index is for administrators |

---

## 🎯 Road to V0.2.0

### What V0.2.0 means

A minor version marks a promise, not a pile of features. **V0.2.0, *Team-ready*,** promises that a team can install RepoMemo on a server others can reach, rely on it daily and recover it when something goes wrong. Concretely:

| Pillar | Promise | How it is checked |
|---|---|---|
| **Secure by default** | Every finding of the security review and its second pass is fixed, or accepted in writing with its mitigation. A fresh install is closed to strangers | Review status table shows no open item; a third pass before the release candidate finds nothing high or medium |
| **Checked on every commit** | Formatting, lints, Rust and web tests, type checks and dependency audits run in CI, and one version number is set in one place | CI green on `main`; System › Overview shows the real version |
| **Deployable and recoverable** | A documented production setup (TLS proxy, timeouts, service), backups that restore, quotas that protect the disk | The guide is followed on a clean machine; a backup is restored into a new environment and opens |
| **Memory stays true** | Memory cards say when their evidence changed; search can put trusted evidence first; code symbols are searchable | Feature tests, and the functional mindmap's gaps table is empty for these |
| **Changeable** | The web client and the server are split into modules with tests, so the next minor can move fast | No module over ~2,000 lines in `apps/server/src`, `crates/storage/src` or the shared web client (stylesheets and the frozen desktop `App.tsx` excepted) |
| **Fast enough to trust** | Search, Ask, the dashboard, map and health stay quick on a large workspace | Timings recorded for a 50,000-passage workspace in the release notes; targets in V0.1.74 |
| **Upgradable** | An environment from V0.1.40 or later upgrades in place, with a changelog and upgrade notes | Upgrade test on stored environments in CI |

**Out of scope for V0.2.0**, and proposed as V0.3.0 candidates: remote repositories (phase 2), mail delivery and email verification, assistant memory across turns, workspace export and import, connectors, and anything for scale-out. Each is valuable, but each widens the surface the V0.2.0 promise has to cover.

### Release plan

Each release is shippable on its own; the order puts what makes later work safer first.

| Release | Theme | Contents (IDs from the backlog below) | Why here |
|---|---|---|---|
| **V0.1.66** | Checkable releases | C8, C9, C10, P0, D1 | Every later release is then built, tested and versioned the same way, and P0 measures before any optimizing |
| **V0.1.67** | Safe defaults and identity | S2 (M1, M3, M4, M8, M9, L10, N2 residual), L4, L7 | Invitations made open registration unnecessary; this closes the biggest default risks |
| **V0.1.68** | Browser hardening | S4 (M2, M5, M6, L9), L3 follow-up | Turns any future injection from a takeover into a nuisance |
| **V0.1.69** | Host exposure | S3 (H2, H3, L8, M10) | The last two high findings; required before remote repositories |
| **V0.1.70** | Web client foundation | C1, C2, C3, C7, P6, P7 | Every product release after this touches the web client |
| **V0.1.71** | Server foundation | C4, C5, C6 (also L6), C12, N7, P8 | Same for the server; typed errors and timeouts land with the split |
| **V0.1.72** | Memory and search you can trust | F1, F2, F3, F8 | The product's core promise, on the new foundations |
| **V0.1.73** | Evidence, ownership, repositories 1b | F7, F11 (N8), F4 phase 1b, F9 | Daily ergonomics and the remaining integrity gap |
| **V0.1.74** | Performance and robustness | P1–P5, P9, S5 (M11, M12, L1, L5) | Measured by P0, on code that is now easy to change |
| **V0.1.75** | Operations | O1, O3, O4, O5, F9 backup commands | What an operator needs before trusting the server |
| **V0.1.76** | Release candidate | R1–R6 | Freeze features; prove the promise |
| **V0.2.0** | Team-ready | Fixes from the candidate only | — |

### Decisions (agreed 2026-10-13)

Each proposal below was accepted as written. They shape the releases named in the second column.

| # | Decision | Applies from | Agreed |
|---|---|---|---|
| 1 | **What app administrators are** (M8) | V0.1.67 | Read-only System pages plus jobs and maintenance; settings, users' sessions, lockouts and reset links stay with system administrators |
| 2 | **Where the refresh token lives** (M2) | V0.1.68 | An `HttpOnly; Secure; SameSite=Strict` cookie scoped to `/v1/auth/refresh`, which needs the SPA and API on the same site (one host behind the proxy) |
| 3 | **The desktop app's future** (F10) | V0.1.70 | Freeze it as the local-loop edition: security fixes only, labelled as such, and no ports of web features. Revisit as a server client in V0.3.0 |
| 4 | **The `apps/worker` stub** (C12) | V0.1.71 | Remove it; the durable queue belongs to the scale-out direction |
| 5 | **Mail in V0.2.0 or V0.3.0** (F6) | V0.1.67 | V0.3.0. Closed registration plus invitations covers M4 for V0.2.0 |
| 6 | **Version format** | V0.1.66 | Semantic versions: `0.2.0`, then `0.2.1` for fixes; tags `v0.2.0` |

### Release checklists

#### V0.1.66: Checkable releases

- [x] **C8 CI** (`.github/workflows/ci.yml`) on every push to `main` and every pull request. Blocking: versions agree, `cargo test` on Linux and Windows (desktop crate excluded), `npm run typecheck`, `cargo deny check advisories sources` for the server's dependencies (policy in `deny.toml`), `npm audit --omit=dev --audit-level=high`. Advisory for now: rustfmt, clippy `-D warnings`, licenses, duplicate crates and desktop-only advisories (they become blocking in V0.1.71)
- [x] **C9 one version source:** `npm run version:set -- <version>` (`scripts/set-version.mjs`) sets `Cargo.toml`, `Cargo.lock`, both `package.json` files, `package-lock.json` and `tauri.conf.json`; `npm run version:check` in CI; the code now says 0.1.66; `CHANGELOG.md` starts with V0.1.63–V0.1.66
- [x] **P0 timings:** search, Ask, metrics, the artifact query, the knowledge map and health record their duration and size under the `timing` target, in a new **Performance** log category (`log_performance_level`, debug to see them), filterable by workspace in System › Logs
- [x] **D1 documentation housekeeping:** `docs/IMPLEMENTATION_TRACKER.md` retired, `PRODUCT.md` describes the team product, Postman v5 collection with the onboarding routes (local, `docs/api/` is git-ignored)
- [x] **C10 dependencies** (V0.1.71): `quick-xml` 0.41 in `repomemo-ingestion` (closes RUSTSEC-2026-0194 and -0195) and a `cargo update`; the license, ban and advisory check is blocking. Vite 5 stays: the esbuild advisory reaches the development server only, and `npm audit --omit=dev` is clean. The remaining duplicates arrive through Tauri and only warn

#### V0.1.67: Safe defaults and identity

- [x] **M1** `REPOMEMO_ALLOW_REGISTRATION` defaults to `false`; the first account still registers, the setup wizard asks, and `/health` no longer carries `registration_open` (I2). Storage quotas stay with V0.1.75 (O4)
- [x] **M4** member adds refuse anyone not already in the organization or workspace with the same `409 invitation_required`, account or not, and the web app offers the invitation link. Registration itself stays distinguishable while it is open (no mail to verify with until V0.3.0), which is why it is closed by default
- [x] **M8** `require_full_system_admin` for settings, roles, sessions, lockouts, reset links, raw log files and detach; workspace activity records hidden from app administrators. Test `app_administrators_read_the_system_pages_and_changes_need_the_system_scope`
- [x] **N2 residual** the `system` token scope (migration 0024); System changes need a session or that scope
- [x] **M9** `check_transport` at startup, `REPOMEMO_ALLOW_INSECURE_HTTP` for a trusted network, HSTS for an HTTPS public URL, console warning
- [x] **M3** `ProxyTrust` (`REPOMEMO_TRUST_PROXY` plus loopback or `REPOMEMO_TRUSTED_PROXIES`), rightmost untrusted entry
- [x] **L10** a refresh token this browser holds bypasses a lockout (per-address counters were already separate from the account-wide bar of five times as many failures)
- [x] **L4** notifications `password_reset_issued` and `password_reset_used`
- [x] **L7** `validate_password`: common list, at least five different characters, no email name

#### V0.1.68: Browser hardening

- [x] **M5, L9** `SafeMarkdown` replaces every `react-markdown` use: images become a note with text and address, external links show their host and open with `noopener noreferrer`. The test of the image rule moves to the frontend test suite (C3, V0.1.70), which needs the test runner that release adds
- [x] **M2** the Vite build injects the policy as a meta tag and writes `content-security-policy.txt` (with `frame-ancestors`) for the host; the refresh token is an `HttpOnly; SameSite=Strict` cookie handled by `web_session.rs` for web clients. The cookie path is `/v1/auth` (not only `/v1/auth/refresh`) so sign-in and sign-out receive it
- [x] **M6** strict `csp` and a `devCsp` that adds the Vite dev server
- [x] **L3 follow-up** the web app keeps the secret in memory and replaces the history entry

#### V0.1.69: Host exposure

- [x] **H2** built-in list (`127.0.0.1`, `localhost`, `::1`, `openrouter.ai`) unless `REPOMEMO_AI_ALLOWED_HOSTS` says otherwise (`*` for everything); a checking DNS resolver filters every connection (public addresses only for the built-in list); provider bodies go to the log, not to the user
- [x] **H3** `RepoLinkPolicy::disabled()` on a server without roots (disabled, not defaulted: an operator opts in by naming folders); the refusal outside the roots does not depend on whether the path exists
- [x] **L8** `repomemo_git::harden`: `protocol.allow=never` (the https/ssh allowlist arrives with remote repositories), no system or global configuration, no submodule recursion, `safe.directory` only for the configured folders
- [x] **M10** scrubbed environment, `REPOMEMO_SOFFICE_WRAPPER` for the operator's sandbox command, version and sandbox state in System › Overview and the server check. The sandbox itself is the operator's (recipes follow in the production guide, V0.1.75)

#### V0.1.70: Web client foundation

- [x] **C1** `SharedWebApp.tsx` is the shell; `src/web/` holds the pages (`WorkspaceDetail.tsx` was the largest at about 1,000 lines; split in V0.2.2), `constants.ts`, `format.ts`, `router.ts`, `navigation.tsx`, `common.tsx`. Splitting the sections of the workspace page into components of their own comes with the next product work that touches them
- [x] **C2** `web/router.ts` (`Route`, `parseRoute`, `routePath`); `lib/queryClient.ts` and `lib/useServerState.ts` (cache-backed state with a `useState`-like setter) used by the workspace page. The dashboard, profile and detail pages still load their own data
- [x] **P7** `workspaceInvalidation` names what an event makes stale; `load()` is a full invalidation, used after bulk changes and on resync
- [x] **P6** `web/lazy.tsx`: System area (with the console), map, document viewer, editor, health and repository panels
- [x] **C7** `scripts/generate-types.mjs` reads `crates/domain` (a regular subset of Rust, failing loudly on anything else). Server-local response types (token response, setup, System) stay hand-written
- [x] **C3** 27 tests in `src/test/`: session renewal, live events, Markdown safety, upload filter, routes, image preview rules, query cache. The link pages are covered through the router and session tests; their rendering tests came in V0.2.4

#### V0.1.71: Server foundation

- [x] **C4** 13 modules plus `tests/` (scripted, then compiled and tested unchanged); the largest is `system.rs` at 1,460 lines
- [x] **C5 + C6** nine storage modules; `repomemo_domain::{not_found, invalid, conflict}` and `classify`; the server maps by kind; unique violations are detected through sqlx, not text; the five handlers that returned storage errors as 400 use the mapping
- [x] **N7** `TimeoutLayer` as a `route_layer` before the stream routes are added; `serve.rs` is the accept loop with a header read timeout (tested with a half-sent request); `Telemetry::claim_event_stream` per account
- [x] **P8** `StorageEngine::reader()` serves 138 read statements, the writer pool has one connection; concurrency settings are environment variables listed in System › Overview. Not yet measured: the 50,000-passage run belongs to V0.1.74
- [x] **C12** removed
- [x] **C8 follow-up** `rustfmt.toml` (`max_width = 120`), every file formatted, clippy clean with `-D warnings`; the lint job and the licenses and bans job are blocking

#### V0.1.72: Memory and search you can trust

- [x] **F1** `StorageEngine::memory_review_reasons` (missing / retired / changed), `needs_review` on summaries, `review` on the detail, the `stale_memory_citation` detector, citations on `PUT /v1/memory-cards/{id}`. Because passages are identified by content, a cited passage that is still there has not changed; whole-file citations also count a later update
- [x] **F2** filters in `push_search_filters` (keyword and vector search), `favour_verified` in hybrid search (Ask and the assistant use it, and hide retired evidence by default)
- [x] **F3** `GET /v1/workspaces/{ws}/symbols`, a *Code symbols* box on Retrieval (a symbol search is not a filter of passage search: it answers where a name is defined), the `find_symbol` assistant capability
- [x] **F8** `useLiveRefresh` in both panels

#### V0.1.73: Evidence, ownership, repositories 1b

- [x] **F11 (N8)** `transfer_workspace_ownership` and `transfer_organization_ownership` (session only, owner only, activity and System audit); `remove_organization_member` refuses with 409 while the person owns a workspace of it
- [x] **F7** `StorageEngine::move_folder`, `POST …/artifacts/bulk`, `replace_artifact_content` (the content hash changes under the same id; indexing runs again)
- [x] **F4 phase 1b** `GitRepo::renamed_files` (`git diff --name-status -z -M50%`) pairs a vanished path with its new one during a sync; `SearchResult.commit` and `Citation.commit` come from `repo_files.commit_sha`. The desktop app does not get either (decision 3)
- [x] **F9** `logs --follow` is the terminal client polling with `--after`; `users reset-link` and `tokens list` are server commands

#### V0.1.74: Performance and robustness

- [x] **Targets**, at 50,000 passages: search under 300 ms and the dashboard under 500 ms (p95), map and health under 2 s on first load, then cached. Measured by `performance_at_scale` (`cargo test -p repomemo-storage --release performance_at_scale -- --ignored --nocapture`). The run recorded so far is a **debug build** (unoptimized SQLite included, so an upper bound): keyword search p95 406 ms, meaning search p95 919 ms and the first search 21 s while the index is built, figures p95 19 ms, content version 22 ms, file query 6 ms. The release-build run is **R4 in V0.1.76**
- [x] **P1** `crates/storage/src/vectors.rs`: one index per workspace (unit vectors in one `f32` matrix), checked by vector count on every search, updated in place by `upsert_embeddings`, rebuilt when the count differs; `REPOMEMO_VECTOR_INDEX_MB` bounds the memory and a workspace that does not fit falls back to the scan. The nearest vectors are widened until enough pass the filters. sqlite-vec stays unneeded until memory is the limit
- [x] **P2, P3, P4** `StorageEngine::query_artifacts` (filters, count and page in SQL), `workspace_artifact_stats` (`GROUP BY type` and language), `dashboard_counts` and `GET /v1/dashboard`; the dashboard page makes one call
- [x] **P5** `DerivedCache` keyed by `workspace_content_version` (counts and latest changes of files, passages, vectors, cards, links, states, health decisions, providers), ten-minute expiry
- [x] **P9, M11** `repomemo_ingestion::budget`: each reading on its own thread with `REPOMEMO_DOCUMENT_TIMEOUT_SECONDS`, abandoned readings counted and new ones refused after four; `heavy` rate limiter on search, symbols, metrics, map, health and the file query. A memory cap per reading is not done: it needs a subprocess
- [x] **M12** `BackgroundAiBudget` in `RepoMemoCore`, spent per embedding batch and per image description; the limit follows the run-time setting
- [x] **L1** 60-second links with a `jti`, `FileLinkUses` (first client, 30 s grace), `redacted_span` for request logs; **L5** `restrict_to_owner` in `secrets.rs`

#### V0.1.75: Operations

- [x] **O1** `apps/server/src/backup.rs`: `StorageEngine::snapshot_database` (`VACUUM INTO` on a read connection, so writers carry on), blobs copied with hard links to the previous backup's files, manifest written last and the folder renamed into place, `BackupStatus` polled by the page; restore into a new `workspace-data/<name>` folder (staged, then renamed) and verified by opening it; scheduled from the maintenance loop (`REPOMEMO_BACKUP_INTERVAL_HOURS`, `REPOMEMO_BACKUP_KEEP`). Console `backup list|create|restore|delete`. Not done: compressing or encrypting a backup, an offsite target (the guide shows rsync and robocopy)
- [x] **O3** [technical/operations.md](technical/operations.md): settings that must be right, nginx and Caddy with timeouts and unbuffered event streams, systemd and NSSM, readiness checks, limits, backup and restore drill, Prometheus scrape and alerts, the LibreOffice wrapper, upgrade and rollback, troubleshooting. Followed on a clean machine in V0.1.76 (R6)
- [x] **O4** `quotas.rs`: `check_incoming` (disk floor, storage per workspace, uploads per hour) on uploads, new versions and notes; `check_new_workspace`; `free_disk_bytes` (`GetDiskFreeSpaceExW` on Windows, `df` elsewhere) shown in System › Overview. Settings in the *Limits* group
- [x] **O5** `GET /v1/system/metrics`, an `Exposition` writer for the OpenMetrics text format; system administrators or a `system` token

#### V0.1.76: Release candidate

- [x] **R1** feature freeze: fixes only until V0.2.0 (see the note at the top)
- [x] **R2** third security pass: [review §12](security/2026-10-10-security-review.md#12-third-pass-2026-10-14-release-candidate-v0176). No high or medium; R2-1 (repository syncs ignored quotas) and R2-2 (key file copy in a backup) fixed; the status table is closed
- [x] **R3** `environments_from_older_versions_upgrade_in_place` (storage tests): V0.1.40, V0.1.52 and V0.1.65 schemas rebuilt from the migration files up to 13, 17 and 23, filled, upgraded and checked. The fixtures are synthesized from the migrations, not captured from old binaries
- [x] **R4** release-build run at 50,000 passages: meaning search p95 112 ms, keyword search p95 63 ms (target 300 ms), figures p95 6 ms and the dashboard's three grouped queries (target 500 ms), map 0.5 s and health 0.4 s on first load (target 2 s). Debug-build numbers (V0.1.74) were 10 times slower, which is why they were not the verdict
- [x] **R5** [api-reference](technical/api-reference.md) (generated), [auth-and-authorization](technical/auth-and-authorization.md), [operations](technical/operations.md), both mindmaps, `CHANGELOG.md`, [UPGRADING.md](../UPGRADING.md)
- [ ] **R6** a clean-machine install following only the production guide. **Needs the maintainer:** it needs a second machine and a running server, which this repository's rules keep from the automated work. `node scripts/smoke-test.mjs` is the checklist's last step; record the result here and fix what the guide got wrong before tagging 0.2.0

#### V0.2.0: Team-ready

- [x] The version is `0.2.0` everywhere (`npm run version:set -- 0.2.0`); the candidate's checks left nothing to fix
- [x] The changelog's V0.2.0 entry names the part of the promise each release delivered; V0.1.66–V0.1.76 stay as its detailed notes
- [ ] **R6** the clean-machine install (above); what it finds becomes V0.2.1
- [ ] Tag `v0.2.0` once R6 passes

---

## 🎯 Road to V0.3.0

> **Status:** proposed on 2026-10-14, revised the same day into single-purpose releases; not started. Work begins after `v0.2.0` is tagged. Decisions 7–14 below need agreeing first.

### What V0.3.0 means

V0.2.0 made RepoMemo trustworthy on its own server. **V0.3.0, *Connected*,** promises that it reaches where the team's knowledge already lives, and that people can come and go with their knowledge intact:

| Pillar | Promise | How it is checked |
|---|---|---|
| **Reaches remote code** | A repository on GitHub, GitLab or any `https`/`ssh` host is linked by URL, kept current on a schedule or by push, and never needs a checkout on the server's disk | A private repository syncs with a stored credential; a push arrives through a signed webhook; a clone that is too big, too slow or on a host not allowed is refused |
| **Talks by mail** | Invitations, reset links, mentions and assignments arrive by email; people reset a forgotten password themselves; addresses are verified | A mail sent through a test SMTP server; the "forgot password" answer is the same for every address; an unverified address cannot join |
| **Links the work around the code** | Issues and pull requests from GitHub and GitLab are evidence: searchable, citable, linked to the files and symbols they mention | A connected project's issues appear, update on change and show on the files they name |
| **Remembers the conversation** | The assistant answers follow-up questions in context, and people in several workspaces can search them together | A follow-up resolves "it" from the earlier turn; a cross-workspace search returns only what each workspace allows |
| **Portable** | A workspace moves between servers as an archive, and backups can be encrypted | An exported workspace imports on another server with its memory, citations, tasks and history; an encrypted backup restores only with its key |
| **One client** | The desktop app opens a RepoMemo server instead of keeping a separate local copy | The desktop shell signs in to a server and shows the same pages as the browser |
| **Still team-ready** | Everything V0.2.0 promised holds for the new surfaces | A fourth security pass on mail, webhooks, remote clones and connectors finds nothing high or medium; upgrade from 0.2.0 tested in CI |

**Out of scope for V0.3.0**, and kept for later: scale-out (PostgreSQL, object storage, a job queue across processes), single sign-on and enterprise policy controls, hosted deployment, and connectors beyond GitHub and GitLab (Linear, Jira).

### How the 0.2.x releases are cut

- **One change per release:** one fix, one feature, or one enhancement or update of one section. A release that would need "and" in its title is two releases.
- **Shippable alone:** each release passes CI, carries its tests, updates the mindmaps, the changelog and its row in [RELEASES_0.2.md](RELEASES_0.2.md), and is committed as `Update V0.2.N` with a short list.
- **Security comes with the surface:** the rules for a new outside connection (mail, remote git, webhooks, connectors) are part of the release that opens it, never a later one.
- **Fixes ride along** with the next release when they are small; a fix that needs its own release gets one (decision 6 named the format, not a fixes-only rule).
- **Feature freeze from V0.2.52:** only the release candidate's checks and their fixes until V0.3.0.

### Release plan

**Type:** 🐞 fix · ✨ feature · 🔧 enhancement · 📘 update (docs, process, dependencies)

#### Track A: After 0.2.0 (V0.2.1–V0.2.8)

Pays off what V0.2.0 left in the web client and the background work, while it is fresh and before new surfaces land on the same pages.

| Release | Type | Section | The one change | IDs | Done when |
|---|---|---|---|---|---|
| **V0.2.1** ✅ | 🐞 | Operations | Corrections the clean-machine install (R6) found in the production guide and the server checks | R6 follow-up | A second clean install follows the guide with no workaround |
| **V0.2.2** ✅ | 🔧 | Workspace page | The workspace page split into one component per section | C13a | No section component over 500 lines; behaviour unchanged, tests green |
| **V0.2.3** ✅ | 🔧 | Web data | The dashboard, profile, artifact and memory pages on the query cache | C13b | No page loads its own data outside the cache; live events refresh them |
| **V0.2.4** ✅ | 🔧 | Web tests | Rendering tests of the invitation and reset pages | C13c | Each state of both pages (valid, used, expired, signed in as someone else) has a test |
| **V0.2.5** ✅ | 📘 | Styles | `styles.css` split per component, dead rules removed | C11 | No stylesheet over 1,000 lines; no visual change |
| **V0.2.6** ✅ | 🔧 | Jobs | Manual re-index endpoints answer `202` with the job instead of indexing in the request | C14 | No request indexes in place; the admin dialog follows the job |
| **V0.2.7** ✅ | ✨ | Console | Workspace commands: `workspace reindex`, `repo sync`, `embeddings rebuild` (system administrators) | F9 | Each has a dry run and an audit line |
| **V0.2.8** ✅ | 🔧 | Map and health | Recompute only what a change touched, instead of the whole workspace | P11 | A one-file change recomputes in under 10% of a full run at 50,000 passages |

#### Track B: Mail (V0.2.9–V0.2.15)

| Release | Type | Section | The one change | IDs | Done when |
|---|---|---|---|---|---|
| **V0.2.9** ✅ | ✨ | System › Settings | SMTP delivery: settings with an encrypted password, an outbox with retries, *Send test mail*, console `mail test` | F6a, decision 7 | A test mail arrives through a TLS server; a slow server never holds a request; the SMTP host follows the outbound address rules |
| **V0.2.10** ✅ | ✨ | People | Invitations sent by email (the link is still shown to copy) | F6b | The invited address receives the link; a crafted name cannot add mail headers |
| **V0.2.11** ✅ | ✨ | Sign-in | "Forgot your password?" by email | F12, decision 8 | The same answer for every address; rate-limited; a one-time link valid 1 hour ends every session |
| **V0.2.12** ✅ | ✨ | Notifications | Mentions and task assignments sent by email | F6c | Sent once per event, never to the person who caused it |
| **V0.2.13** ✅ | ✨ | Accounts | Email verification on registration and on a changed address | F13, decision 9 | An unverified account cannot join, create or be added; invitations count as verification |
| **V0.2.14** ✅ | ✨ | Profile | Notification preferences: in the app, by mail, both or none, per kind | F14a | Each kind of notification honours the choice |
| **V0.2.15** ✅ | ✨ | Notifications | A daily digest by email | F14b | One mail a day at most, only when something happened |

#### Track C: Accounts (V0.2.16–V0.2.18)

| Release | Type | Section | The one change | IDs | Done when |
|---|---|---|---|---|---|
| **V0.2.16** ✅ | ✨ | System › Users | Deactivate and reactivate an account | F15a, decision 10 | A deactivated account cannot sign in or use tokens; owners must transfer first |
| **V0.2.17** ✅ | ✨ | System › Users | Anonymize a deactivated account | F15b, decision 10 | Personal data gone; what they wrote stays, credited to "Former member"; creator links no longer block (risk 12 closed) |
| **V0.2.18** ✅ | 🐞 | Registration | Open registration answers the same for a known address and mails its owner instead | M4 residual | Registration can be opened without revealing who has an account |

#### Track D: Remote repositories (V0.2.19–V0.2.26)

| Release | Type | Section | The one change | IDs | Done when |
|---|---|---|---|---|---|
| **V0.2.19** ✅ | 🔧 | Security | Groundwork for remote sources: an encrypted credential store per link and `REPOMEMO_GIT_ALLOWED_HOSTS` (remote links off when unset) | S6a, decision 11 | Credentials are never returned or logged; the host list is checked before any network call |
| **V0.2.20** ✅ | ✨ | Repositories | Link a remote repository over `https` with a token: partial bare clone, fetch on the poll schedule | F4 phase 2a | A private GitHub or GitLab repository syncs through the existing engine |
| **V0.2.21** ✅ | ✨ | Repositories | Remote repositories over `ssh` with a deploy key and pinned host keys | F4 phase 2b | An unknown host key is refused; the key never reaches the command line |
| **V0.2.22** | 🔧 | Repositories | Clone limits: size and time caps, the clone counted in the storage allowance, clone sizes in System › Overview | S6b | A clone over its limit stops and says why |
| **V0.2.23** | ✨ | Repositories | Push webhooks from GitHub, GitLab and a generic sender | F16a | An unsigned or wrongly signed call is refused (HMAC, constant time); a push queues one sync |
| **V0.2.24** | ✨ | Artifact page | Last author and date per repository file | F16b | Shown on the file and in search results, from one `git log` pass per sync |
| **V0.2.25** | ✨ | Artifact page | `CODEOWNERS` as ownership hints | F16c | The matching owners show on the file, as hints next to its lifecycle owner |
| **V0.2.26** | ✨ | Citations | View a cited passage at the commit it names | F16d | A citation opens the text as it was at that commit, labelled as such |

#### Track E: Issues and pull requests (V0.2.27–V0.2.31)

| Release | Type | Section | The one change | IDs | Done when |
|---|---|---|---|---|---|
| **V0.2.27** | ✨ | Connectors | GitHub issues as evidence (read-only token, incremental sync) | F17a, decision 12 | Issues and their comments are searchable and citable; closed issues are *outdated* |
| **V0.2.28** | ✨ | Connectors | GitHub pull requests as evidence | F17b | Pull requests with their review discussion, linked to their commits |
| **V0.2.29** | ✨ | Connectors | GitLab issues and merge requests | F17c | The same model as GitHub, including self-hosted GitLab |
| **V0.2.30** | 🔧 | Artifact page | Links from issues and pull requests to the files, symbols and commits they mention | F17d | A file shows the issues that name it |
| **V0.2.31** | 🔧 | Connectors | Connector webhooks and remote rate limits | F17e | Updates arrive on webhook; `Retry-After` is honoured; a revoked token pauses the connector and says so |

#### Track F: Search, assistant and review (V0.2.32–V0.2.39)

| Release | Type | Section | The one change | IDs | Done when |
|---|---|---|---|---|---|
| **V0.2.32** | ✨ | Assistant | Earlier turns go with each question, within a token budget | F5a | A follow-up resolves "it" and "that file" |
| **V0.2.33** | ✨ | Retrieval | Search across all of the caller's workspaces | F5b | Results merged by score, each checked against its workspace's role and AI policy |
| **V0.2.34** | ✨ | Web shell | A keyboard command palette | F19 | Jump to a workspace, file, memory card or setting, and start assistant actions, without the mouse |
| **V0.2.35** | ✨ | Indexing | Code symbols and structure-aware chunks for Go and Java | F21a | Both languages chunk by declaration and their symbols are searchable |
| **V0.2.36** | ✨ | Indexing | Code symbols and structure-aware chunks for C# and PHP | F21b | Same as V0.2.35 |
| **V0.2.37** | ✨ | Review | A review queue: evidence *needing review*, stale memory cards and open health findings in one list | F23 | Each item opens where it is resolved; the queue empties as they are |
| **V0.2.38** | ✨ | Artifact page | Version history of a file: earlier versions kept when a new one is uploaded, viewable and comparable | F24 | Uploading a new version keeps the old text readable |
| **V0.2.39** | 🔧 | Discussion | `@mention` suggestions while typing in comments | F25 | Only workspace members are suggested |

#### Track G: Portability and operations (V0.2.40–V0.2.46)

| Release | Type | Section | The one change | IDs | Done when |
|---|---|---|---|---|---|
| **V0.2.40** | ✨ | Workspace settings | Export a workspace as an archive | O2a | Evidence, folders, memory cards with citations, tasks, comments and lifecycle history in one file with a manifest |
| **V0.2.41** | ✨ | Workspaces | Import an archive into a new workspace, with a dry run | O2b | An export imports on another 0.2.x server with everything linked as before |
| **V0.2.42** | ✨ | Backups | Encrypted backups | O6a, decision 14 | A backup restores only with `REPOMEMO_BACKUP_KEY`; a wrong key is refused before anything is written (R2-5 closed) |
| **V0.2.43** | ✨ | Backups | A second copy of each backup to another folder or network share | O6b | The copy is verified against the manifest |
| **V0.2.44** | 🐞 | Indexing | Document readings in a subprocess with time and memory limits | P10 | An overrunning reading is killed, not abandoned (R2-4 closed) |
| **V0.2.45** | ✨ | Operations | A container image and a compose file with the proxy | O7 | `docker compose up` gives a working server behind TLS, with data in a volume |
| **V0.2.46** | ✨ | System › Audit | Export of the audit trail (JSON lines, CSV) | O8 | A date range downloads; the download itself is audited |

#### Track H: Experience (V0.2.47–V0.2.48)

| Release | Type | Section | The one change | IDs | Done when |
|---|---|---|---|---|---|
| **V0.2.47** | 🔧 | Web client | Accessibility pass: keyboard paths, focus, labels, contrast | X1 | Every page usable by keyboard alone; automated checks find no serious issue |
| **V0.2.48** | 🔧 | Web client | Narrow screens: the workspace pages and the assistant usable on a tablet or phone | X2 | No horizontal scrolling at 375 px; the dock becomes a sheet |

#### Track I: One client (V0.2.49–V0.2.51)

| Release | Type | Section | The one change | IDs | Done when |
|---|---|---|---|---|---|
| **V0.2.49** | ✨ | Desktop | The desktop app connects to a server: asks for its address, keeps the session in the operating system's credential store, shows the web client | F20a, decision 13 | The desktop window and the browser show the same pages for the same account |
| **V0.2.50** | ✨ | Desktop | *Export to a server*: a local workspace moves to a server through the archive format | F20b | A local workspace appears on the server with its memory and citations |
| **V0.2.51** | 📘 | Desktop | The local edition retired: labelled in the app and the docs, removal planned for V0.3.x | F20c | The docs no longer describe the local edition as current |

#### Track J: Release candidate (V0.2.52–V0.2.56)

| Release | Type | Section | The one change | IDs | Done when |
|---|---|---|---|---|---|
| **V0.2.52** | 📘 | Security | Feature freeze and the fourth security pass over mail, webhooks, remote clones, connectors, export and import, and the desktop client, with its fixes | R7, R8 | No high or medium finding open |
| **V0.2.53** | 📘 | Release | Upgrade test from 0.2.0 environments in CI | R9 | Stored 0.2.0 schemas upgrade in place and pass |
| **V0.2.54** | 📘 | Performance | Performance run at 50,000 passages plus a 100,000-file remote repository and 10,000 issues | R10 | The V0.2.0 targets still hold; results in the release notes |
| **V0.2.55** | 📘 | Documentation | Mindmaps, branch pages (`repository-sources`, a new `connectors` page, `operations`), changelog and upgrade notes from 0.2.x | R11 | Every page matches the code |
| **V0.2.56** | 📘 | Release | Clean-machine install with mail and a remote repository, following only the guide | R12 | Installed without a workaround; ready to tag |
| **V0.3.0** | — | — | *Connected* | — | Tag `v0.3.0` |

### Release checklists (0.2.x)

#### V0.2.1: Production guide and server checks

- [x] The guide's variables, defaults, routes, metrics and commands compared with the code (`config.rs`, `setup.rs`, `main.rs`, `backup.rs`, metrics): no other difference
- [x] `public_url` and `backups` checks, `proxy` warns on HTTPS without `REPOMEMO_TRUST_PROXY`; advice only when something is missing (asserted in the first-run test in `tests/auth.rs`)
- [ ] A second clean install with no workaround: the maintainer's, with R6

#### V0.2.2: Workspace page split

- [x] **C13a** `web/WorkspaceDetail.tsx` (320 lines) is the shell; `web/workspace/` has seven section components (largest 371 lines), `data.ts` (the server-state hooks, one per cache key) and `page.ts` (`WorkspacePage`, `sectionState`)
- [x] Behaviour kept: sections remember their state in a per-workspace store while away; `key={workspaceId}` restarts them in another workspace
- [x] Tests: `src/test/sectionState.test.tsx`; 30 tests, typecheck clean

#### V0.2.3: Remaining pages on the query cache

- [x] **C13b** `Dashboard.tsx`, `Profile.tsx`, `Notifications.tsx`, `ArtifactDetail.tsx` (and its chunks dialog) and `MemoryCardDetail.tsx` use `useServerState`; keys in `queryClient.ts` (`accountKeys`, and `workspaceKeys.artifact*` / `memoryCard` under the workspace); `staleTime: 0` where an old answer must not linger
- [x] Live events: the evidence page invalidates its entries; the memory card page opens the workspace stream and refreshes the card on other people's edits and on evidence changes
- [x] Kept: the avatar check has its own small module cache (`fetchSharedAvatar`); the app shell's organization and workspace lists load once per session
- [x] Tests: two more in `queryState.test.tsx` (32)

#### V0.2.4: Link-page rendering tests

- [x] **C13c** `src/test/linkPages.test.tsx` renders `InvitePage` and `PasswordResetPage` against a mocked API: every state of both pages (valid, used, expired, withdrawn, signed in as someone else, failing to join, no reason given) has a test; 19 tests, 51 in the web suite
- [x] No page split was needed: the pages take their session and callbacks as props

#### V0.2.5: Stylesheets split

- [x] **C11** `src/styles/01…12-*.css` (largest 879 lines) imported in order by `styles/index.ts`; `styles.css` and `blueprint-refinement.css` removed; `components.json` points at the tokens file
- [x] Dead rules: a class counts as live when its name appears anywhere in the web client, the Rust sources or the HTML, or is built from a prefix or suffix found there, or belongs to TipTap; 47 rules or selectors removed. A diff of the old and new text shows deletions only
- [x] `npm run styles:check` (and CI) keeps every stylesheet under 1,000 lines and the import list complete and in order
- [x] No build was run for this release: this repository keeps builds to the maintainer, so the check is the diff of old and new text and the script; the result is seen when the app is opened

#### V0.2.6: Asynchronous manual re-index

- [x] **C14** `RepoMemoCore::begin_index_artifact` / `finish_index_artifact` and `begin_index_workspace` / `finish_index_workspace` (`PendingArtifactIndex`, `PendingWorkspaceIndex`); the handlers answer `202` and spawn the second half; `index_artifact` and `index_workspace` stay as the two halves in a row for the queue and the tests
- [x] `409` while a whole-workspace pass runs (`workspace_reindex_running`); cancel through the existing job endpoint
- [x] Web: `getSharedJob` and `waitForSharedJob` (`lib/sharedApi.ts`); the chunks dialog follows the job
- [x] Tests: `indexing_publishes_jobs_that_can_be_listed_and_cancelled` now expects `202` and follows both jobs; `reindex_tests` for the conflict rule; `jobs.test.ts` (55 web tests)

#### V0.2.7: Console workspace commands

- [x] **F9** `workspace reindex`, `repo sync`, `embeddings rebuild` in `system/console.rs`, with `workspace list` and `repo list` to look things up; system administrators only (`require_full_system_admin`); each is a dry run until `--yes` and audited as `console_command`
- [x] `StorageEngine::clear_workspace_embeddings`, `RepoMemoCore::{embedding_rebuild_plan, clear_embeddings, stale_artifact_count}`
- [x] Tests: `the_console_starts_workspace_work_as_dry_runs_then_audited_jobs`, `the_console_rebuilds_a_workspaces_embeddings_after_a_dry_run`

#### V0.2.8: Incremental map and health

- [x] **P11** `crates/api/src/meaning.rs`: per file `FileMeaning` (passages, embedded, centroid, stamp), per comparison `PairSimilarity` (map, health) reused for pairs of unchanged files; `RepoMemoCore::file_meanings` and `similarity_edges_cached`; the map and health use them (`knowledge_map.rs`, `health.rs`)
- [x] `StorageEngine::vector_stamps` (journal in `vectors.rs`, written by `upsert_embeddings`, `clear_workspace_embeddings`, `replace_artifact_index`, `remove_repo_file`); `chunk_counts_for_artifacts`, `artifact_embedding_centroids_for` read named files through the index on `chunks.artifact_id`
- [x] Measured at 50,000 passages (debug build): full run 5.0 s, one changed file 233 ms (4.6%), nothing changed 0.2 s; the run asserts under 10%
- [x] Tests: `the_map_and_health_after_a_change_match_a_cold_computation`, three in `meaning::tests`, journal assertions in `vector_search_uses_the_index_and_agrees_with_a_scan`
- [ ] A release-build run of `map_and_health_at_scale` for the release notes (the maintainer's: this repository keeps release builds to them)

#### V0.2.9: SMTP delivery

- [x] **F6a** `crates/mail` (`compose`, `send`, `OutboundPolicy`); migration 0025 (`mail_settings`, `mail_outbox`); `StorageEngine::{mail_settings, save_mail_settings, enqueue_mail, claim_due_mail, …}`; `apps/server/src/mail.rs` (`MailService`, the worker, `/v1/system/mail*`); `MailSettingsPanel.tsx`; console `mail status` and `mail test`
- [x] Decision 7: SMTP only, TLS required (implicit or STARTTLS), password sealed like provider keys, an outbox with retries; the host follows the outbound address rules (`repomemo_ai::is_forbidden_address`, `is_non_public_address`, `is_metadata_host`) with `REPOMEMO_SMTP_ALLOWED_HOSTS` for private relays and `REPOMEMO_SMTP_CA_FILE` for a private authority
- [x] Tests: 15 in `repomemo-mail` (TLS and STARTTLS against a scripted server, refusals, injection, policy), storage (sealed password, outbox), server (`mail_is_set_up_in_system_settings_and_sent_from_the_outbox`)
- [ ] A real mail server (the maintainer's): send a test message through the production guide's setup

#### V0.2.10: Invitations by email

- [x] **F6b** `mail::mail_invitation` / `invitation_message` called from `create_workspace_invitation` and `create_organization_invitation`; `CreatedInvitation.mail`; `InviteMembers.tsx` toasts the outcome; the link stays in the dialog
- [x] A crafted name cannot add headers: `plain()` cleans names for the subject, and `compose` refuses control characters in any header (V0.2.9)
- [x] Tests: `an_invitation_names_who_where_and_for_how_long_and_a_crafted_name_adds_nothing`, `an_invitation_is_mailed_through_the_outbox_when_mail_is_set_up`

#### V0.2.11: Forgot your password

- [x] **F12** (decision 8) `access::forgot_password` and `send_reset_link`; the answer is fixed and sent before any lookup; 3 requests an hour per address through `guards.auth` (`forgot:{email}`), plus `check_auth_quota`; link life 1 hour (`SELF_SERVICE_RESET_TTL_HOURS`); notification `password_reset_issued`; web: *Get a link by email* in `AuthCanvas.tsx`
- [x] Test: `forgot_password_answers_the_same_for_every_address_and_mails_only_a_real_one`
- [ ] Try it with a real mailbox (the maintainer's)

#### V0.2.12: Mentions and assignments by mail

- [x] **F6c** `mail::mail_notification` / `NotificationMail`, called from `notify_task_assignee` and `notify_artifact_mentions` (`profile.rs`) right after the in-app notice, so one event is one mail; `actor == recipient` returns early
- [x] Test: `assignments_and_mentions_are_mailed_once_and_never_to_the_person_who_caused_them`; unit: `a_notification_mail_says_who_what_and_where_and_a_crafted_title_adds_nothing`

#### V0.2.13: Email verification

- [x] **F13** (decision 9) migration 0026; `StorageEngine::{email_is_verified, mark_email_(un)verified, create_email_verification, consume_email_verification, change_user_email}`; `apps/server/src/verification.rs` (`enforced`, `require_verified`, `send_link`, handlers); registration flags unverified (not for invitations or the first account); `accept_invitation` verifies; `create_organization` and `create_workspace` require it; `VerifyEmailPage`, profile section
- [x] "Join" and "be added" already go through invitations since M4 (V0.1.67), and accepting one verifies; "create" is what is withheld
- [x] Enforcement is "registration open and mail ready" (`verification::enforced`): without mail nothing is withheld
- [x] Tests: `unverified_accounts_are_held_back_until_an_invitation_or_a_link_proves_the_address`, `an_address_is_verified_once_by_a_link_made_for_it`, three web tests (58)

#### V0.2.14: Notification preferences

- [x] **F14a** migration 0027; `StorageEngine::{notification_channel, notification_preferences, set_notification_preference}`; `profile::delivery` decides `(in the app, by mail)` for `task_assigned` and `evidence_mention`, everything else (security notices) is always both; `GET`/`PUT /v1/profile/notification-preferences`; Profile › Notifications
- [x] Test: `a_person_chooses_where_each_kind_of_notice_reaches_them` (none, app, mail, both against the notice list and the outbox)

#### V0.2.15: Daily digest

- [x] **F14b** migration 0028; `StorageEngine::{digest_candidates, unread_notices_since, record_digest_sent}`; `MailService::queue_digests` runs on every pass of the sender and in the same step wakes on a preference change; `digest_message`; the `daily_digest` kind (default `none`, `mail` or `none`) in `profile.rs`
- [x] Test: `the_daily_digest_is_asked_for_and_sent_once_when_something_is_unread`; unit: `a_digest_counts_and_lists_the_newest_and_a_crafted_title_adds_nothing`
- [ ] Track B (mail) is built: a real mail server, a real mailbox and the clean install with mail are the maintainer's checks

#### V0.2.16: Deactivate and reactivate an account

- [x] **F15a** (decision 10) migration 0029 (`users.deactivated_at`); `crates/storage/src/accounts.rs` (`is_deactivated`, `owned_places`, `deactivate_user` in one transaction, `reactivate_user`); `user_access` and `user_session_version` answer `None` for a deactivated account, so tokens of either kind, refresh and the stream watchers (E15) stop at once; file links carry the asker (`uid`) and are re-checked
- [x] Refused: oneself, the last active system administrator (`system_admin_count` counts active ones), an owner of an organization or workspace; sign-in says *deactivated* only after the right password; no reset link, notification mail or digest for a deactivated account; member forms refuse it
- [x] `POST /v1/system/users/{u}/deactivate` and `/reactivate`; console `users deactivate` and `users reactivate` (dry run until `--yes`); System › Users menu and *Deactivated* tag; `SystemUser.deactivated_at`
- [x] Test: `a_deactivated_account_is_shut_out_everywhere_and_comes_back_when_reactivated`
- [x] Anonymizing is V0.2.17, which also covers names in activity summaries

#### V0.2.17: Anonymize a deactivated account

- [x] **F15b** (decision 10) migration 0030 (`users.anonymized_at`); `StorageEngine::{anonymize_user, is_anonymized}` in one transaction, only for a deactivated account that owns nothing; the row stays as `Former member` with an unreachable address and an unusable password hash, roles removed
- [x] Removed: avatar, memberships, notices, preferences, digest record, assistant conversations, tokens, links, mail waiting for the address (sent ones lose address and subject), invitations made out to it; tasks assigned to it are unassigned
- [x] Rewritten: the address in activity summaries and system audit details; the name on the account's own activity and audit rows. **Remains:** log files and earlier backups, and other people's text that names the person
- [x] `POST /v1/system/users/{u}/anonymize` (`{confirm}` is the address); console `users anonymize`; System › Users dialog that asks for the address; `SystemUser.anonymized_at`
- [x] Test: `an_anonymized_account_loses_its_personal_data_and_what_it_wrote_stays`
- [x] Risk 12 (creator links block deletion) closed: nothing deletes the row

#### V0.2.18: Registration without revealing who has an account

- [x] **M4 residual** `auth::register_without_revealing` (used when registration is open, mail is ready, the public address is known, and the request has no invitation and is not the first account): `202` with one message for every address; the account of a new address is created unverified with its link mailed; a known address mails its owner (`registration_attempt`, 2 an hour through `guards.auth`, none for a deactivated account)
- [x] The password is hashed on every path; response times are close, not identical (documented)
- [x] Web: `signUpSharedUser`; the sign-up form shows the message and switches to sign in
- [x] Tests: `open_registration_answers_the_same_for_a_known_address_and_mails_its_owner` (same status and body, the known account untouched, two notices for three attempts); the email-verification test signs in after registering

#### V0.2.19: Groundwork for remote sources

- [x] **S6a** (decision 11) `crates/api/src/remote_policy.rs` (`RemoteGitPolicy`, `RemoteRepository`): only `https` and `ssh`, no userinfo with a secret, no transports, options, local paths or control characters, a valid host, host allow list (`host`, `host:port`, `*.suffix`, `*`), metadata and link-local literals never allowed; `RepoMemoCore::with_remote_git_policy`
- [x] `REPOMEMO_GIT_ALLOWED_HOSTS` in `config.rs` (unset: remote links off), the *Remote repository hosts* server check and overview fact
- [x] Migration 0031 `repo_credentials`; `crates/storage/src/credentials.rs`: `set_repo_credential`, `repo_credential_info`, `open_repo_credential` (only for the sync engine), `clear_repo_credential`; sealed with context `repo_credentials:{source}:secret`; `RepoSecret` hides its value in `Debug` and has no `Serialize`
- [x] Tests: three in `remote_policy` (off by default, forms accepted and refused, metadata and private addresses), `a_credential_is_sealed_never_shown_and_dies_with_its_link`
- [ ] The git version check (2.50.1 or later), the clone itself and the credential endpoints are V0.2.20

#### V0.2.20: Remote repositories over https

- [x] **F4 phase 2a** `crates/git/src/remote.rs`: `remote_head` (`ls-remote --symref`), `clone_bare` (`--bare --no-tags --single-branch --filter=blob:limit=1m`), `fetch_branch`, cleared environment, `protocol.https.allow=always`, no redirects, `transfer.fsckObjects`, `GIT_ASKPASS` through `run_as_askpass_if_requested` (called first in `main`), patched-git check, tokens removed from messages; `GitRepo::open_bare` and sizes from `cat-file --batch-check`
- [x] `crates/api/src/remote_repo.rs`: `check_remote_repo_link`, `connect_remote_repo` (token stored sealed, link removed if that fails), `set_repo_token`, `clear_repo_token`; the address goes through `RemoteGitPolicy` on every use; `RepoSource.remote` and `has_credential`; metadata `remote.url`
- [x] `PUT`/`DELETE /v1/repositories/{id}/token`; web: link form with token fields, remote and token tags, *Remove the token*
- [x] Tests: `a_bare_partial_clone_is_read_like_any_repository`, the remote git unit tests, `remote_links_are_off_until_hosts_are_named_and_a_token_is_never_echoed`
- [ ] `ssh` and deploy keys are V0.2.21; clone limits V0.2.22; a clone against a real host is for the maintainer to try

#### V0.2.21: Remote repositories over ssh

- [x] **F4 phase 2b** `SshAuth`, `known_hosts_lines` (pasted key to `known_hosts` lines for the link's own host and port; certificate authorities and revoked keys refused), `SshFiles` (private folder, `0700`/`0600`, removed on drop), `GIT_SSH_COMMAND` with `StrictHostKeyChecking=yes`, `IdentitiesOnly`, `BatchMode`, no agent, no passwords, `-F /dev/null`; `protocol.ssh.allow` in place of `https`
- [x] `RemoteLink.host_key`; `remote_target` picks token or deploy key by the address; both required for `ssh`; `set_repo_deploy_key` tries the key first
- [x] `PUT /v1/repositories/{id}/deploy-key`; web: deploy key and host key fields for `ssh` addresses, *deploy key* tag
- [x] Tests: `host_keys_are_pinned_to_the_host_and_port_of_the_link`, `a_deploy_key_lives_in_a_private_file_and_never_on_the_command_line`, the link-refusal list now includes an `ssh` address without a key
- [ ] A connection to a real host is for the maintainer to try; clone limits are V0.2.22

### Decisions to agree before work starts

| # | Decision | Needed by | Proposal |
|---|---|---|---|
| 7 | **Mail transport** | V0.2.9 | SMTP only (any provider offers it), TLS required (implicit or STARTTLS), the password encrypted like provider keys; an outbox table with retries so a slow mail server never holds a request |
| 8 | **Self-service password reset** | V0.2.11 | Yes, by email, alongside the administrator's reset links: the same answer for every address, rate-limited per address and per account, a 1-hour one-time link, and the reset notifications from V0.1.67 |
| 9 | **Registration with verification** | V0.2.13 | Registration stays closed by default; when an operator opens it, verification is mandatory and an unverified account can sign in but not join or create anything |
| 10 | **Account deletion** (risk 12) | V0.2.16 | Deactivate, then anonymize: the account's sessions, tokens, avatar and email go; what they wrote stays, credited to "Former member". Only system administrators, audited; an owner must transfer ownership first |
| 11 | **Who links remote repositories, and where from** | V0.2.19 | Workspace administrators, only to hosts in `REPOMEMO_GIT_ALLOWED_HOSTS` (unset: remote links off, like `REPOMEMO_REPO_ROOTS` for local ones); `https` and `ssh` only; credentials per link (a read-only token or deploy key), encrypted, never shown again |
| 12 | **First connectors** | V0.2.27 | GitHub (including GitHub Enterprise Server) and GitLab, read-only, issues and pull/merge requests with their discussion; Linear and Jira later |
| 13 | **The desktop app** (revisits decision 3) | V0.2.49 | The desktop shell becomes a client: it asks for a server address, signs in and shows the web client. The local edition is retired after one release that offers "export to a server" |
| 14 | **Backup encryption** | V0.2.42 | AES-256-GCM in streaming chunks, key from `REPOMEMO_BACKUP_KEY` (never stored in the backup); off by default, recommended in the guide; restore refuses a wrong key before writing anything |

---

## 📚 Backlog reference

Every item of both roads, with the release it is planned for. The evidence behind each is in the [enhancement analysis](analysis/2026-10-13-enhancement-analysis.md) and the [security review](security/2026-10-10-security-review.md).

### Product features

| ID | Item | Size | Target |
|---|---|---|---|
| F1 | **Memory that stays true.** Stale-citation detector, *needs review* on cards, citation editing | S–M | ✅ V0.1.72 |
| F2 | **Search by trust.** Lifecycle, folder and date filters; boost verified evidence | S | ✅ V0.1.72 |
| F3 | **Symbol search on the web** | S | ✅ V0.1.72 |
| F4 | **Repositories.** Phase 1b (renames with edits, commit on citations) | M | ✅ V0.1.73 |
| F4 | **Repositories.** Phase 2: remote by URL over `https` (2a) and `ssh` (2b) | L | V0.2.20, V0.2.21 |
| F5 | **Assistant memory** (5a) and **cross-workspace search** (5b) | M | V0.2.32, V0.2.33 |
| F6 | **Mail delivery:** SMTP and outbox (6a ✅ V0.2.9), invitations (6b ✅ V0.2.10), mentions and assignments (6c ✅ V0.2.12) | M | V0.2.9, V0.2.10, V0.2.12 (decisions 5, 7) |
| F7 | **Evidence ergonomics.** Move folders, multi-select, new version of a file | M | ✅ V0.1.73 |
| F8 | **Live map and health** | S | ✅ V0.1.72 |
| F9 | **Console follow-ups.** `logs --follow`, `users reset-link`, `tokens list`, backup commands; workspace commands for system administrators | S–M | ✅ V0.1.73, V0.1.75, V0.2.7 |
| F10 | **Desktop decision:** frozen as the local edition, security fixes only | Decision | ✅ Decided (decision 3) |
| F11 | **Ownership transfer** and no removal of a workspace owner (N8) | S | ✅ V0.1.73 |
| F12 | **Self-service password reset** by email, the same answer for every address | S | ✅ V0.2.11 (decision 8) |
| F13 | **Email verification** for self-registration and changed addresses | S–M | ✅ V0.2.13 (decision 9) |
| F14 | **Notification preferences** (14a ✅ V0.2.14) and a **daily digest** (14b ✅ V0.2.15) | S | V0.2.14, V0.2.15 |
| F15 | **Account lifecycle:** deactivate and reactivate (15a ✅ V0.2.16), anonymize (15b ✅ V0.2.17) (risk 12) | M | V0.2.16, V0.2.17 (decision 10) |
| F16 | **Push webhooks and git awareness:** signed webhooks (16a), last author and date (16b), `CODEOWNERS` hints (16c), a citation at its commit (16d) | M | V0.2.23–V0.2.26 |
| F17 | **Issues and pull requests:** GitHub issues (17a), GitHub pull requests (17b), GitLab (17c), links to files and symbols (17d), webhooks and rate limits (17e) | L | V0.2.27–V0.2.31 (decision 12) |
| F19 | **Command palette** | S | V0.2.34 |
| F21 | **More languages:** symbols and structure-aware chunks for Go and Java (21a), C# and PHP (21b) | S–M | V0.2.35, V0.2.36 |
| F23 | **Review queue:** evidence needing review, stale memory cards and open health findings in one list | S–M | V0.2.37 |
| F24 | **Version history** of a file, viewable and comparable | M | V0.2.38 |
| F25 | **`@mention` suggestions** in comments | S | V0.2.39 |
| F20 | **Desktop as a server client** (20a), export to a server (20b), local edition retired (20c) | M | V0.2.49–V0.2.51 (decision 13) |

### Security

| ID | Bundle | Findings | Target |
|---|---|---|---|
| S1 | Ship-blockers for onboarding | H1, H4, L2, L3, M7 | ✅ V0.1.64 |
| — | Second pass | N1–N6, I3 | ✅ V0.1.65 |
| S2 | Safe defaults and identity | M1, M3, M4, M8, M9, L4, L7, L10, N2 residual | ✅ V0.1.67 |
| S4 | Browser hardening | M2, M5, M6, L9 | ✅ V0.1.68 |
| S3 | Host exposure | H2, H3, L8, M10 | ✅ V0.1.69 |
| — | Server timeouts | N7 | ✅ V0.1.71 |
| S5 | Robustness | M11, M12, L1, L5, L6 | ✅ V0.1.71, V0.1.74 |
| — | Integrity | N8 | ✅ V0.1.73 |
| — | Supply chain | dependency scanning (review §7) | ✅ V0.1.66 (CI) |
| — | Third pass | R2-1, R2-2 fixed; R2-3, R2-6 accepted | ✅ V0.1.76 |
| — | Accepted for later | R2-4 (readings cannot be killed), R2-5 (backups not encrypted) | V0.2.44 (P10), V0.2.42 (O6) |
| — | Accepted | N9 (not a boundary, documented), I1, I4 | — |
| S6 | Remote sources | Credential store and host allowlist (6a ✅ V0.2.19), clone limits (6b); each later surface (SMTP, webhooks, connectors) brings its own rules | V0.2.19, V0.2.22 |
| — | Registration without enumeration | M4 residual | ✅ V0.2.18 |
| — | Fourth pass | Mail, webhooks, remote clones, connectors, export and import, desktop client | V0.2.52 (R8) |

### Platform performance

| ID | Item | Size | Target |
|---|---|---|---|
| P0 | Timings (Performance log category) | S | ✅ V0.1.66 |
| P1 | Vector search without a full scan | M | ✅ V0.1.74 |
| P2 | `artifacts/query` in SQL | S | ✅ V0.1.74 |
| P3 | Workspace metrics as SQL aggregates | S–M | ✅ V0.1.74 |
| P4 | One dashboard call | S | ✅ V0.1.74 |
| P5 | Cached map and health | M | ✅ V0.1.74 |
| P6 | Code splitting of the web bundle | S | ✅ V0.1.70 |
| P7 | Query cache with targeted invalidation | M | ✅ V0.1.70, remaining pages V0.2.3 |
| P8 | Writer connection plus read pool; configurable concurrency | M | ✅ V0.1.71 |
| P9 | Parser time budget (memory caps need a subprocess) | M | ✅ V0.1.74 |
| P10 | Readings in a subprocess with time and memory limits (R2-4) | M | V0.2.44 |
| P11 | Map and health recomputed only where a change touched | M | ✅ V0.2.8 |

### Code health and release engineering

| ID | Item | Size | Target |
|---|---|---|---|
| C1 | Split `SharedWebApp.tsx` | M | ✅ V0.1.70 (sections: C13) |
| C2 | Router module and query cache | M | ✅ V0.1.70 |
| C3 | Frontend tests | S–M | ✅ V0.1.70 |
| C4 | Split `apps/server/src/lib.rs` | M | ✅ V0.1.71 |
| C5 | Split `crates/storage/src/lib.rs` | M | ✅ V0.1.71 |
| C6 | Typed errors | M | ✅ V0.1.71 |
| C7 | Generated TypeScript types | S–M | ✅ V0.1.70 |
| C8 | CI | S | ✅ V0.1.66 (rustfmt and clippy blocking since V0.1.71) |
| C9 | One version source, changelog | S | ✅ V0.1.66 |
| C10 | Dependency hygiene (`quick-xml` in ingestion; Vite 5 kept) | S | ✅ V0.1.71 |
| C11 | Split `styles.css` per component | M | ✅ V0.2.5 |
| C12 | Remove the worker stub | S | ✅ V0.1.71 (decision 4) |
| C13 | Workspace page split per section (13a ✅ V0.2.2), remaining pages on the query cache (13b ✅ V0.2.3), link-page rendering tests (13c ✅ V0.2.4) | M | V0.2.2–V0.2.4 |
| C14 | Asynchronous manual re-index endpoints | S | ✅ V0.2.6 |

### Operations and documentation

| ID | Item | Size | Target |
|---|---|---|---|
| O1 | Backup and restore | M | ✅ V0.1.75 |
| O2 | Workspace export (2a) and import (2b) | M | V0.2.40, V0.2.41 |
| O3 | Production guide | S–M | ✅ V0.1.75 |
| O4 | Quotas and disk floor | M | ✅ V0.1.75 |
| O5 | OpenMetrics export | S | ✅ V0.1.75 |
| O6 | Encrypted backups (6a, R2-5) and a second copy after each backup (6b) | S–M | V0.2.42, V0.2.43 (decision 14) |
| O7 | Container image and compose file with the proxy | S–M | V0.2.45 |
| O8 | Audit trail export | S | V0.2.46 |
| D1 | Tracker retired, `PRODUCT.md`, Postman v5 | S | ✅ V0.1.66 |
| R5 | Mindmaps, branch pages, changelog, upgrade notes | M | ✅ V0.1.76 |
| R6 follow-up | Corrections from the clean-machine install | S | V0.2.1 |
| R7–R12 | Release candidate for V0.3.0: freeze and fourth security pass (R7, R8), upgrade from 0.2.0 (R9), performance with remote sources (R10), documentation (R11), clean install (R12) | M | V0.2.52–V0.2.56 |

### Experience

| ID | Item | Size | Target |
|---|---|---|---|
| X1 | Accessibility pass: keyboard paths, focus, labels, contrast | M | V0.2.47 |
| X2 | Narrow screens: workspace pages and the assistant usable at 375 px | M | V0.2.48 |

### Repository indexing track

Design and limits: [technical/repository-sources.md](technical/repository-sources.md).

- [x] **Phase 1:** local repositories, the sync engine, automatic sync when the branch moves, `REPOMEMO_REPO_ROOTS` and the UNC guard.
- [x] **Phase 1b (V0.1.73):** renames with edits, the commit on results and citations.
- [ ] **Phase 2 (V0.2.19–V0.2.22):** credential store and allowed hosts (✅ V0.2.19), then remote repositories over `https` and `ssh`, then clone limits.
- [ ] **Phase 3a (V0.2.23–V0.2.26):** push webhooks, last author and date per file, `CODEOWNERS` hints, a citation at its commit.
- [ ] **Phase 3b (V0.2.27–V0.2.31):** issues and pull requests, through the GitHub and GitLab connector.

### Background jobs track

- [x] Background indexing, embedding and repository-sync queues with restart recovery, retries and failure reasons; incremental re-index; live progress.
- [x] Async manual index endpoints (C14, V0.2.6).
- [ ] A durable job queue claimed by a worker: scale-out direction.

---

## 💭 Later: V0.4.0 and beyond

What V0.3.0 leaves out. The order is not decided.

| Direction | Scope |
|---|---|
| **More connectors** | Linear and Jira, after the GitHub and GitLab connector (F17) proves its model |
| **Assistant depth** | Beyond F5 and the palette: actions that propose memory cards and tasks for a person to confirm |
| **Scale-out team server** | PostgreSQL, object storage, a durable job queue claimed by a worker process, shared rate-limit counters, and Qdrant when vector search needs a service (see [ADR-0004](../docs/decisions/0004-embedded-vector-storage-before-qdrant.md)) |
| **Enterprise / hosted** | Single sign-on (OIDC, SAML), permission-aware retrieval across organizations, audit export, policy controls and redaction, hosted and self-hosted packaging |

---

## Guiding rules (unchanged)

- **Storage first, AI second.** Every AI feature operates on indexed, inspectable evidence and returns citations.
- **Useful without AI.** Storing, browsing, indexing and search must keep working with no provider configured.
- **Cloud is explicit.** No content leaves the server until an admin enables a cloud provider and acknowledges it.
- **Show state.** Storage, indexing progress, provider status and errors are always visible.
