# Polish review before 1.0: 2026-11-25

> **Scope:** `main` at V0.9.69: the server (`apps/server`, `crates/*`), the web client (`apps/desktop/src`), the desktop shell (`apps/desktop/src-tauri`), CI, the release indexes, the changelog and the user guide.
> **Method:** the code and documents were read and searched; nothing was run, built or tested. Bundle sizes come from the last `dist/` on disk (built 2026-10-27), so they are old and only show proportions. Route coverage was found by comparing the routes in `apps/server/src/lib.rs` with the paths `apps/desktop/src/lib/sharedApi.ts` calls, then checked by hand against the changelog's *Not in this release* lines.
> **Feeds:** the [0.11 polish plan](../plans/2026-11-25-0.11-polish.md) and the rows V0.11.0 to V0.11.68 in the [release index](../RELEASES_0.11.md).

---

## 1. Summary

1. **The server is ahead of the product people can use.** About sixty of the server's routes have no screen. The whole 0.7 project line (projects, briefs, milestones, project updates, the paged task list, saved views, the board, labels, workflow states, subtasks, dependencies, discussion, references, bulk changes, recurring tasks, CSV) and most of the 0.8 line's new surfaces (outbound webhooks, follow-ups, chat destinations, folder and Confluence imports, AI usage, publications) are reachable only through the API. Each changelog entry says so honestly (*"Not in this release: the web screens"*), but the screens never followed, the [user guide](../../docs/USER_GUIDE.md) describes the board, views, milestones and project updates as if they were on screen, and the 0.7 release rows are ✅ with *Done when* lines that describe screens. This is the largest gap between RepoMemo and a 1.0.
2. **Security is strong for a self-hosted product of this age.** Eleven reviews, an `HttpOnly` `SameSite=Strict` refresh cookie with the `__Secure-` prefix, a strict CSP, argon2, lockouts, optional two-step sign-in, an SSRF guard that checks every resolved address for AI providers, a pinned resolver for OIDC, sandboxed downloads, signed desktop updates, encrypted backups and stored secrets. What is left is small and closable: a DNS-rebinding window on webhook and chat sends, no way to *require* two-step sign-in for administrators, no common-password check, the access token in `sessionStorage`, per-account leftovers in `localStorage`, and the three open CAIRN findings (CR-4, CR-5, CR-7) plus the CR-3 residual.
3. **Validation has fallen behind building.** 125 release rows are 🧪 *built, not validated*: 48 of 0.8, 70 of 0.9 and 7 earlier. Seventy CAIRN rows were built in four days. Nothing in CI drives the web client against a real server, so a screen that calls a route wrongly, or a route nobody calls, is not caught.
4. **The code is clean and the guard rails are real.** 54 `unwrap`/`expect` in production Rust (most in the evaluation fixtures), SQL built only from constants, clippy with `-D warnings`, about 950 Rust tests and 125 web test files, checks for versions, migrations, styles, accessibility, templates and docs. The weak spots are size (a 2,424-line API client, Rust modules over 2,000 lines, one 320-route function) and leftovers of earlier designs (fetches in effects, hand-written types beside generated ones, stylesheets named after design eras).
5. **Speed is good on the server and wasteful in a few client paths.** WAL with a reader pool, an in-memory vector index, batched embeddings, load shedding and compression are in place. The client fans out one request per workspace for CAIRN's chat list, polls notifications every minute although a live stream exists, opens a stream per tab, keeps the CAIRN page and workspace sections in the first download, and waits for a round trip on every toggle.

---

## 2. Security

| # | Severity | Finding | Where | Row |
|---|---|---|---|---|
| S1 | Medium | Webhook and chat sends check the resolved address, then let the HTTP client **resolve the name again** when it connects. The code says so: *"this narrows the window for a rebinding trick and does not close it"*. OIDC already does it right by pinning the checked addresses (`resolve_to_addrs`), and the AI client filters every address in its resolver | `apps/server/src/outbound.rs:192`, `apps/server/src/chat.rs:81`; compare `apps/server/src/identity.rs:635`, `crates/ai/src/lib.rs:314` | V0.11.32 |
| S2 | Medium | Two-step sign-in can be turned on by each person but **cannot be required**, not even for system administrators, whose accounts can change every setting, read the audit and restore backups | `apps/server/src/mfa.rs` | V0.11.33 |
| S3 | Low | Passwords need 12 characters and 5 distinct characters, and nothing else: `password1234`, `qwertyuiop12` and the organization's name pass | `apps/server/src/auth.rs:577` | V0.11.34 |
| S4 | Low | The access token is kept in `sessionStorage`, readable by any script that runs in the page. The CSP makes that unlikely, but a token in memory, renewed from the cookie on reload, removes the prize | `apps/desktop/src/lib/sharedApi.ts:179` | V0.11.35 |
| S5 | Low | The admin console's history (`repomemo.system.console-history`: emails, user changes, backup names) and the last chat per workspace are in `localStorage`, not keyed by account and not removed at sign-out. The next person on a shared browser reads them | `apps/desktop/src/components/SystemConsole.tsx:13`, `apps/desktop/src/components/AssistantPanel.tsx:83` | V0.11.36 |
| S6 | Low | A person can end *all* sessions but cannot see which devices are signed in or end one | `apps/desktop/src/web/Profile.tsx:145` | V0.11.37 |
| S7 | Low | The audit log can be edited by anyone with the database file and nothing would show it; there is no chain or verification | `crates/storage/src/system.rs` | V0.11.38 |
| S8 | Low | No `Permissions-Policy`, `Cross-Origin-Opener-Policy` or `Cross-Origin-Resource-Policy` on the API or the web files | `apps/server/src/security.rs:509`, `apps/server/src/web_files.rs:103` | V0.11.39 |
| S9 | Medium, carried | CR-4: memory upkeep lists the words of other people's unanswered questions to every writer | [CAIRN review](../security/2026-11-25-cairn-review.md) | V0.11.40 |
| S10 | Low, carried | CR-5: sharing a chat does not say that replies may restate the owner's personal memory | same | V0.11.41 |
| S11 | Low, carried | CR-7: event routines have a debounce but no daily cap | same | V0.11.42 |
| S12 | Low, carried | CR-3 residual: a colleague's file can still steer the model into recording a correction that reaches later runs | same | V0.11.43 |
| S13 | Info | Errors carry no request id, so a person who reports *"something went wrong"* cannot be matched with a log line | `apps/server/src/errors.rs` | V0.11.6 |
| S14 | Info | Every review so far was done by reading code. No penetration test, fuzzing of the document parsers or external audit has been done | the [security folder](../security/) | 2.0 milestone 10 |

Reviewed with no finding: SQL (73 dynamic strings, all interpolating constants such as a column name or a fixed `SELECT`), archive import (entries are bound by the manifest and checked by hash), stored-file downloads (`attachment` and `sandbox`), the document readers' limits, the CORS list, token scopes and expiry, refresh-token rotation, the Tauri capabilities (`core:default` and one window permission) and its CSP.

---

## 3. Usability and experience

### 3.1 Screens owed by earlier lines

Every item below has working, tested routes and no screen. The *changelog* column is the release that said *"Not in this release: the web screen"*.

| Feature | Changelog | Routes | Row |
|---|---|---|---|
| Paged task list with filters, saved views | V0.7.12 | `/task-list`, `/task-views` | V0.11.7, V0.11.8 |
| Task page with its key, edit conflicts | V0.7.3, V0.7.6 | `/tasks/{id}` with `expected_version` | V0.11.9 |
| Task discussion and watching | V0.7.4 | `/tasks/{id}/comments`, `/watch` | V0.11.10 |
| Several references per task | V0.7.5 | `/tasks/{id}/references` | V0.11.11 |
| Subtasks and dependencies | V0.7.8, V0.7.9 | `/children`, `/dependencies` | V0.11.12 |
| Labels | V0.7.10 | `/task-labels`, `/merge` | V0.11.13 |
| Workflow states | V0.7.11 | `/workflow-states` | V0.11.14 |
| Task board | V0.7.13 | `/task-board`, `/tasks/{id}/move` | V0.11.15 |
| Bulk changes with preview | V0.7.14 | `/tasks/bulk` | V0.11.16 |
| Projects, brief, milestones, updates | V0.7.1, V0.7.2, V0.7.7, V0.7.17 | `/projects`, `/milestones`, `/project-updates` | V0.11.17 to V0.11.20 |
| Recurring tasks | V0.7.16 | `/recurring-tasks` | V0.11.21 |
| Task CSV import and export | V0.7.18 | `/tasks/import`, `/tasks/export.csv` | V0.11.22 |
| Outbound webhooks | V0.8.4 | `/outbound-endpoints` | V0.11.23 |
| Tracker follow-ups | V0.8.8 | `/artifacts/{id}/follow-up` | V0.11.24 |
| Chat destinations | V0.8.9 | `/chat-destinations` | V0.11.25 |
| Folder and Confluence imports | V0.8.11, V0.8.12 | `/imports/directory`, `/imports/confluence` | V0.11.26 |
| AI usage and limits | V0.8.13 | `/ai-usage`, `/ai-limit` | V0.11.27 |
| Publications | V0.8.16 to V0.8.18 | `/publication`, `/publications` | V0.11.28 |
| CAIRN inbox badge and notice | V0.9.67 | the inbox chat (`kind = inbox`), `/v1/mcp` | V0.11.29 |
| *Wrong answer* and *Review with CAIRN* buttons | V0.9.53, V0.9.65 | the chat routes | V0.11.30 |
| Daily and monthly routine schedules | V0.9.63 | `/agent/routines` | V0.11.31 |

The Tasks tab today (`apps/desktop/src/web/workspace/TasksSection.tsx`) creates, edits and deletes tasks with a checklist through the pre-0.7 `/v1/workspaces/{id}/tasks` list, which returns every task at once.

### 3.2 Other findings

| # | Impact | Finding | Where | Row |
|---|---|---|---|---|
| U1 | High | The user guide describes the board, views, bulk changes, milestones and project updates as usable; the 0.7 rows are ✅ although their *Done when* describes screens. The roadmap's summary is right (*built API-first, no web screens yet*) | `docs/USER_GUIDE.md:216`, `mindmap/RELEASES_0.7.md` | V0.11.1 |
| U2 | Medium | There is no error boundary anywhere: one component that throws while rendering blanks the whole app | `apps/desktop/src` (no `componentDidCatch`) | V0.11.4 |
| U3 | Medium | Nothing can be undone from where it was done. Trash is restorable, but only from Settings › Trash; toasts carry text only | `apps/desktop/src/components/ui/toast.tsx` | V0.11.5 |
| U4 | Medium | CAIRN's thread scrolls to the end on every new turn, even when the reader has scrolled up to read an earlier answer | `apps/desktop/src/components/AssistantPanel.tsx:195` | V0.11.54 |
| U5 | Low | No drag and drop: files are added only through a picker | `apps/desktop/src` (no `onDrop`) | V0.11.53 |
| U6 | Low | CAIRN's answers have no Copy or Export, although the Ask page exports an answer as Markdown | `apps/desktop/src/web/workspace/RetrievalSection.tsx:181` | V0.11.55 |
| U7 | Low | Once the theme is toggled, the system's light or dark setting is never followed again | `apps/desktop/src/components/SharedLayout.tsx:32` | V0.11.56 |
| U8 | Low | Shortcuts exist (palette, send, board moves to come) but nothing lists them | n/a | V0.11.57 |
| U9 | Low | The whole thread is one polite live region, so a streamed answer can be announced piece by piece | `apps/desktop/src/components/AssistantPanel.tsx:558` | V0.11.58 |
| U10 | Low | A new server is empty: the first value needs the person's own files | `apps/desktop/src/components/OnboardingChecklist.tsx` | V0.11.59 |
| U11 | Info | The interface is English only, while CAIRN answers in the person's language | the whole client | 2.0 milestone 6 |

---

## 4. Speed

| # | Finding | Where | Row |
|---|---|---|---|
| P1 | CAIRN's chat list sends **one request per workspace** with `staleTime: 0`, every time the page opens: 40 workspaces, 40 requests | `apps/desktop/src/web/cairn.ts:46` | V0.11.44 |
| P2 | Notifications are polled every 60 s and the review queue every 60 s, per tab, although the server already streams events; a mention can take a minute to show | `apps/desktop/src/components/NotificationsMenu.tsx:23`, `apps/desktop/src/web/workspace/data.ts:34` | V0.11.45 |
| P3 | Each tab opens its own event streams; with several tabs a browser on HTTP/1.1 reaches its six connections per host and requests queue | `apps/desktop/src/lib/liveEvents.ts` | V0.11.46 |
| P4 | Only the map, health, review, viewer, editor, repositories and System are split out. The CAIRN page, the workspace sections, organization, profile, dashboard and the Markdown renderer are in the first download (667 KB, about 195 KB gzip, in the last build on disk) | `apps/desktop/src/web/lazy.tsx` | V0.11.47 |
| P5 | Nothing checks the size of the first download, so it only grows | CI | V0.11.48 |
| P6 | The web host reads each file from disk on every request, with no `ETag`, no `304` and no precompressed copy | `apps/server/src/web_files.rs:80` | V0.11.49 |
| P7 | No change is shown before the server answers (no optimistic update anywhere): ticking a checklist item or marking a notice read waits a round trip | client | V0.11.50 |
| P8 | Pages fetch only when opened, never on hover or focus of their link | client | V0.11.51 |
| P9 | The vector index holds `f32` values: a 1,024-dimension workspace of 200,000 passages needs about 800 MB, so large workspaces fall back to scanning the database | `crates/storage/src/vectors.rs` | V0.11.52 |
| P10 | The Tasks tab downloads every task of the workspace (see 3.1) | `TasksSection.tsx` | V0.11.7 |
| P11 | A chat destination builds a new HTTP client for each message | `apps/server/src/chat.rs:86` | V0.11.32 |

Already good: WAL with `synchronous = NORMAL`, one writer and a configurable reader pool with cache and mmap sizes, the vector index updated in place, batched embedding calls, a request timeout, load shedding per server and per account, compressed JSON, immutable caching of hashed assets.

---

## 5. Code quality

| # | Finding | Where | Row |
|---|---|---|---|
| Q1 | The API client is one file of 2,424 lines and 286 functions; about 25 screens are about to add to it | `apps/desktop/src/lib/sharedApi.ts` | V0.11.2 |
| Q2 | Nothing stops a route from shipping without a screen (the cause of 3.1) | CI | V0.11.3 |
| Q3 | Some pages still fetch in effects instead of the query cache: the app shell (14 effects), Evidence (8), the palette (6), Ask (5), the workspace page (4) and several settings panels | `apps/desktop/src/SharedWebApp.tsx` and others | V0.11.60 |
| Q4 | `types.ts` holds 83 hand-written types beside 238 generated from the domain crate | `apps/desktop/src/types.ts` | V0.11.61 |
| Q5 | All ~320 routes are declared in one function | `apps/server/src/lib.rs:653` | V0.11.62 |
| Q6 | Modules over 1,900 lines: `system.rs` 2,623, `crates/api/src/lib.rs` 2,369, `agent/run.rs` 1,996, `crates/storage/src/lib.rs` 1,964 | | V0.11.63 |
| Q7 | Stylesheets are named after design eras (*neutral research canvas*, *quiet evidence desk*, *blueprint*); 26 of 428 `shared-*` classes are not referenced by any component (a few may be built from variables) | `apps/desktop/src/styles` | V0.11.64 |
| Q8 | No browser test drives the client against a real server; the web tests mock the API | CI | V0.11.65 |
| Q9 | 125 rows are 🧪: the walkthroughs and live evaluations they ask for are owed | release indexes | V0.9.119, V0.10.x, V0.11.67 |

---

## 6. Ratings

Marks out of 10, for the code as read on 2026-11-25. They judge what a team gets from the product today, not the plans.

| Area | Mark | Why |
|---|---|---|
| Security | **8.5** | Secure defaults, many reviews, careful handling of secrets, outbound calls and AI boundaries. Loses points for S1 to S3, the carried CAIRN findings, and no outside test |
| Server and storage code | **8.5** | Clean Rust, small error surface, strong tests and CI gates, honest migrations and archive compatibility. Large modules |
| Web client code | **7** | Typed, tested, one design system and a query cache. A monolithic API client, leftover effect fetches, no error boundary, era-named styles |
| Performance | **7.5** | The server is tuned and measured; the client wastes requests and ships a large first download |
| Usability of what is on screen | **7.5** | The core loop (workspaces, evidence, search, Ask, memory, review, CAIRN) has been polished many times and is consistent, keyboard- and screen-reader-aware, and works down to phone width |
| Usability of the product as promised | **5.5** | A team that reads the guide will look for projects, a board, milestones, publications and webhooks and not find them |
| Operability | **8.5** | First-run setup, System area, console, backups with restore drills, logs, update feed, troubleshooting page checked in CI |
| Documentation | **7** | Thorough and current for operators and developers; the user guide overstates the screens |
| **Overall maturity** | **7** | **Late beta.** Feature breadth is past what a 1.0 needs; validation and the last mile of the interface are behind |

**Where it stands:** a self-hosted knowledge product with an agent, beyond most open-source peers on governance (review states, citations, quarantine, provider boundaries), and an MVP-grade project tool that exists only as an API. A team could adopt it today for evidence, search, memory and CAIRN; not yet for planning work.

---

## 7. Milestones for 2.0

1.0 is the product the 0.10 pilot and the 0.11 polish prove. 2.0 is what changes who can use it and at what size. In order of the value they unlock:

1. **Scale-out server.** PostgreSQL as an option beside SQLite, object storage for files, a durable job queue, rate limits shared between nodes, and an approximate vector index (HNSW or a vector service) so a workspace of millions of passages answers in milliseconds. Trigger: a measured 0.10 capacity ceiling or an availability requirement ([ADR 0004](../../docs/decisions/0004-embedded-vector-storage-before-qdrant.md)).
2. **Enterprise identity.** SAML, SCIM provisioning and deprovisioning, passkeys (WebAuthn), several identity providers, and mirroring of a source's per-person permissions so restricted sources can be connected instead of refused.
3. **Collaborative editing.** Several people editing a note or a memory card at once (CRDT), presence, and comments anchored in the text; today edits are checked against a version and conflicts are shown.
4. **Live sources.** Google Drive, SharePoint, Confluence and Slack kept in sync, with deletion and permission semantics of the 0.8 contract, instead of one-off imports.
5. **CAIRN beyond the workspace.** Outside MCP tools under an administrator policy, web reading, IDE extensions, routines that apply changes under an approval policy, and a model support matrix gated by the evaluation set.
6. **Languages.** An interface in French and Arabic at least (right-to-left included), translated templates and guides; CAIRN already answers in the person's language.
7. **Mobile.** An installable web app with offline reading, push notifications and a capture flow (photo, voice note, share sheet), then native apps if the web app falls short.
8. **Extensibility.** A versioned public API (v2) with generated SDKs, connector and template packages that a team can install, and outbound events covering every entity.
9. **Advanced planning.** Portfolios, sprints, workload and time when teams outgrow the bounded project model, built on the 0.11 screens.
10. **Assurance.** An outside penetration test, fuzzing of the document readers, a published threat model, retention policies with legal hold, and a hosted offer as its own product plan.
