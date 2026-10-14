# RepoMemo releases: 0.2 → 0.3

> **Line:** `V0.2.1` to `V0.3.0` *Connected* · **Current:** `V0.2.11` (2026-10-14) · **Next:** `V0.2.12`
> **Previous line:** [Releases 0.1 → 0.2](RELEASES_0.1.md) · **Related:** [Roadmap, Road to V0.3.0](ROADMAP.md#-road-to-v030) · [Changelog](../CHANGELOG.md) · [Functional mindmap](FUNCTIONAL_MINDMAP.md) · [Technical mindmap](TECHNICAL_MINDMAP.md)

Every release of the 0.2 line, shipped and planned, one row each. The plan comes from the [Road to V0.3.0](ROADMAP.md#-road-to-v030): **one change per release**, each shippable alone, with the security rules for a new outside connection in the release that opens it. Planned rows are proposals until decisions 7–14 in the roadmap are agreed.

**Type:** ✨ Feature · 🔧 Enhancement · 🐞 Fix · 🔒 Security · 📘 Update (docs, process, dependencies, release). Releases whose purpose is a security rule or a review finding are marked 🔒 here; the roadmap's plan table files them under the kind of work instead.
**Status:** ✅ shipped (with its date) · 🔄 in progress · 🔜 planned

When a release ships, set its status to ✅ with the date and make *What changes* say what was built.

---

## The tracks

| Releases | Track | What's in it |
|---|---|---|
| V0.2.1–V0.2.8 | A. After 0.2.0 | Clean-install fixes, workspace page split, remaining pages on the cache, link-page tests, `styles.css` split, background re-index, console workspace commands, incremental map and health |
| V0.2.9–V0.2.15 | B. Mail | SMTP and outbox, invitations by mail, forgot password, mentions and assignments by mail, email verification, notification preferences, daily digest |
| V0.2.16–V0.2.18 | C. Accounts | Deactivate, anonymize, open registration without revealing who has an account |
| V0.2.19–V0.2.26 | D. Remote repositories | Credential store and allowed hosts, `https`, `ssh`, clone limits, push webhooks, last author, `CODEOWNERS`, view a citation at its commit |
| V0.2.27–V0.2.31 | E. Issues and pull requests | GitHub issues, GitHub pull requests, GitLab, links to files and symbols, webhooks and rate limits |
| V0.2.32–V0.2.39 | F. Search, assistant, review | Assistant memory, cross-workspace search, command palette, Go and Java, C# and PHP, review queue, file version history, `@mention` suggestions |
| V0.2.40–V0.2.46 | G. Portability and operations | Export, import, encrypted backups, a second backup copy, readings in a subprocess, container image, audit export |
| V0.2.47–V0.2.48 | H. Experience | Accessibility pass, narrow screens |
| V0.2.49–V0.2.51 | I. One client | Desktop as a server client, export to a server, local edition retired |
| V0.2.52–V0.3.0 | J. Release candidate | Freeze and fourth security pass, upgrade test, performance run, documentation, clean install, V0.3.0 *Connected* |

---

## Every release

| Release | Status | Track | Type | Area | What changes | Done when |
|---|---|---|---|---|---|---|
| **V0.2.1** | ✅ 2026-10-14 | A. After 0.2.0 | 🐞 Fix | Operations | The production guide and the server checks corrected against the code: *Public address* and *Automatic backups* checks, a warning for HTTPS without `REPOMEMO_TRUST_PROXY`, `REPOMEMO_BACKUP_INTERVAL_HOURS` in the guide (R6 follow-up) | A second clean install follows the guide with no workaround (the maintainer's, with R6) |
| **V0.2.2** | ✅ 2026-10-14 | A. After 0.2.0 | 🔧 Enhancement | Workspace page | `WorkspaceDetail.tsx` (1,089 lines) became a 320-line shell plus seven section components in `web/workspace/` (largest 371 lines), with `data.ts` hooks and `sectionState`; 30 web tests (C13a) | No section component over 500 lines; behaviour unchanged, tests green |
| **V0.2.3** | ✅ 2026-10-14 | A. After 0.2.0 | 🔧 Enhancement | Web data | The dashboard, profile, notifications, evidence detail and memory card pages read through the query cache; live events mark them stale, and the memory card page follows the workspace stream; `staleTime` and `accountKeys`; 32 web tests (C13b) | No page loads its own data outside the cache; live events refresh them |
| **V0.2.4** | ✅ 2026-10-14 | A. After 0.2.0 | 🔧 Enhancement | Web tests | Rendering tests of the invitation page (every link state, with and without an account, signed in as someone else, a failed join) and the password reset page; 51 web tests, no product code changed (C13c) | Each state of both pages (valid, used, expired, signed in as someone else) has a test |
| **V0.2.5** | ✅ 2026-10-14 | A. After 0.2.0 | 📘 Update | Styles | `styles.css` and `blueprint-refinement.css` became twelve files in `src/styles/` (largest 879 lines) imported in cascade order, 47 dead rules removed, `npm run styles:check` in CI (C11) | No stylesheet over 1,000 lines; no visual change |
| **V0.2.6** | ✅ 2026-10-14 | A. After 0.2.0 | 🔧 Enhancement | Jobs | Both manual re-index endpoints answer `202` with the running job and finish in the background; `409` while a workspace pass runs; the chunks dialog follows the job (C14) | No request indexes in place; the admin dialog follows the job |
| **V0.2.7** | ✅ 2026-10-14 | A. After 0.2.0 | ✨ Feature | Console | `workspace reindex`, `repo sync` and `embeddings rebuild` for system administrators, each a dry run until `--yes` and audited, with `workspace list` and `repo list` to find what they take (F9) | Each has a dry run and an audit line |
| **V0.2.8** | ✅ 2026-10-14 | A. After 0.2.0 | 🔧 Enhancement | Map and health | Per-file centroids and pairwise comparisons kept between requests and redone only for files written since (a journal in storage); one changed file costs 4.6% of a full run at 50,000 passages, debug build (P11) | A one-file change recomputes in under 10% of a full run at 50,000 passages |
| **V0.2.9** | ✅ 2026-10-14 | B. Mail | ✨ Feature | System › Settings | SMTP over TLS only: settings with a sealed password, an outbox with retries (1, 5, 30 min, 2, 6 h), *Send test mail*, console `mail status` and `mail test`, a `repomemo-mail` crate tested against a scripted TLS server; the host follows the outbound address rules (F6a, decision 7) | A test mail arrives through a TLS server; a slow server never holds a request; the SMTP host follows the outbound address rules |
| **V0.2.10** | ✅ 2026-10-14 | B. Mail | ✨ Feature | People | Creating an invitation queues a mail with the link (who, where, role, expiry) when mail is set up and `REPOMEMO_PUBLIC_URL` is known; the link is still shown to copy and the answer says what became of the mail; names are cleaned so none can add a header (F6b) | The invited address receives the link; a crafted name cannot add mail headers |
| **V0.2.11** | ✅ 2026-10-14 | B. Mail | ✨ Feature | Sign-in | "Forgot your password?": the same answer for every address, 3 mails an hour per address plus the sign-in limits, a one-time link valid 1 hour that ends every session, an in-app notice (F12, decision 8) | The same answer for every address; rate-limited; a one-time link valid 1 hour ends every session |
| **V0.2.12** | 🔜 Planned | B. Mail | ✨ Feature | Notifications | Mentions and task assignments sent by email (F6c) | Sent once per event, never to the person who caused it |
| **V0.2.13** | 🔜 Planned | B. Mail | ✨ Feature | Accounts | Email verification on registration and on a changed address (F13, decision 9) | An unverified account cannot join, create or be added; invitations count as verification |
| **V0.2.14** | 🔜 Planned | B. Mail | ✨ Feature | Profile | Notification preferences per kind: in the app, by mail, both or none (F14a) | Each kind of notification honours the choice |
| **V0.2.15** | 🔜 Planned | B. Mail | ✨ Feature | Notifications | A daily digest by email (F14b) | One mail a day at most, only when something happened |
| **V0.2.16** | 🔜 Planned | C. Accounts | ✨ Feature | System › Users | Deactivate and reactivate an account (F15a, decision 10) | A deactivated account cannot sign in or use tokens; owners must transfer first |
| **V0.2.17** | 🔜 Planned | C. Accounts | ✨ Feature | System › Users | Anonymize a deactivated account (F15b, decision 10) | Personal data gone; what they wrote stays, credited to "Former member"; creator links no longer block (risk 12 closed) |
| **V0.2.18** | 🔜 Planned | C. Accounts | 🔒 Security | Registration | Open registration answers the same for a known address and mails its owner instead (M4 residual) | Registration can be opened without revealing who has an account |
| **V0.2.19** | 🔜 Planned | D. Remote repositories | 🔒 Security | Remote sources | Groundwork: an encrypted credential store per link and `REPOMEMO_GIT_ALLOWED_HOSTS`, remote links off when unset (S6a, decision 11) | Credentials are never returned or logged; the host list is checked before any network call |
| **V0.2.20** | 🔜 Planned | D. Remote repositories | ✨ Feature | Repositories | Link a remote repository over `https` with a token: partial bare clone, fetch on the poll schedule (F4 phase 2a) | A private GitHub or GitLab repository syncs through the existing engine |
| **V0.2.21** | 🔜 Planned | D. Remote repositories | ✨ Feature | Repositories | Remote repositories over `ssh` with a deploy key and pinned host keys (F4 phase 2b) | An unknown host key is refused; the key never reaches the command line |
| **V0.2.22** | 🔜 Planned | D. Remote repositories | 🔒 Security | Repositories | Clone limits: size and time caps, the clone counted in the storage allowance, clone sizes in System › Overview (S6b) | A clone over its limit stops and says why |
| **V0.2.23** | 🔜 Planned | D. Remote repositories | ✨ Feature | Repositories | Push webhooks from GitHub, GitLab and a generic sender (F16a) | An unsigned or wrongly signed call is refused (HMAC, constant time); a push queues one sync |
| **V0.2.24** | 🔜 Planned | D. Remote repositories | ✨ Feature | Artifact page | Last author and date per repository file (F16b) | Shown on the file and in search results, from one `git log` pass per sync |
| **V0.2.25** | 🔜 Planned | D. Remote repositories | ✨ Feature | Artifact page | `CODEOWNERS` as ownership hints (F16c) | The matching owners show on the file, as hints next to its lifecycle owner |
| **V0.2.26** | 🔜 Planned | D. Remote repositories | ✨ Feature | Citations | View a cited passage at the commit it names (F16d) | A citation opens the text as it was at that commit, labelled as such |
| **V0.2.27** | 🔜 Planned | E. Issues and pull requests | ✨ Feature | Connectors | GitHub issues as evidence, read-only token, incremental sync (F17a, decision 12) | Issues and their comments are searchable and citable; closed issues are *outdated* |
| **V0.2.28** | 🔜 Planned | E. Issues and pull requests | ✨ Feature | Connectors | GitHub pull requests as evidence (F17b) | Pull requests with their review discussion, linked to their commits |
| **V0.2.29** | 🔜 Planned | E. Issues and pull requests | ✨ Feature | Connectors | GitLab issues and merge requests, self-hosted included (F17c) | The same model as GitHub, including self-hosted GitLab |
| **V0.2.30** | 🔜 Planned | E. Issues and pull requests | 🔧 Enhancement | Artifact page | Links from issues and pull requests to the files, symbols and commits they mention (F17d) | A file shows the issues that name it |
| **V0.2.31** | 🔜 Planned | E. Issues and pull requests | 🔧 Enhancement | Connectors | Connector webhooks and remote rate limits (F17e) | Updates arrive on webhook; `Retry-After` is honoured; a revoked token pauses the connector and says so |
| **V0.2.32** | 🔜 Planned | F. Search, assistant, review | ✨ Feature | Assistant | Earlier turns go with each question, within a token budget (F5a) | A follow-up resolves "it" and "that file" |
| **V0.2.33** | 🔜 Planned | F. Search, assistant, review | ✨ Feature | Retrieval | Search across all of the caller's workspaces (F5b) | Results merged by score, each checked against its workspace's role and AI policy |
| **V0.2.34** | 🔜 Planned | F. Search, assistant, review | ✨ Feature | Web shell | A keyboard command palette (F19) | Jump to a workspace, file, memory card or setting, and start assistant actions, without the mouse |
| **V0.2.35** | 🔜 Planned | F. Search, assistant, review | ✨ Feature | Indexing | Code symbols and structure-aware chunks for Go and Java (F21a) | Both languages chunk by declaration and their symbols are searchable |
| **V0.2.36** | 🔜 Planned | F. Search, assistant, review | ✨ Feature | Indexing | Code symbols and structure-aware chunks for C# and PHP (F21b) | Same as V0.2.35 |
| **V0.2.37** | 🔜 Planned | F. Search, assistant, review | ✨ Feature | Review | A review queue: evidence needing review, stale memory cards and open health findings in one list (F23) | Each item opens where it is resolved; the queue empties as they are |
| **V0.2.38** | 🔜 Planned | F. Search, assistant, review | ✨ Feature | Artifact page | Version history of a file: earlier versions kept, viewable and comparable (F24) | Uploading a new version keeps the old text readable |
| **V0.2.39** | 🔜 Planned | F. Search, assistant, review | 🔧 Enhancement | Discussion | `@mention` suggestions while typing in comments (F25) | Only workspace members are suggested |
| **V0.2.40** | 🔜 Planned | G. Portability and operations | ✨ Feature | Workspace settings | Export a workspace as an archive (O2a) | Evidence, folders, memory cards with citations, tasks, comments and lifecycle history in one file with a manifest |
| **V0.2.41** | 🔜 Planned | G. Portability and operations | ✨ Feature | Workspaces | Import an archive into a new workspace, with a dry run (O2b) | An export imports on another 0.2.x server with everything linked as before |
| **V0.2.42** | 🔜 Planned | G. Portability and operations | 🔒 Security | Backups | Encrypted backups, AES-256-GCM with `REPOMEMO_BACKUP_KEY` (O6a, decision 14) | A backup restores only with its key; a wrong key is refused before anything is written (R2-5 closed) |
| **V0.2.43** | 🔜 Planned | G. Portability and operations | ✨ Feature | Backups | A second copy of each backup to another folder or network share (O6b) | The copy is verified against the manifest |
| **V0.2.44** | 🔜 Planned | G. Portability and operations | 🐞 Fix | Indexing | Document readings in a subprocess with time and memory limits (P10) | An overrunning reading is killed, not abandoned (R2-4 closed) |
| **V0.2.45** | 🔜 Planned | G. Portability and operations | ✨ Feature | Operations | A container image and a compose file with the proxy (O7) | `docker compose up` gives a working server behind TLS, with data in a volume |
| **V0.2.46** | 🔜 Planned | G. Portability and operations | ✨ Feature | System › Audit | Export of the audit trail as JSON lines or CSV (O8) | A date range downloads; the download itself is audited |
| **V0.2.47** | 🔜 Planned | H. Experience | 🔧 Enhancement | Web client | Accessibility pass: keyboard paths, focus, labels, contrast (X1) | Every page usable by keyboard alone; automated checks find no serious issue |
| **V0.2.48** | 🔜 Planned | H. Experience | 🔧 Enhancement | Web client | Narrow screens: the workspace pages and the assistant usable on a tablet or phone (X2) | No horizontal scrolling at 375 px; the dock becomes a sheet |
| **V0.2.49** | 🔜 Planned | I. One client | ✨ Feature | Desktop | The desktop app connects to a server: asks for its address, keeps the session in the system credential store, shows the web client (F20a, decision 13) | The desktop window and the browser show the same pages for the same account |
| **V0.2.50** | 🔜 Planned | I. One client | ✨ Feature | Desktop | *Export to a server*: a local workspace moves to a server through the archive format (F20b) | A local workspace appears on the server with its memory and citations |
| **V0.2.51** | 🔜 Planned | I. One client | 📘 Update | Desktop | The local edition retired: labelled in the app and the docs, removal planned for V0.3.x (F20c) | The docs no longer describe the local edition as current |
| **V0.2.52** | 🔜 Planned | J. Release candidate | 🔒 Security | Security | Feature freeze and the fourth security pass over mail, webhooks, remote clones, connectors, export and import, and the desktop client, with its fixes (R7, R8) | No high or medium finding open |
| **V0.2.53** | 🔜 Planned | J. Release candidate | 📘 Update | Release | Upgrade test from 0.2.0 environments in CI (R9) | Stored 0.2.0 schemas upgrade in place and pass |
| **V0.2.54** | 🔜 Planned | J. Release candidate | 📘 Update | Performance | Performance run at 50,000 passages plus a 100,000-file remote repository and 10,000 issues (R10) | The V0.2.0 targets still hold; results in the release notes |
| **V0.2.55** | 🔜 Planned | J. Release candidate | 📘 Update | Documentation | Mindmaps, branch pages (`repository-sources`, a new `connectors` page, `operations`), changelog and upgrade notes from 0.2.x (R11) | Every page matches the code |
| **V0.2.56** | 🔜 Planned | J. Release candidate | 📘 Update | Release | Clean-machine install with mail and a remote repository, following only the guide (R12) | Installed without a workaround; ready to tag |
| **V0.3.0** | 🔜 Planned | J. Release candidate | 📘 Update | Connected | The version set to `0.3.0` and tagged `v0.3.0` | Tag `v0.3.0` |
