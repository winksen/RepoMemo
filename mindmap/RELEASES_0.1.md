# RepoMemo releases: 0.1 → 0.2

> **Line:** `V0.1.0` (2026-05-31) to `V0.2.0` *Team-ready* (2026-10-14) · **Status:** complete; the `v0.2.0` tag waits for the clean-machine install (R6)
> **Next line:** [Releases 0.2 → 0.3](RELEASES_0.2.md) · **Related:** [Roadmap](ROADMAP.md) · [Changelog](../CHANGELOG.md) · [Functional mindmap](FUNCTIONAL_MINDMAP.md) · [Technical mindmap](TECHNICAL_MINDMAP.md)

Every release of the 0.1 line, one row each, from the desktop skeleton to the first team-ready server. Each row comes from the release's commit (`Update V0.1.N`), the [changelog](../CHANGELOG.md) from V0.1.63 on, and the roadmap's *Delivered* tables before that.

**Type:** ✨ Feature · 🔧 Enhancement · 🐞 Fix · 🔒 Security · 📘 Update (docs, process, dependencies, version). Before V0.1.66 a release often mixed several kinds; the main one comes first.

---

## The tracks

| Releases | Track | What's in it |
|---|---|---|
| V0.1.0–V0.1.11 | Local memory loop | Desktop skeleton, Windows setup, import and store, chunking, full-text search, pasted notes, code symbols, AI providers, embeddings and Ask, image understanding, memory cards, design system |
| V0.1.12–V0.1.32 | Shared team mode | API server, accounts and the web client, uploads, membership, workspace admin and AI, Ask, filters and facets, profiles, tasks and comments, evidence review, notifications, dashboard, organization admin, Word documents, jobs and live events |
| V0.1.33–V0.1.41 | Indexing and evidence | Mindmaps, automatic and incremental indexing, structure-aware code chunks, toasts, retries with failure reasons, vision providers, rich-text notes, folders |
| V0.1.42–V0.1.50 | Documents and AI | Business documents, LibreOffice previews, the workspace assistant and its conversations, semantic search, knowledge map, workspace health |
| V0.1.51–V0.1.57 | Repositories and security | Rotating refresh tokens, local git repositories as living sources, repository page and summary, stack detection, the security hardening bundle |
| V0.1.58–V0.1.62 | Administration | System area, setup wizard, avatars, app administrators, environment folders, admin console, daily-use polish and live progress |
| V0.1.63–V0.1.65 | Onboarding and security | Invitation links, reset links, personal access tokens, security ship-blockers (S1), the security second pass |
| V0.1.66–V0.1.76 | Road to V0.2.0 | CI and one version source, safe defaults, browser hardening, host exposure, web and server foundations, memory you can trust, ownership and repositories 1b, performance, operations, release candidate |
| V0.1.77–V0.2.0 | Planning and release | The Road to V0.3.0 written and revised, then V0.2.0 *Team-ready* |

---

## Every release

| Release | Shipped | Track | Type | Area | What changed |
|---|---|---|---|---|---|
| **V0.1.0** | 2026-05-31 | Local memory loop | ✨ Feature | Desktop skeleton | Tauri 2 + React shell, Rust workspace (`api`, `domain`, `storage` crates), SQLite schema (migration 0001), create and list workspaces, architecture brief, ADR-0001, development commands (phase 1A) |
| **V0.1.1** | 2026-06-11 | Local memory loop | 📘 Update | Windows setup | `docs/WINDOWS_SETUP.md`, `npm run icons` to generate the `icon.ico` Tauri needs on Windows, `Cargo.lock` committed; first full local run |
| **V0.1.2** | 2026-06-11 | Local memory loop | ✨ Feature, 📘 Update | Import and store | `crates/ingestion`: file and folder import with ignore rules, extension and size filters and binary detection; content-addressed blob store; sources and artifacts in SQLite; Artifacts view with a stored-content preview. Docs: phase specs 1B–1H, ADRs 0002–0004, the UI design system (phase 1B) |
| **V0.1.3** | 2026-06-11 | Local memory loop | ✨ Feature | Chunking | `crates/indexer`: Markdown split by heading path, 100-line windows for code and text, chunk metadata (lines, heading, token estimate, hash), indexing jobs, chunk previews on the artifact (phase 1C) |
| **V0.1.4** | 2026-06-19 | Local memory loop | ✨ Feature, 📘 Update | Full-text search | `crates/retrieval`: SQLite FTS5 with safe query building, type, language and source filters, highlighted snippets with line ranges, a Search workbench with evidence preview (phase 1D). Docs: the team collaboration and backend plan |
| **V0.1.5** | 2026-06-20 | Local memory loop | ✨ Feature, 🔧 Enhancement | Pasted notes, interface | Paste text as a note (Markdown, text or code); interface redesign with page blueprints, mockups and the Fjalla One display font |
| **V0.1.6** | 2026-07-19 | Local memory loop | ✨ Feature | Code symbols, AI providers | tree-sitter symbols for TS/TSX/JS, Python and Rust, file outlines and definition-first search (phase 1E); `crates/ai` with Ollama, per-workspace provider settings, a connection test and cited artifact summaries (phase 1F) |
| **V0.1.7** | 2026-07-19 | Local memory loop | ✨ Feature | Cloud AI | OpenRouter as an explicit cloud option, with an API key and an acknowledgement that excerpts leave the device; a cited workspace *Project briefing* (phase 1F) |
| **V0.1.8** | 2026-07-19 | Local memory loop | ✨ Feature | Embeddings and Ask | Local embeddings in `chunk_embeddings` (migration 0002), embedding jobs, hybrid FTS + cosine retrieval, Ask with citations, the "insufficient context" guard and an FTS-only fallback (phase 1G) |
| **V0.1.9** | 2026-07-19 | Local memory loop | ✨ Feature, 🔧 Enhancement | Images, workspace list | Vision providers describe images (`analyze_image`) so they become searchable; workspace list redesigned with readiness steps (sources, indexed, memory) and a theme toggle |
| **V0.1.10** | 2026-08-11 | Local memory loop | ✨ Feature, 📘 Update | Memory cards | Durable memory cards with evidence links, missing evidence shown as stale, search and Markdown export; *Save memory* from summaries and Ask, never automatic (phase 1H). `DESIGN.md` and `PRODUCT.md` added |
| **V0.1.11** | 2026-08-14 | Local memory loop | 🔧 Enhancement | Design | Visual system rewritten in `DESIGN.md` (colour and tone, type scale, do's and don'ts) and the desktop styles rebuilt to match |
| **V0.1.12** | 2026-08-14 | Shared team mode | ✨ Feature | Server foundation | `apps/server` (Axum) with `/health` and `/v1/session`, an `apps/worker` process stub, shared identity types, the shared-mode implementation plan; blueprint refinement styles |
| **V0.1.13** | 2026-08-15 | Shared team mode | ✨ Feature | Accounts, web client | Registration and login (Argon2 + JWT), organizations, workspaces and membership (migration 0003), text evidence, indexing, search and memory over the API, and the first **web client** (`SharedWebApp.tsx`) |
| **V0.1.14** | 2026-08-16 | Shared team mode | ✨ Feature | Uploads | Evidence file uploads up to 10 MiB; artifact and memory card pages; protected web routes |
| **V0.1.15** | 2026-08-16 | Shared team mode | ✨ Feature | Membership | Add, change and remove workspace members, with role rules; first shared UI components (button, dropdown) |
| **V0.1.16** | 2026-08-17 | Shared team mode | ✨ Feature | Workspace admin, AI | Rename and delete workspaces, capabilities per role, per-workspace AI providers, AI overview, activity log (migration 0004) |
| **V0.1.17** | 2026-08-19 | Shared team mode | ✨ Feature | Ask | Cited Ask in shared mode, provider test, memory search; workspace sections (Evidence, Retrieval, Memory, People, Activity, Settings) |
| **V0.1.18** | 2026-08-19 | Shared team mode | ✨ Feature | Evidence filters | `POST …/artifacts/query`: the evidence ledger filters by type, source and indexing status |
| **V0.1.19** | 2026-08-21 | Shared team mode | ✨ Feature | Retrieval facets | Retrieval limited by type, language and source facets, with a result count |
| **V0.1.20** | 2026-08-21 | Shared team mode | ✨ Feature | Profiles, metrics | User profile and password change (migration 0005), "workspace pulse" metrics |
| **V0.1.21** | 2026-08-21 | Shared team mode | ✨ Feature | Collaboration | Task board, evidence comments, assigned tasks, activity calendar (migration 0006) |
| **V0.1.22** | 2026-08-22 | Shared team mode | ✨ Feature | Review, notifications | Evidence lifecycle (verified, outdated, superseded) with history, notifications, @mentions (migrations 0007, 0008) |
| **V0.1.23** | 2026-08-22 | Shared team mode | ✨ Feature | Workflow | Saved searches and task checklists (migration 0009) |
| **V0.1.24** | 2026-08-23 | Shared team mode | 🔧 Enhancement | Web header | Account controls in the header: theme toggle and a notifications button |
| **V0.1.25** | 2026-08-25 | Shared team mode | ✨ Feature | Dashboard | Dashboard page, shared layout with a sidebar, organization switcher |
| **V0.1.26** | 2026-08-25 | Shared team mode | 🔧 Enhancement | Branding, navigation | RepoMemo logos, a workspace top bar with a workspace switcher |
| **V0.1.27** | 2026-08-29 | Shared team mode | ✨ Feature | Organization admin | Organization rename and member management; access flows to every workspace of the organization |
| **V0.1.28** | 2026-09-01 | Shared team mode | 🔧 Enhancement | Page headings | Dashboard, profile, notifications and workspace headings simplified (eyebrows and duplicate buttons removed) |
| **V0.1.29** | 2026-09-02 | Shared team mode | ✨ Feature | Evidence views | Grid and list views of the evidence ledger, readable type labels |
| **V0.1.30** | 2026-09-02 | Shared team mode | ✨ Feature | Word documents | `.doc` and `.docx` text extraction and indexing, a Documents view in the desktop app |
| **V0.1.31** | 2026-09-14 | Shared team mode | ✨ Feature, 🔧 Enhancement | Word documents | Documents section with preview in the web client; server code formatted, more API tests |
| **V0.1.32** | 2026-09-29 | Shared team mode | ✨ Feature | Jobs, live events | Job kinds and cooperative cancel (migration 0010), job listing, an **SSE event stream** for job and activity events |
| **V0.1.33** | 2026-09-29 | Indexing and evidence | 📘 Update | Mindmaps | `mindmap/` created: functional and technical mindmaps, the roadmap (replacing `docs/ROADMAP.md`); README rewritten |
| **V0.1.34** | 2026-09-29 | Indexing and evidence | 🔧 Enhancement | Branding | Simplified logo |
| **V0.1.35** | 2026-09-29 | Indexing and evidence | ✨ Feature | Automatic indexing | Notes and uploads indexed in the background after saving, with restart recovery and an *Indexing…* status; stored passages and a single re-index for owners and admins (`GET /v1/artifacts/{a}/chunks`) |
| **V0.1.36** | 2026-10-01 | Indexing and evidence | 🔧 Enhancement | Incremental indexing | Structure-aware code chunks (declarations, scope in the heading); unchanged passages keep their identity, embeddings and links; `INDEXER_VERSION` refreshes older indexes in the background (migration 0011) |
| **V0.1.37** | 2026-10-02 | Indexing and evidence | 🔧 Enhancement | Theme, sign-in | Theme remembered in a cookie, a theme toggle on the sign-in pages, the full logo in the sidebar |
| **V0.1.38** | 2026-10-02 | Indexing and evidence | 🔧 Enhancement, 📘 Update | Toasts | Toasts for errors and confirmations; design rules "toasts, never inline banners" and "no tinted neutrals" |
| **V0.1.39** | 2026-10-02 | Indexing and evidence | 🔧 Enhancement | Evidence filters | File categories in the ledger (Markdown, text, code, Word, image) with their icons |
| **V0.1.40** | 2026-10-02 | Indexing and evidence | ✨ Feature | Retries, notes, folders | Indexing retries (30 s, 2 min, 10 min) with stored failure reasons (migration 0012); separate text and vision providers; TipTap rich-text notes stored as Markdown; nested folders up to 5 levels (migration 0013) |
| **V0.1.41** | 2026-10-02 | Indexing and evidence | ✨ Feature | Folders | Rename and delete folders, move evidence between folders; action menu and dialog components |
| **V0.1.42** | 2026-10-02 | Documents and AI | ✨ Feature | Business documents | Excel, PowerPoint, PDF, OneNote and Outlook text extraction and previews (`documents.rs`); signed *Open in Office* links (`/v1/shared-files/…`) |
| **V0.1.43** | 2026-10-02 | Documents and AI | ✨ Feature | Rendered previews | Layout-accurate PDF previews through LibreOffice when it is installed (`conversion.rs`, rendered-preview routes) |
| **V0.1.44** | 2026-10-02 | Documents and AI | ✨ Feature | Assistant | The workspace assistant: a closed capability router (find files, search, summarize, ask, overview) with keyword and model routing |
| **V0.1.45** | 2026-10-02 | Documents and AI | ✨ Feature | Assistant | Stored per-user conversations, listed, renamed and deleted (migration 0014) |
| **V0.1.46** | 2026-10-03 | Documents and AI | ✨ Feature | Semantic search | Embedding providers (Ollama, OpenRouter), a background embedding queue, hybrid search with reciprocal rank fusion in Retrieval, Ask and the assistant, an LLM rerank in Ask |
| **V0.1.47** | 2026-10-03 | Documents and AI | 🔧 Enhancement | Layout | Dashboard, profile, metrics and assistant pages use the full width |
| **V0.1.48** | 2026-10-03 | Documents and AI | ✨ Feature | Knowledge map | Indexing pipeline, coverage per kind, a relation graph of similar files and memory-card citations |
| **V0.1.49** | 2026-10-04 | Documents and AI | 🔧 Enhancement | Knowledge map | Zoom, pan and drag; selecting a dot glides to it; reduced-motion support |
| **V0.1.50** | 2026-10-06 | Documents and AI | ✨ Feature | Workspace health | Deterministic checks (older versions, duplicates, removed symbols still mentioned, outdated references, failed indexing, unconnected files) with actions and recorded outcomes (migration 0015) |
| **V0.1.51** | 2026-10-06 | Repositories and security | 🔒 Security | Sessions | Rotating refresh tokens (migration 0016), `/v1/auth/refresh` and `/v1/auth/logout`; the web client renews its session |
| **V0.1.52** | 2026-10-06 | Repositories and security | ✨ Feature | Repository sync | Local git repositories as living sources (`crates/git`, migration 0017): the branch is polled and synced when it moves; the repository-sources design page |
| **V0.1.53** | 2026-10-06 | Repositories and security | 🔧 Enhancement | Repository links | Links managed in Settings › Repositories, with an access check before linking (branch, commit, file estimate), a branch choice and include/exclude patterns; image preview and pagination components |
| **V0.1.54** | 2026-10-06 | Repositories and security | ✨ Feature | Repository page | Each repository is one evidence item with its own page: sync state, file browser, a content overview without AI, an on-request cited AI summary |
| **V0.1.55** | 2026-10-06 | Repositories and security | 🔧 Enhancement | Map, health | A repository is one node in the knowledge map and one item in the duplicate and unconnected checks |
| **V0.1.56** | 2026-10-06 | Repositories and security | ✨ Feature | Stack detection | The technologies of a repository detected, with a language chart |
| **V0.1.57** | 2026-10-08 | Repositories and security | 🔒 Security, ✨ Feature | Security hardening | Session versions and "sign out everywhere", encrypted provider keys (`secrets.rs`), sign-in throttling and lockout, workspace AI policy and per-user quotas, provider-URL and repository-root guards, security headers, typed 404s, blob garbage collection and hourly maintenance (migration 0018); server settings documented |
| **V0.1.58** | 2026-10-08 | Administration | ✨ Feature | System area | Overview, usage, users, run-time settings, logs (live and daily files, per-category levels), jobs and maintenance, audit trail (migration 0019) |
| **V0.1.59** | 2026-10-09 | Administration | ✨ Feature, 📘 Update | Setup, avatars | First-run setup wizard with a one-time code, profile pictures, app administrators without wildcard access (migrations 0020–0022); `CLAUDE.md` |
| **V0.1.60** | 2026-10-09 | Administration | ✨ Feature | Environments | Choose, create, verify and detach data environments under `workspace-data/` from the browser |
| **V0.1.61** | 2026-10-09 | Administration | ✨ Feature | Admin console | System › Console and the `repomemo-console` terminal client, with dry runs and audited changes |
| **V0.1.62** | 2026-10-10 | Administration | ✨ Feature, 🔧 Enhancement | Polish, live progress | AI output as Markdown with numbered citations, safe search highlights, several citations per card, *Save as memory*, several files or whole folders per upload, **live progress** over a fetch-based event stream, `GET /v1/system/events`, the assistant docked beside every section, the last environment remembered |
| **V0.1.63** | 2026-10-13 | Onboarding and security | ✨ Feature, 📘 Update | Invitations, tokens | Invitation links, password reset links, personal access tokens `rmp_…` (migration 0023); the security review and the enhancement analysis written |
| **V0.1.64** | 2026-10-13 | Onboarding and security | 🔒 Security | Ship-blockers (S1) | **H1** one workspace grants that workspace only; **H4** admin emails promote only accounts that exist at startup; **L2** token limits on the handlers; **L3** link secrets in the URL fragment; **M7** only `VITE_` and `TAURI_ENV_` variables reach the bundle |
| **V0.1.65** | 2026-10-13 | Onboarding and security | 🔒 Security | Second pass | **N1 (high)** stored XSS through image previews closed (content types from the file name, also I3); **N2** settings need a session; **N3** owners keep their role; **N4** viewers no longer edit old comments or tasks; **N5** a used reset link revokes tokens; **N6** synchronous re-index for admins only |
| **V0.1.66** | 2026-10-13 | Road to V0.2.0 | 📘 Update, ✨ Feature | Checkable releases | CI on every push (versions, Rust tests on Linux and Windows, typecheck, `cargo deny`, `npm audit`), `npm run version:set`, performance timings in a *Performance* log category, `CHANGELOG.md`; the implementation tracker retired |
| **V0.1.67** | 2026-10-13 | Road to V0.2.0 | 🔒 Security | Safe defaults (S2) | Released in two commits. Registration closed by default (**M1**), transport check at startup (**M9**), proxy trust (**M3**), the same `409 invitation_required` for every unknown address (**M4**), app administrators read and system administrators change (**M8**), a `system` token scope (migration 0024), held sessions get past lockouts (**L10**), reset-link notifications (**L4**), weak passwords refused (**L7**) |
| **V0.1.68** | 2026-10-14 | Road to V0.2.0 | 🔒 Security | Browser hardening (S4) | Refresh token in an `HttpOnly; SameSite=Strict` cookie and a content security policy (**M2**), a strict desktop policy (**M6**), `SafeMarkdown` with no remote images and visible link hosts (**M5**, **L9**), link secrets leave the address bar (**L3**) |
| **V0.1.69** | 2026-10-14 | Road to V0.2.0 | 🔒 Security | Host exposure (S3) | AI providers only on allowed hosts, checked at connect time (**H2**); repository links off until `REPOMEMO_REPO_ROOTS` is set (**H3**); git isolated (**L8**); LibreOffice without the server's secrets, optional sandbox wrapper (**M10**) |
| **V0.1.70** | 2026-10-14 | Road to V0.2.0 | 🔧 Enhancement | Web foundation | `SharedWebApp.tsx` split by page, a typed router, a TanStack Query cache with targeted invalidation, lazily loaded panels, types generated from Rust, 27 web tests; the desktop app labelled *Local edition* |
| **V0.1.71** | 2026-10-14 | Road to V0.2.0 | 🔧 Enhancement, 🔒 Security | Server foundation | Server split into 13 modules and storage into 9, typed errors, one writer connection and a read pool, request and header timeouts with a stream cap (**N7**), rustfmt and clippy blocking in CI, the worker stub removed, `quick-xml` 0.41 and patched dependencies |
| **V0.1.72** | 2026-10-14 | Road to V0.2.0 | ✨ Feature | Memory and search you can trust | Cards show *Needs review* when their evidence moved, a *Stale memory* health check, editable citations; search by evidence state, folder and date with verified evidence ranked higher; code symbols on the web and in the assistant; live map and health |
| **V0.1.73** | 2026-10-14 | Road to V0.2.0 | ✨ Feature, 🔒 Security | Evidence, ownership, repositories 1b | Ownership transfer; folders moved; bulk actions on files; a new version of a file in place; repository renames with edits; the commit on results and citations; console `users reset-link`, `tokens list`, `logs --follow`; owners cannot leave their organization (**N8**) |
| **V0.1.74** | 2026-10-14 | Road to V0.2.0 | 🔧 Enhancement, 🔒 Security | Performance and robustness | In-memory vector index, file query and figures in SQL, a one-call dashboard, cached map and health; heavy requests rate limited and parsers time-boxed (**M11**), a daily background AI allowance (**M12**), 60-second single-client file links (**L1**), owner-only `secret.key` (**L5**) |
| **V0.1.75** | 2026-10-14 | Road to V0.2.0 | ✨ Feature | Operations | Backup and restore into a new environment, scheduled backups, quotas and a disk floor, OpenMetrics at `GET /v1/system/metrics`, the production guide |
| **V0.1.76** | 2026-10-14 | Road to V0.2.0 | 📘 Update, 🔒 Security | Release candidate | Feature freeze; third security pass with no high or medium finding (R2-1 and R2-2 fixed); upgrade test from V0.1.40, V0.1.52 and V0.1.65 schemas; release-build timings at 50,000 passages; `UPGRADING.md`, API reference, smoke test |
| **V0.1.77** | 2026-10-14 | Planning and release | 📘 Update | Roadmap | Road to V0.3.0 *Connected* proposed (documentation only; the code stays 0.1.76) |
| **V0.1.78** | 2026-10-14 | Planning and release | 📘 Update | Roadmap | Road to V0.3.0 revised into 56 single-purpose releases in tracks A–J, decisions 7–14 (documentation only) |
| **V0.2.0** | 2026-10-14 | Planning and release | 📘 Update | Team-ready | The version set to `0.2.0` everywhere and the changelog entry mapping each promise to its release; the `v0.2.0` tag waits for the clean-machine install (R6) |

---

## Notes on numbering

- **V0.1.6 brought code symbols.** The roadmap's *Delivered* table lists phase 1E under V0.1.3, but the tree-sitter work is in the V0.1.6 commit; this page follows the commits.
- **V0.1.67 is two commits** with the same title: the first closed registration and added the transport and proxy checks, the second finished the bundle and set the version.
- **V0.1.77 and V0.1.78 are documentation commits.** The code reports 0.1.76 until V0.2.0.
- **Reported versions before V0.1.66** lagged behind the commits (0.1.0 until V0.1.58, then 0.1.58 and 0.1.60). One version source since V0.1.66 fixed that.
