# Changelog

What changed in each release, newest first, with what to do when upgrading. Versions follow [semantic versioning](https://semver.org): `0.2.0` is the next minor release (see the [roadmap](mindmap/ROADMAP.md#-road-to-v020)); the 0.2.x releases after it lead to `0.3.0` ([Road to V0.3.0](mindmap/ROADMAP.md#-road-to-v030), proposed). Releases before V0.1.63 are summarized in the roadmap's *Delivered* tables.

## V0.5.3 (2026-10-29)

Link provider identities to accounts only by explicit proof or invitation.

### Changed
- An authenticated session can start linking its own account (`GET`/`POST /v1/profile/identity`); the callback requires the provider proof and that session's unchanged version. A provider subject cannot be moved to another account, and an email change never changes the account behind an issuer/subject pair.
- New-account enrollment takes an `invitation_token` on sign-in start, a verified provider email matching the invitation and the domain allowlist. An email already in use is refused with a pointer to explicit linking, never merged.
- Migration 0047 adds an enrollment confirmation flag: a new identity cannot sign in until invitation acceptance completes. The sign-in session carries the account's session version, so a session revoked mid-flow is refused.

### Checks
- One new storage test covers subject reassignment, email takeover, revoked-session linking and unconfirmed enrollment; clippy (server, storage), formatting, migration, version and documentation checks pass. Tests were compiled but not executed; real-provider journeys remain pending.

### Upgrading
- Migration 0047 runs automatically; existing links stay confirmed. Rollback uses the pre-upgrade backup.

## V0.5.2 (2026-10-29)

Add the OIDC authorization, callback and session handoff.

### Changed
- Add single-use state, nonce and S256 PKCE, a fixed public callback, bounded token exchange and RS256 validation of issuer, audience, authorized party, expiry and fresh authentication.
- Only a linked issuer/subject can receive the existing session. A separate initiating-client proof consumes the result once; cancellation, expiry, replay and provider changes refuse sign-in. Browser refresh tokens use the existing HttpOnly cookie.
- Migration 0046 adds identity pairs and sealed five-minute handoffs with a server-wide cap. Explicit linking and enrollment follow in V0.5.3; no email-only linking is introduced.

### Checks
- Seven focused Rust tests pass, including real signed token cases, wrong proofs, expiry, replay and session issuance. Cargo check, formatting, migration, version and documentation checks pass. Real-provider/client journeys remain pending.

### Upgrading
- Migration 0046 runs automatically. Identity links and ephemeral handoffs are server-level data excluded from workspace archives; rollback uses the pre-upgrade backup.

## V0.5.1 (2026-10-29)

Configure and check a sealed OpenID Connect provider.

### Changed
- System administrators can save and check one provider from System settings. Credentials are sealed, never returned, and must be replaced when the issuer or client ID changes.
- Discovery enforces public HTTPS, same-origin endpoints, pinned DNS, no redirects or proxy, bounded responses and timeouts, and advertised code/PKCE/RS256/client_secret_post support. Enabling repeats discovery/key checks; sign-in itself follows in the next releases.
- Migration 0045 stores server identity settings outside workspace archives. Configuration changes use the existing audit trail; the team-access guide records protocol and provider limitations.

### Checks
- Four focused Rust tests and two UI tests pass. TypeScript, generated types, styles, accessibility, migration, version and documentation checks pass. Static UI review fixes applied. Live provider and visual validation remain pending.

### Upgrading
- Migration 0045 runs automatically. Full backup restoration needs the existing sealing key; provider settings are excluded from workspace archives.

## V0.5.0 (2026-10-29)

Open the team-access and identity implementation line.

### Changed
- Adopt the expanded release plan through V0.5.19 and preserve the following proposed lines. Existing guests, ownership transfer, audit export and citation editing are foundations, not new features.
- Add the team-access guide and correct stale citation-editing and cross-workspace-filter descriptions.
- The documentation checker follows the current minor index and checks every available release index. Live evidence remains pending under the maintainer's implementation-overlap decision.

### Checks
- Version, documentation and release-note checks pass. This release changes no runtime behavior; the preceding candidate test results are historical evidence, not a new live sign-in test.

### Upgrading
- No migration or access change.

## V0.4.105 (2026-10-29)

Record the final 0.4 candidate and the limits of its validation.

### Changed
- The final candidate evidence matrix separates local source checks from pending exact-commit workflows, image scans, clean installs and live desktop validation.
- Record the unchanged migration 0044 in the migration lock; it was missing its checksum.
- The maintainer authorized implementing the 0.5 line while live evidence stays pending; no distribution is newly certified ready and no push or tag is made.

### Checks
- Static checks, TypeScript, 92 web files / 496 tests and production npm audit pass. Rust tests pass (622 passed, four existing ignores), as do documentation and release-note checks. Sandbox filesystem refusals are recorded separately from test outcomes.

### Upgrading
- No schema change; migration 0044 is unchanged. Live install, upgrade, restore and provider checks remain pending.

## V0.4.104 (2026-10-28)

The documentation check covers every guide, version claims, command examples and setup paths.

### Changed
- `npm run docs:check` now checks the links of every guide in `docs/`, that the README's current version is the product's, that each `npm run` script, `deploy/` file and `repomemo-console` command shown in a guide exists, that no guide uses Compose v1 syntax or a second quick start URL, and that the Node and Rust prerequisites match `package.json` and `Cargo.toml`.
- A read-through of the README, quick start, user guide and administrator path fixed one contradiction: the user guide said saved searches are not in workspace archives, which the functional mindmap says they have been since V0.3.19.

### Checks
- `npm run docs:check` passes with the new rules, and fails with the expected messages when a README version, an npm script, a console command, a `deploy/` file and a Compose v1 command are made wrong on purpose.
- The read-through was done by reading the documents against the code and the mindmaps. Nobody followed the user or administrator workflows in a running app or on a clean machine, so those walkthroughs remain open for V0.4.105.

### Upgrading
- No migration is required.

## V0.4.103 (2026-10-28)

A trace from each major request and background job to its code and data stores.

### Changed
- `mindmap/technical/request-and-job-trace.md` maps the pieces (server, core, storage, ingestion, retrieval, AI, mail, console, client), the data stores, the request path through limits, authentication and guards, nine request traces, seven background job kinds, the desktop transport, and the deployment topology and security boundaries.
- The documentation index links it for contributors.

### Checks
- `npm run docs:check` passes. File and function names were checked against the repository with search; the page was not exercised against a running server.

### Upgrading
- No migration is required.

## V0.4.102 (2026-10-28)

The functional reference and PRODUCT.md no longer describe unshipped work as shipped.

### Changed
- The functional mindmap's known-gaps section opens with a table of what is proposed or not built (single sign-on, scale-out, a hosted service, published releases and signed installers, live accessibility validation) and a table naming the canonical guide for each primary workflow.
- PRODUCT.md states that remote repositories, mail and connectors are shipped and that single sign-on, scale-out, hosted deployment, signed installers and published releases are not; the planned branch documents are marked as unwritten.

### Checks
- `npm run docs:check` passes. Statuses were taken from the roadmap, the 0.5 single sign-on plan and the V0.4.73 evidence matrix.

### Upgrading
- No migration is required.

## V0.4.101 (2026-10-28)

An API guide for scripts that use personal access tokens.

### Changed
- `docs/API.md` explains token scopes, errors, paging, search budgets, limits and uploads, then walks a read workflow (workspaces, search, paged evidence) and a write workflow (note, upload, task, memory card), event streams, and which routes are for scripts.
- The API reference marks its audience: content sections for scripts, administration by scope, and sign-in, setup, platform, console, avatar and webhook routes as internal; its stale status line is replaced.

### Checks
- `npm run docs:check` passes, and a one-off check of every `docs/*.md` page's links also passes.
- Request bodies, header names, task states and scopes were read from the server's request types and storage rules. The curl workflows were not run against a live server.

### Upgrading
- No migration is required.

## V0.4.100 (2026-10-28)

A security administration guide with safe rotation sequences.

### Changed
- `docs/SECURITY_ADMIN.md` lists each secret and what it protects, every credential lifetime, and a safe sequence to rotate the session secret, the stored-credentials key, the backup key, a personal access token and a provider key.
- It covers cloud AI consent, TLS and trusted proxies, what the audit trail and security log record, and a checklist to verify access and audit behavior after a rotation.

### Checks
- `npm run docs:check` passes, and a one-off check of every `docs/*.md` page's links also passes.
- Lifetimes, defaults and rotation limits were read from the authentication page and the settings reference. The sequences were not rehearsed on a running server.

### Upgrading
- No migration is required.

## V0.4.99 (2026-10-28)

An operator runbook for health, logs, jobs, backup, restore, upgrade and rollback.

### Changed
- `docs/RUNBOOK.md` gives each routine a command, the signal that means healthy and the recovery when it is not: health checks, logs, metrics, jobs and maintenance, disk, backup, restore, upgrade, rollback and a failed-upgrade table.
- It closes with what was verified and what was not, and the documentation index links it.

### Checks
- `npm run docs:check` passes, and a one-off check of every `docs/*.md` page's links also passes.
- Console commands were matched to the admin console catalog and the scripts' arguments. `deploy/upgrade.sh` against released images was not run because no tag exists.

### Upgrading
- No migration is required.

## V0.4.98 (2026-10-28)

An administrator path from an empty host to a working team.

### Changed
- `docs/ADMIN_GUIDE.md` names the accounts involved (host operator, system, app, organization and workspace administrators) and lists eleven setup steps in order, each with who performs it and the guide for it.
- It covers invitations (one use, one email address, seven days), deactivating leavers, the desktop handout and the guides for upgrades, operations and errors; the index links it.

### Checks
- `npm run docs:check` passes, and a one-off check of every `docs/*.md` page's links also passes.

### Upgrading
- No migration is required.

## V0.4.97 (2026-10-28)

A guide to the team review lifecycle.

### Changed
- `docs/COLLABORATION.md` follows an item from flagged to verified to memory: review states, comments and mentions, the task board, notifications, saved searches, a worked example and how an owner audits the lifecycle.
- The documentation index links it.

### Checks
- `npm run docs:check` passes. Rules were taken from the functional mindmap's collaboration, review-queue and notification sections; the flow was not replayed in a running app.

### Upgrading
- No migration is required.

## V0.4.96 (2026-10-28)

A guide to search, Ask and citations, with the no-AI path.

### Changed
- `docs/SEARCH_AND_ASK.md` explains keyword and semantic search, the retrieve-then-generate order of Ask, what a citation proves and does not, missing sources in exports, and provider limits with worked examples.
- The documentation index links it.

### Checks
- `npm run docs:check` passes. Behavior statements were taken from the functional mindmap's retrieval sections (V0.4.78–V0.4.79) rather than run against a live provider.

### Upgrading
- No migration is required.

## V0.4.95 (2026-10-28)

The user guide opens with five task workflows.

### Changed
- A new section of the user guide walks through creating a workspace, adding evidence, reading the index state, reviewing sources and saving memory, each with who can do it, the steps and what to expect.
- The guide's version stamp now names the 0.4 line.

### Checks
- `npm run docs:check` passes. The role requirements, index states and review actions were read against the functional mindmap and the web client's labels; the steps were not walked through in a running app.

### Upgrading
- No migration is required.

## V0.4.94 (2026-10-28)

The first-run path is written from what was actually run.

### Changed
- The quick start gains prerequisites, a table of the five images and when each runs, the seven first-run setup steps for the first sign-in, and an honest *What has been tested* section.
- The Windows page says it is for building from source and points server installs to the quick start; the deployment guide links the setup steps and states the test status.

### Checks
- `npm run docs:check` passes. The tested and untested paths are copied from the V0.4.71 validation record: no tag exists, so the released-image pull, edge, upgrade and direct install remain unrun.

### Upgrading
- No migration is required.

## V0.4.93 (2026-10-28)

A task-based documentation index for every reader.

### Changed
- `docs/README.md` lists one canonical guide for each task, grouped for users, workspace owners, system administrators, operators, API users and contributors.
- The README links to it, and `docs:check` now checks its links.

### Checks
- `npm run docs:check` passes with `docs/README.md` among the checked documents.

### Upgrading
- No migration is required.

## V0.4.92 (2026-10-28)

The README is a short, current entry point.

### Changed
- The README drops the stale v0.1.x status, the placeholder worker and the long feature table, and states the current version.
- It offers one table of run paths (containers, direct install, desktop app, development checkout) and one table that sends each reader to the right guide.

### Checks
- `npm run docs:check` passes: every relative link in the README resolves.

### Upgrading
- No migration is required.

## V0.4.91 (2026-10-27)

Navigation, results and dialogs fit small windows and zoom.

### Changed
- A 320 CSS px layout wraps header controls into a second row so they remain available.
- Search results, notices and dialog actions wrap; stacked pages remain scrollable.

### Checks
- Focused narrow-screen tests and style checks pass. Live 200% zoom validation remains open because the local preview was unreachable.

### Upgrading
- No migration is required.

## V0.4.90 (2026-10-27)

Accessibility fixes for the assistant and review queue.

### Changed
- The narrow assistant behaves as a named modal sheet with trapped Tab focus and focus return.
- Review loading and fetch errors are announced inline with a retry action.
- Reduced motion removes decorative animation and transition while retaining immediate feedback.

### Checks
- Focused keyboard and review tests, contrast audit in both themes, TypeScript and style checks pass. Manual browser and screen reader validation remains open because the local preview was unreachable.

### Upgrading
- No migration is required.

## V0.4.89 (2026-10-27)

Workspace screens explain loading, partial results and failures with direct recovery.

### Changed
- Inline notices show workspace, evidence and task loading or fetch failures with retry actions.
- Search timeouts and errors offer a new search; empty filtered evidence and tasks offer Clear filters.
- Jobs distinguish an empty queue from a failed refresh, with recovery actions and no duplicate error toast.

### Checks
- Focused workspace and System feedback tests, TypeScript, styles, version and layout checks pass.

### Upgrading
- No migration is required.

## V0.4.88 (2026-10-27)

Dense evidence and System views keep long values and actions readable.

### Changed
- Evidence list rows show and wrap full paths, align selection, keep actions visible and state their newest-first order.
- Jobs and users tables use bounded columns and wrap long values; user sorting covers name, recent connection and join date.
- Task cards keep actions in a consistent position and support priority, due-date and title sorting.

### Checks
- Focused evidence, tasks and System component tests, TypeScript, styles, version and layout checks pass.

### Upgrading
- No migration is required.

## V0.4.87 (2026-10-27)

Search results are easier to compare and include evidence state.

### Changed
- Search responses carry the current evidence lifecycle status, including the active default for older records.
- Workspace and cross-workspace results separate title, state, source, path, match reason and citation preview.
- Result counts remain visible while a new search runs.

### Checks
- Focused search UI and lifecycle integration tests, Rust compilation, types, styles and version checks pass.

### Upgrading
- No migration is required.

## V0.4.86 (2026-10-27)

Workspace navigation now leads with the sections used most often.

### Changed
- A persistent, keyboard accessible workspace switcher replaces the long rail list.
- Search, evidence and review stay visible in the default workspace tabs; other sections remain in More, with the open section promoted.
- The workspace bar names the current workspace and section.

### Checks
- Focused workspace navigation tests, type, style, version and documentation checks pass.

### Upgrading
- No migration is required.

## V0.4.85 (2026-10-27)

The desktop private-CA relay has an end-to-end TLS regression.

### Changed
- A test-only localhost certificate and TLS server exercise streamed uploads and event responses through the relay.
- The regression checks the relay's route secret and confirms foreign redirects are returned without being followed.

### Checks
- The focused desktop relay test, formatting, version, types and documentation checks pass.

### Upgrading
- No migration is required.

## V0.4.84 (2026-10-27)

Revoked workspace access now has direct read and reconnect regressions.

### Changed
- Tests cover membership removal, session invalidation and personal token revocation across active streams, new reads and reconnects.
- Denied workspace reads record a system audit event with the workspace and actor.

### Checks
- Focused authorization regressions, Rust formatting, version, types and documentation checks pass.

### Upgrading
- No migration is required.

## V0.4.83 (2026-10-27)

A repeatable restore drill verifies backup copies and recovered records.

### Changed
- `npm run backup:drill` restores a checksum-verified copy into an isolated environment and checks accounts, evidence bytes and SQLite integrity.
- Restoring a copied backup checks its `SHA256SUMS`; all restores run a full database integrity check before success.
- The drill confirms the live fixture remains available and cleans up its temporary environment.

### Checks
- The focused restore drill, formatting, version, types and documentation checks pass.

### Upgrading
- No migration is required.

## V0.4.82 (2026-10-27)

Background indexing and embedding now give waiting workspaces turns.

### Changed
- Index requests run round-robin across workspace queues; retries rejoin the same fair queue.
- Embedding yields after two 32-chunk batches and resumes behind other workspaces.
- System Overview shows oldest queue waits and current-process index retry counts.

### Checks
- Queue, embedding, server and web checks pass.

### Upgrading
- No migration is required.

## V0.4.81 (2026-10-27)

Broad search requests now have a bounded response time and an explicit timeout state.

### Changed
- Workspace search has a five-second budget; cross-workspace search has ten seconds. Timed-out responses identify incomplete results.
- Queries over 512 characters are refused; existing term and result limits remain in force.
- The web client cancels superseded searches and suggests narrowing a timed-out query.

### Checks
- Server and web type checks, focused search UI/API tests, formatting and release checks pass.

### Upgrading
- No migration is required.

## V0.4.80 (2026-10-27)

Large evidence and jobs lists now offer bounded cursor pages.

### Changed
- Artifact and job list endpoints return a next cursor for stable, filtered paging; offset artifact requests remain supported.
- Evidence and System jobs views load one server page at a time and retain previous-page cursors.
- Artifact ordering now breaks timestamp and path ties by ID.

### Checks
- Storage cursor regressions, server and web type checks, UI tests and release checks pass.

### Upgrading
- No migration is required.

## V0.4.79 (2026-10-27)

Answer and memory card Markdown exports carry portable citation provenance.

### Changed
- Exported citations include artifact and source IDs, line or section location, and review status.
- Missing or inaccessible sources are explicitly marked with their original citation IDs.
- Ask answers have an Export answer action.

### Checks
- Focused export tests, web typecheck, generated types, formatting and release checks pass.

### Upgrading
- No migration is required.

## V0.4.78 (2026-10-27)

Saved searches can remain private or be shared by a workspace owner.

### Changed
- New saved searches are private to their author. Owners can share or make one private; existing searches remain shared after migration.
- Listing returns shared searches and the caller's own private searches. Opening a saved search runs the query again through current workspace access and filters.
- Workspace archives preserve the sharing state, and older archives retain the formerly shared behavior.

### Checks
- Web typecheck, styles check, saved-search UI tests and server permission regression pass.

### Upgrading
- Migration 0044 adds saved-search visibility. Existing searches remain shared; new searches start private.

## V0.4.77 (2026-10-27)

Search results now state how a passage matched, and retrieval filters survive a shared URL.

### Added
- Results identify keyword, semantic or combined matches; code definition results identify symbol matches.
- Retrieval query, type, language, source, lifecycle, folder, dates and limit are reflected in the URL. Opening a URL with a query runs that search with its filters.
- The search response signals when semantic search was unavailable; the page then names the keyword-only fallback.

### Checks
- Retrieval fusion tests, focused web search tests, web typecheck, and semantic endpoint regression pass.

### Upgrading
- Nothing to do. The search result adds `match_kind` and the endpoint adds an exposed response header.

## V0.4.76 (2026-10-27)

Failed evidence indexing can be retried from its item and workspace Health.

### Added
- An item with a recorded index failure shows the reason and offers a scoped retry to a writer. Health offers the same action for each failed item to an owner or administrator.
- The retry shows its accepted job ID and status, then its terminal result or server refusal. Completed retries refresh the evidence and Health views.

### Checks
- Focused retry tests, web typecheck, styles check, version and documentation checks pass.

### Upgrading
- Nothing to do. The existing per-item indexing endpoint and job queue are reused.

## V0.4.75 (2026-10-27)

Bulk evidence review now previews the selected files and reports each result.

### Changed
- The selection toolbar opens a review preview before changing lifecycle state. The result names each updated item and each failure; failed items stay selected for another attempt.
- The existing bulk API keeps the item's review history and rejects an item from another workspace while processing eligible items.

### Checks
- Web typecheck, styles check, focused evidence UI tests and the bulk API regression test pass.

### Upgrading
- Nothing to do. The bulk endpoint and stored schema are unchanged.

## V0.4.74 (2026-10-27)

Evidence detail now shows provenance and freshness in one place.

### Added
- Source name and identifier, import or latest source sync time, current content time, last successful index, index version, and lifecycle review state appear together on the evidence page.
- The detail API reads the stored index version and source sync timestamp; a changed item without a current index says so explicitly.

### Checks
- Storage tests, web typecheck, styles check and provenance component tests pass.

### Upgrading
- Nothing to do. The API response adds two fields; stored data needs no migration.

## V0.4.73 (2026-10-27)

Added a release evidence matrix before the daily-workflow releases.

### Added
- [The matrix](mindmap/technical/release-evidence-0.4.73.md) links local web, Rust and desktop checks; image-scan configuration; container and desktop install records; unresolved risks; and maintainer sign-off. It lists built, tested and released status separately for each distribution path.

### Fixed
- `container:check` now accepts completed six-column clean-install record rows instead of requiring the rows to remain blank. It still requires C1–C9 and D1–D10 to exist.

### Checks
- `version:check`, `container:check` and `docs:check` pass. Exact-commit GitHub Actions runs, image-scan reports and tag-based release artifacts remain unavailable because these commits have not been pushed.

### Upgrading
- Nothing to do. This release changes the evidence record and its local check.

## V0.4.72 (2026-10-27)

Recorded Windows desktop build and handout evidence, with explicit status for D1–D10.

### Checked
- `cargo test -p repomemo-desktop --locked` passed 38 tests; `desktop:test` passed 9 package tests, `desktop:parity:test` passed 6 tests, and `desktop:check` passed.
- A fresh Windows x64 executable and unsigned NSIS installer built in an isolated target directory because an older desktop process was running from the normal release path. The installer SHA-256 is `2aca7af724d114c78582577b715683c494e6a2d2f8635d72f93916e6836a8d39`.
- A `--from` handout produced a portable zip, configuration, installer and verified checksums. [The D1–D10 record](mindmap/technical/release-validation-0.4.72.md) marks live installation, private-CA relay, everyday flows and removal untested; signed updates are blocked on credentials, and Linux/macOS runs require those platforms.

### Changed
- Set version `0.4.72` everywhere and updated the roadmap, clean-install sheet, release index and mindmaps. No desktop behavior changed.

### Upgrading
- Nothing to do. The local validation artifacts are unsigned and were not published.

## V0.4.71 (2026-10-27)

Recorded the first local container clean-install run for the 0.4.71 candidate.

### Fixed
- A server pinned to an environment now prints its generated first-run setup code in the container logs, as the quick start directs.
- The smoke script reads the current `workspace.id` response and reports an indexing timeout only when indexing actually times out.

### Checked
- The local compose package passed checksum verification and rejected a tampered archive without unpacking. Locally built server and web images became healthy; first-run setup and all 25 smoke checks passed.
- A four-volume backup restored into a second stack. The original account signed in and its uploaded note was present. `cargo test -p repomemo-server --lib --locked` passed (245 tests; one existing ignored test); `version:check`, `container:check`, `pins:test` and `cargo fmt --all --check` passed.
- [C1–C9 evidence](mindmap/technical/release-validation-0.4.71.md) records dates, platform, local image IDs and status. C4, C6 and C9 need published artifacts and a tag; C7 and C8 remain untested. Nothing was pushed.

### Upgrading
- Nothing to do. The setup-code fix applies when a new pinned environment starts without an explicit code.

## V0.4.70 (2026-10-27)

Distribution workflows now run web and Rust tests on the exact commit before they build or publish.

### Added
- A reusable `release-tests.yml` gate runs `npm run test:web` and `cargo test --workspace --exclude repomemo-desktop --locked`. The image, archive, Windows, Linux and macOS workflows require it before their build jobs. Each test job reports the commit and result in its GitHub summary.
- Each platform build reports its job result, commit and artifact name with its checksum list (or image ID), or says the artifact is unavailable. The build jobs report failures as well as passes.

### Fixed
- The local Rust gate found an AI endpoint policy mismatch, an incorrect PDF preview for invalid files, and upload attempts that counted against the hourly quota even when storage limits refused them. Those paths now enforce the intended behavior.
- Updated server tests whose fixtures or assertions no longer matched the current API, configuration validation, and inherited organization access.

### Changed
- `container:check` verifies that the five distribution workflows still call the gate before building and produce an evidence summary. Set the version to `0.4.70` everywhere. Updated the roadmap and release index.

### Checks
- **Pass:** `container:check`, `version:check`, `docs:check`, `pins:test`, `cargo fmt --all --check`, `npm run test:web` (88 files, 476 tests), and `cargo test --workspace --exclude repomemo-desktop --locked` with incremental compilation disabled. The workflow runs themselves require a pushed ref and have not run here.

### Upgrading
- Nothing to do. The release workflows perform more tests before producing artifacts.

## V0.4.69 (2026-10-27)

The web test suite completes locally: 88 files and 476 tests pass.

### Fixed
- Form validation finds the first invalid control through its `validity` state instead of a recursive `:invalid` selector, removing the jsdom stack overflow and preserving focus on the first refused field.
- The Vitest setup supplies browser layout APIs absent from jsdom (`ResizeObserver`, `scrollIntoView`). Test fixtures and assertions now use fresh response bodies, account for asynchronous cache and navigation updates, and match the current citation, palette, import, template and stylesheet behavior. No test was skipped or removed.

### Changed
- Set the version to `0.4.69` everywhere. Updated the [test-debt record](mindmap/technical/web-test-debt.md), roadmap and release index.

### Checks
- **Pass:** `npm run test:web` (88 files, 476 tests), `npm run typecheck`, `npm run version:check`, `npm run docs:check`. CI is not run for this unpushed commit.

### Upgrading
- Nothing to do.

## V0.4.68 (2026-10-27)

The web test failures are inventoried for repair in V0.4.69.

### Added
- [Web test debt](mindmap/technical/web-test-debt.md) records each of the 22 failing assertions with its reproduction command, observed cause, disposition and owner. A separate field-validation test hangs in jsdom and prevents the combined suite from finishing here.

### Changed
- Set the version to `0.4.68` everywhere. Updated the roadmap and 0.4 release index; V0.4.69 remains the repair release. No test was skipped or deleted.

### Checks
- All 88 web test files ran in isolated Vitest processes on Windows: 22 failed assertions in 14 files, 73 files passed, and one file was stopped after 25 seconds for the constraint-validation loop. The normal combined suite first encountered sandbox `EPERM` temporary-file errors, then stalled on the loop when given a writable temporary directory. `version:check` and `docs:check` passed.

### Upgrading
- Nothing to do. This release changes the test record and version, not runtime behavior.

## V0.4.67 (2026-10-27)

The 0.4.x line is closed: preparation for deployment, on the server and on the client. The next theme is proposed.

### What the line made (V0.4.1–V0.4.66)
- **Configuration and the server image:** a checked configuration (`--check-config`), build details in `/health`, a check of the folders, split data folders, a clean stop, a locked-down image with a health check and a clear version; one server release line with the app.
- **The web image and compose:** the web client as its own image with runtime configuration, an optional TLS edge, a converter for Office previews, health checks, logs in JSON, an example Prometheus, backups and restores of the volumes, an upgrade script that falls back, tests in CI that start the stack and upgrade it from the previous release.
- **Direct install:** a server archive for Linux and Windows, a hardened systemd unit and a Windows service, front doors, archives built natively in CI and attached to the release, and a start that refuses a database written by a newer release.
- **Registry and release pipeline:** four images on GHCR for two architectures with signed provenance and an SBOM, a release page written from the changelog, compose on the released images, a quick start with a checksum, and pinning rules enforced by a test.
- **The desktop app on the web client:** one transport and one look, a preset server and a configuration file, an administrator's packaging command, Windows installers (per person, silent, an uninstaller that offers to forget the session), identity (icons, one window, remembered place), a native save dialog, notices and a badge, `repomemo://` links, a private certificate authority, signed updates, code signing, a parity check, and builds for Linux and macOS.
- **Documents and security:** the deployment guide, the desktop administrator's guide, where the app keeps its files, a troubleshooting page that CI keeps true to the code (47 errors), a security pass (0 high, 1 medium, 2 low, all fixed) and the clean-install run with a record sheet.

### Added
- **[The plan for the next line](mindmap/plans/2026-10-27-0.5-single-sign-on.md):** single sign-on with OpenID Connect proposed as the theme of 0.5.x, with the ten releases of the earlier plan renumbered `V0.5.1`–`V0.5.10` after a `V0.5.0` that opens the line, four decisions to confirm (41–44), what 0.4 leaves to the maintainer (the record sheet, the first tag, the keys and certificates, Linux arm64 and updates for Linux and macOS, the Rust tests that were compiled and not run), and how the line is tested.

### Changed
- **The roadmap and the release index** close the line (67 of 67) and point to the next plan; decisions 41–44 are proposed. **The earlier single sign-on plan** says it is proposed again. **The version** is `0.4.67` everywhere.

### Checks (repeated for the closing release)
- **Pass:** `version:check`, `container:check`, `docs:check`, `migrations:check`, `templates:check` and `templates:test`, `types:check`, `styles:check`, `a11y:check`, `typecheck`, `package:test`, `notes:test`, `pins:test`, `desktop:test`, `feed:test`, `desktop:parity:test`, `desktop:check`, `troubleshooting:check` and its tests, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo deny check` (with the maintainer-approved zip ignores), `npm audit --omit=dev`.
- **Same as the baseline:** the web suite has 22 failing tests on this machine (`ResizeObserver` and the like); they are the same 22 as at V0.4.43, before the desktop work began, and none is new.
- **Not run, and still the maintainer's:** `cargo test`, every workflow, every image, installer and package, the app, and the nineteen steps of the clean-install run (`mindmap/technical/clean-install.md` § 6). The line is closed as written and checked, not as tried.

### Upgrading
- Nothing to do. The next minor waits for the maintainer's decision on the theme and for the clean-install run.

## V0.4.66 (2026-10-27)

The clean-install page has the 0.4 run: what the maintainer does on a clean machine and on the desktop systems, and a sheet to record it.

### Added
- **§ 6 of `mindmap/technical/clean-install.md`, *The 0.4 run*:** what to have (Docker with Compose v2; before the first tag, the local route for the quick start; each workflow run once by hand; a Windows, a Linux and a macOS computer), then **nine container steps** (C1 fetch with the quick start and its checksum, C2 initialise and start, C3 first-run setup and the smoke test, C4 verify the four images, C5 backups and the volumes' scripts into a second stack, C6 upgrade and fall back, C7 previews and monitoring, C8 a direct install on Linux and Windows, C9 the release page of a tag) and **ten desktop steps** (D1 the Windows installer, D2 a handout, D3 a bad configuration file, D4 a private authority, D5 everyday use, saving and one window, D6 notices and links, D7 updates, D8 removal, D9 Linux, D10 macOS). Each names the guide it follows and what should be seen.
- **A record sheet** with a row for each of the nineteen steps (date, machine, result, workaround, fixed in), and the rule of § 1: anything done that the guide does not say is a workaround, written down, and a release fixes it. When every row is `ok` with none, the next minor can be cut.

### Changed
- **`container:check`** requires the section, a step and a record row for each of the nineteen, and that the run follows the deployment guide, the quick start, the administrator's guide, the troubleshooting page, the image verification and the desktop files. **The documents:** the operations guide, the technical mindmap, the roadmap and the release index. **The version** is `0.4.66` everywhere.

### Checks
- **Run:** `container:check`, `version:check`, `docs:check`.
- **Not run:** the steps themselves. This release gives the maintainer the page to run them from; it is not their result.

### Upgrading
- Nothing to do.

## V0.4.65 (2026-10-27)

A security pass over what 0.4.x added around the server and the client: one medium and two low findings, all fixed; none open.

### Added
- **[The review](mindmap/security/2026-10-27-deployment-security-review.md)** (the eighth pass): the cross-origin cookie, the web image and `config.json`, `_FILE` secrets, the release pipeline and the images, and the desktop app's configuration file, installers, links, relay, updates, signing and save dialog; by a static reading of the code and the workflows, with the existing checks read as part of it. **0 high, 1 medium, 2 low, 5 info.**

### Fixed
- **X1, medium:** the Windows workflow gave the update **signing private key** to every step of the job, `npm ci` (whose dependencies run install scripts) included. It is now given to the one build step that signs; `container:check` refuses it at job level. What remains (the Rust dependencies' build scripts run inside that step) is written down as accepted.
- **X2, low:** the web image put `REPOMEMO_PRODUCT_NAME` into `/config.json` unchecked, and Caddy reads `{placeholders}` in a response at request time, so a name with `{env.…}` would have been answered to anyone. The entrypoint now refuses a quote, backslash, brace, dollar sign, backtick, control character, second line or more than 60 characters, and CI starts the image with three bad names and requires the refusal.
- **X3, low:** a `NAME_FILE` secret was read whole with no limit. At most 64 KiB is read now and a larger file is refused by name; a test covers both sides.

### Changed
- **The troubleshooting page** lists the new refusals, and `troubleshooting:check` now also reads the web image's entrypoint (47 errors, all listed). **`container:check`** keeps the three fixes fixed and the review's count honest. **The documents:** the roadmap's list of reviews, the operations guide, the technical mindmap and the release index. **The version** is `0.4.65` everywhere.

### Accepted and written down
- X4 the signing key during the dependencies' build scripts; X5 a cookie request with no `Origin` (protected by the custom header and the preflight); X6 `locked` and the compiled-in address are policy, not a barrier against the person at the keyboard; X7 the tag holder can change the publishing workflow (protect the `v*` tags); X8 `curl | sh` for the quick start (the careful way is given first-class).

### Checks
- **Run:** `cargo clippy -p repomemo-server --all-targets -- -D warnings`, `cargo fmt --all`, `troubleshooting:check` and its tests, `container:check`, `version:check`, `docs:check`; the web image's name rule was exercised in a shell against nine names.
- **Not run:** the new Rust test (compiled with clippy, not run), the web image itself, the workflow. Nothing was run against a live system.

### Upgrading
- Nothing to do. A web image given a product name with a quote, brace, dollar sign or backtick now refuses to start and says why; remove the character.

## V0.4.64 (2026-10-27)

Every error a start and the first-run setup can raise has a sentence, a cause and a remedy, and CI keeps it so.

### Added
- **`docs/TROUBLESHOOTING.md`:** *The three checks* (`repomemo-server --check-config` and its exit codes, `/health` and `/health/ready`, *System › Overview* and *Logs*, `docker compose ps` and the JSON log); *The server will not start: settings* (every refused setting: the secrets, the files, the web folder, plain HTTP, the public address and origins, the proxies, the cookie, the product name, the document reader, the split folders, the absolute-path, yes/no and value rules, the backup keys); *folders* (a missing or misplaced folder, not writable, full, under the disk floor, on a network file system); *the database* (a database written by a newer release, by marker or by migration, and a failed migration); *The first-run setup* (the six sentences the setup answers with); and *Questions* (sign-in lost after a restart, provider keys missing, which version runs, the desktop app's configuration error, a private authority, checking a download, going back).
- **`npm run troubleshooting:check`** (`scripts/check-troubleshooting.mjs`, four tests, in CI): reads the places the errors are raised (`config.rs`, `setup.rs`, `check_config.rs` and the storage crate's `newer.rs`: what `bail!`, `with_context`, `anyhow!`, `ApiError` and the folder check's errors and warnings are given) and **fails when a sentence is not quoted on the page**, so a new error cannot ship without its cause and remedy. It finds 40 today and all are quoted.

### Changed
- **`container:check`** requires CI to run the check and its tests, the script, and the page's six sections. **The documents:** the operations guide, the technical mindmap, the roadmap and the release index. **The version** is `0.4.64` everywhere.

### Checks
- **Run:** `troubleshooting:check` (40 errors, all listed), its tests (4 passed), `container:check`, `version:check`, `docs:check`.
- **Not run:** each error was not provoked on a running server; the causes and remedies are read from the code and the guides. The check finds errors by the shape of the Rust that raises them, so an error raised another way is not seen by it.

### Upgrading
- Nothing to do.

## V0.4.63 (2026-10-27)

One page takes an administrator from "I want my team on the desktop app" to a working install, an update path and a way to remove it.

### Added
- **`docs/DESKTOP_ADMIN.md`:** *1. What you hand out* (the program per system and the artifacts CI keeps, the configuration file, optionally the authority's certificate); *2. Package it for your server* (`desktop:package` with its flags, and a table of building with Rust, where the address is compiled in, against taking the CI artifact and adding the file, which can be deleted); *3. Hand it to a team* (silent install, where the file goes, what people see, the warnings of an unsigned app, the notices and links, the release-line rule); *4. Keep it current* (your own signed feed, the person's choice for the project's releases, or installing over the old one; the server first); *5. Take it away* (one person, the sign-in only, the app, the data, and why ending an account is done on the server).
- It names only what exists: the flags, the files and the three artifacts are checked against the repository.

### Changed
- **`container:check`** requires the five sections, the commands, flags, files and artifacts it names, and that the scripts and workflows it relies on are there. **The documents:** the operations guide, the technical mindmap, the roadmap and the release index. **The version** is `0.4.63` everywhere.

### Checks
- **Run:** `container:check`, `version:check`, `docs:check`.
- **Not run:** the page was not followed to hand a team a working app; V0.4.66 gives the maintainer the steps and a record sheet.

### Upgrading
- Nothing to do.

## V0.4.62 (2026-10-27)

One page takes a new administrator from a machine to a running, backed-up, upgradable server.

### Added
- **`docs/DEPLOYMENT.md`:** *1. Choose* containers or a direct install (a table: what you need, what you get, how it is installed and upgraded, when to choose it; containers when in doubt); *2. Size it* (memory, disk with the backups counted, CPU, network, and the limits that protect the disk); *3. The first hour* (the quick start and `init.sh` for containers, the archive, its installer and a front door for a direct install, then for both the first-run setup, a look at *System › Overview* and `/health`, the first backup before anyone depends on the server, the optional pieces and the desktop app); *4. Backups* (the server's own, the second copy it checks, the volumes' scripts, sealing, trying a restore); *5. Upgrading* (back up, read the changelog, `deploy/upgrade.sh` or the direct guide, look, and what going back means); *6. When something is wrong*.
- It names only what the repository has: the commands and files are the ones that exist, and the figures (memory, disk floor, upload and workspace limits, backups kept) are the defaults of the production guide.

### Changed
- **`container:check`** requires the page's five sections, its links to the quick start, the direct upgrade guide, the front doors, the image verification and the desktop files, the commands it gives, and that every script it names is there. **The documents:** the operations guide, the technical mindmap, the roadmap and the release index. **The version** is `0.4.62` everywhere.

### Checks
- **Run:** `container:check`, `version:check`, `docs:check`.
- **Not run:** the page was not followed on a clean machine; V0.4.66 gives the maintainer the steps to do that and a sheet to record it.

### Upgrading
- Nothing to do.

## V0.4.61 (2026-10-26)

An administrator can find, change and remove everything the app keeps on a computer, on Windows, macOS and Linux.

### Added
- **`docs/DESKTOP_FILES.md`:** for each system, where the program, the configuration folder, the saved session and the web view's own data are; what is in the configuration folder (`repomemo-desktop.json`, `server.json`, `.window-state.json`) and who writes each; and how to find what a machine is set up for, change the server, remove the saved sign-in without removing the app (`--forget-session`, or `cmdkey`, `security` or `secret-tool` by hand), remove the app and reset the page's own choices.
- **About logs, said plainly:** the app writes none of its own; a bad configuration file, an unreachable server, a refused link or a failed update is shown as a sentence on the page, and the server's *System › Logs*, *System › Overview* (which lists the client as `desktop`) and audit trail are where a connection problem is answered. A Linux program started from a terminal prints what it prints.
- **Links** from the development commands and the user guide.

### Changed
- **`container:check`** requires the page to name every file and flag the code uses and the three systems' places, checks that the saved-server file name and the credential-store service the page describes are the code's, and fails if a logging plugin is ever linked while the page says there is no log. **The documents:** the operations guide, the technical mindmap, the roadmap and the release index. **The version** is `0.4.61` everywhere.

### Checks
- **Run:** `container:check`, `version:check`, `docs:check`.
- **Not run:** nothing was looked at on a Mac or a Linux machine. The places for the web view's data and the credential entries' exact names are the operating systems' and the library's defaults for the app's identifier, written from their documentation, not found on a machine: look at one before scripting against them.

### Upgrading
- Nothing to do.

## V0.4.60 (2026-10-26)

CI builds the desktop app for macOS: one universal .dmg, signed and notarized when the credentials exist.

### Added
- **`.github/workflows/desktop-macos.yml`:** on a `v*` tag (the tag must be the version), on a pull request that touches the desktop app, and by hand. On `macos-latest`, with both Apple Rust targets, it builds `RepoMemo_<version>_universal.dmg` (`tauri build --target universal-apple-darwin --bundles dmg`: Apple silicon and Intel in one app, macOS 10.15 or later) and keeps it with `SHA256SUMS` as the artifact `desktop-macos-universal`. It publishes nothing and has read access only.
- **Signed and notarized with credentials:** the secrets `APPLE_CERTIFICATE` (a base64 `.p12`), its password and `APPLE_SIGNING_IDENTITY` sign the app; `APPLE_ID`, `APPLE_PASSWORD` and `APPLE_TEAM_ID` add Apple's notarization. They are read by the build step alone, under names of their own and exported only when present. **Without them** the app is signed ad hoc (`tauri.adhoc.conf.json`; an Apple-silicon Mac will not start a program with no signature) and the log says plainly that it is not notarized.
- **What CI checks:** the program holds `x86_64` and `arm64` (`lipo -archs`), the bundle identifier is `com.repomemo.desktop` and `repomemo` is registered as a link scheme in `Info.plist`, the signature is valid (`codesign --verify --deep --strict`), and, when notarization was asked for, `spctl --assess` and `stapler validate` pass; then the app is opened from the mounted image and must stay up.
- **Guidance** in the development commands (*The desktop app on macOS*): the credentials, what Gatekeeper asks of an app that is not notarized and how a person or IT answers, where the configuration file goes, and that the session is in the Keychain.

### Changed
- **`tauri.conf.json`** sets the minimum macOS version. The same preset address and configuration file work on macOS unchanged. **`container:check`** holds the runner, the universal build, the checks, the notarization rule, the credentials read by the build step alone, the ad hoc fallback, the artifact and the absence of any write. **The documents:** the development commands, the parity audit, the operations guide, the technical mindmap, the roadmap and the release index. **The version** is `0.4.60` everywhere.

### Checks
- **Run:** `cargo check` (it reads the changed configuration), `desktop:check`, `container:check`, `version:check`, `docs:check`.
- **Not run:** the workflow, the .dmg, signing, notarization, the app on any Mac. No Apple credentials exist yet, so every build is ad hoc signed. Left out: signed updates for macOS (the feed carries Windows only).

### Upgrading
- Nothing to do.

## V0.4.59 (2026-10-26)

CI builds the desktop app for Linux: an AppImage and a .deb, with the libraries listed.

### Added
- **`.github/workflows/desktop-linux.yml`:** on a `v*` tag (the tag must be the version), on a pull request that touches the desktop app, and by hand. On Ubuntu 22.04 (the oldest glibc and WebKitGTK 4.1 a current Ubuntu or Debian still has, so the AppImage runs on newer systems) it installs the build libraries, builds with `tauri build --bundles appimage,deb`, and keeps `RepoMemo_<version>_amd64.AppImage`, the `.deb`, the bare program and `SHA256SUMS` as the artifact `desktop-linux-x64`. It publishes nothing and has read access only.
- **What CI checks:** the `.deb` says what it is and what it needs (`dpkg-deb --info`: the package name, `xdg-utils`, WebKitGTK 4.1), carries its desktop entry and registers `repomemo://` links; the AppImage is started on a display of its own (xvfb, no installation) and must stay up with a window titled RepoMemo; the `.deb` is installed with `apt` and the installed program is started the same way.
- **The libraries, written down** in the development commands: what to build with, what the `.deb` pulls in, FUSE 2 for the AppImage (or `APPIMAGE_EXTRACT_AND_RUN=1`), the Secret Service the session is kept in, where the configuration file goes, and what is not there yet.

### Changed
- **`tauri.conf.json`** no longer runs `npm.cmd` (a Windows-only name, the thing V0.4.49 noted): `npm run dev` and `npm run build` work on every system. The `.deb` depends on `xdg-utils`, which opens links outside the app.
- **The same preset address and configuration file** work on Linux unchanged (`REPOMEMO_PRESET_SERVER`, `repomemo-desktop.json`). **`container:check`** holds the runner, the bundles, the libraries, the three tries, the link registration, the artifact and the absence of any write. **The documents:** the development commands (*The desktop app on Linux*), the parity audit, the operations guide, the technical mindmap, the roadmap and the release index. **The version** is `0.4.59` everywhere.

### Checks
- **Run:** `cargo check` (it reads the changed configuration), `desktop:check`, `container:check`, `version:check`, `docs:check`.
- **Not run:** the workflow, either package, the app on any Linux system. Left out on purpose: Linux arm64, and signed updates for Linux (the update feed carries Windows only).

### Upgrading
- Nothing to do.

## V0.4.58 (2026-10-26)

Adding a screen or a style only the desktop app has now fails CI until it is listed with a reason.

### Added
- **`npm run desktop:check`** (`scripts/check-desktop.mjs`): reads `apps/desktop/src` and fails when a file exists for the desktop alone and is not in `apps/desktop/desktop-only.json`, or a stylesheet class is named for the desktop and is not listed. A file is desktop-only when its name says desktop, or it imports the desktop module or a Tauri package, or tests the runtime. A listed entry whose file is gone, that no longer knows about the desktop, or that gives no reason (at least a sentence) fails too, so the list cannot grow stale. Tests and generated files are not counted.
- **`desktop-only.json`:** the twelve files (the entry and the mount, the desktop's start, the connect screen, the configuration problem, the server note, the update offer and the six library files that talk to the shell or hold the seam) and the two styles of the server note, each with the reason it is not part of the web client.
- **`check-desktop.test.mjs`** (six tests): a clean web client passes; a screen named for the desktop, one that calls the shell and one that imports the desktop module each fail until listed; a style class named for the desktop fails until listed; a stale entry, a runtime-free file on the list and a reason that says nothing fail; tests and generated files are ignored; the real app passes. CI runs the check and its tests with the other checks.

### Changed
- **The parity audit** no longer keeps a list of files itself: the list is the check's, and the test that compared the audit's list with the code (V0.4.43) is gone. **`container:check`** holds that CI runs the check and its tests and that the list exists. **The documents:** the development commands, the parity audit, the operations guide, the technical mindmap, the roadmap and the release index. **The version** is `0.4.58` everywhere.

### Checks
- **Run:** `desktop:check` (12 files, 2 styles), `desktop:parity:test` (6 passed), the desktop parity page test (7 passed), `typecheck`, `container:check`, `version:check`, `docs:check`.
- **Not run:** CI itself.

### Upgrading
- Nothing to do.

## V0.4.57 (2026-10-26)

The Windows program and installers are signed when the repository has a certificate, and the warning for an unsigned build is explained.

### Added
- **Signing in CI:** when the repository has the secrets `WINDOWS_CERTIFICATE` (a base64 `.pfx`) and `WINDOWS_CERTIFICATE_PASSWORD`, the *Desktop Windows* workflow imports the certificate in the one step that reads it (the file is removed at once), writes a signing configuration with its thumbprint, SHA-256 and a timestamp server, and builds both installers and the program with it. A new step then **fails the job unless every `.exe` carries a valid signature and a timestamp**, so a signature that would end with the certificate does not pass. Without the secrets the build is unsigned and the log says so plainly (a notice per file).
- **Guidance:** *Windows code signing and the SmartScreen warning* in the development commands: how to make the secret, why SmartScreen warns for an unsigned (and for a newly signed) program, what a person does (*More info*, *Run anyway*, unblocking a downloaded zip), and what IT does (a publisher rule, or the hash from `SHA256SUMS`, or its software tool and the installer's `/S`), with the command that shows a file's signature. A handout built from a signed artifact stays signed, since the configuration file is separate from the program; the portable zip itself is not signed, the program in it is.

### Changed
- **`.gitignore`** keeps the generated `tauri.signing.conf.json` out of the repository. **`container:check`** holds that signing happens only when the repository has a certificate, that the certificate is read and its file removed in the import step and nowhere else, the thumbprint, SHA-256 and timestamp, both installers built with the configuration, the failure on a bad signature, the plain notice when unsigned, and the SmartScreen guidance. **The documents:** the development commands, the parity audit, the operations guide, the technical mindmap, the roadmap and the release index. **The version** is `0.4.57` everywhere.

### Checks
- **Run:** `container:check`, `version:check`, `docs:check`.
- **Not run:** the workflow, the import, a signed build, the signature check. No certificate exists yet; until the maintainer buys one and sets the secrets, every build is unsigned and says so.

### Upgrading
- Nothing to do.

## V0.4.56 (2026-10-26)

The app can look for a newer version of itself, offer it, and install it only if it is signed.

### Added
- **The updater** (`updates.rs`, `tauri-plugin-updater`): `check_update` looks in a feed and keeps what it finds, `install_update` downloads it, checks its signature and restarts the app. The updater is configured with `requireSignedVersion`, so an update announced for a version other than the one its signature was made for (a replayed older release) is refused as well as an unsigned or wrongly signed one; nothing "dangerous" (insecure transport, invalid certificates) is switched on, and `container:check` refuses it.
- **Who decides whether the app looks:** the administrator's `updates` in `repomemo-desktop.json` (`feed`, an `https://` address or `http://` on the own network, and `publicKey`, the key it is signed by): the app looks without asking and the person is offered what it finds. Otherwise the project's release feed (`.../releases/latest/download/latest.json`) in a build that carries the release key (`REPOMEMO_UPDATE_PUBKEY`), and only if the person turns on **Check for updates** in the account area (off until then, remembered). A build with neither never looks.
- **The page:** `UpdateOffer` looks once when the client is up and asks *Install and restart* or *Later*; a refusal by the shell (a signature that does not match) is shown as its own sentence.
- **The feed:** `scripts/update-feed.mjs` writes the `latest.json` from a version, the notes and, per platform, the installer's address and signature; it refuses an empty signature, a plain-http or login-carrying address, an unknown or repeated platform and a bad version (`npm run feed:test`, three tests, in CI).
- **The release:** the Windows workflow signs the installer when the repository has `TAURI_SIGNING_PRIVATE_KEY` (and its password) and builds with `tauri.updater.conf.json` (`createUpdaterArtifacts`); a new `publish-update` job, on a tag and only when the variable `REPOMEMO_UPDATE_PUBKEY` is set, attaches the signed installer, its signature and `latest.json` to the release. It is the only job of that workflow that writes. To start: `npx tauri signer generate`, put the private key in the secret and the `.pub` text in the variable.
- **Tests:** four Rust tests (the administrator's feed wins and is looked at without asking; the project's feed is the person's, and only in a build with its key; its address; the feed's rules in the file) and `appUpdates.test.tsx` (six).

### Changed
- **`deny.toml`** ignores RUSTSEC-2026-0367, -0368 and -0369: `zip` 4.6.1, brought by the updater, extracts with unsafe permissions and symbolic links. The updater extracts only an archive whose signature it has checked, as the person and not as root. **Approved by the maintainer on 2026-10-26**; revisit by 2027-01-15 or when the plugin uses zip 9.
- **`container:check`** holds the signed-version rule, the absence of anything dangerous, who decides, the registered commands, the publishing job's conditions and the feed tests. **The documents:** the development commands (`updates`), the user guide, the parity audit (the updates row is closed), the operations guide, the functional and technical mindmaps, the roadmap and the release index. **The version** is `0.4.56` everywhere.

### Checks
- **Run:** `cargo check`, `cargo clippy -p repomemo-desktop --all-targets -- -D warnings`, `cargo fmt --all`, `cargo deny check` (with the three ignores), `typecheck`, `a11y:check`, `styles:check`, `feed:test` (3), the desktop page tests (36 with the neighbours), `container:check`, `version:check`, `docs:check`.
- **Not run:** the Rust tests (compiled), the updater against a real feed, a signed build, an installation from an older app, the publishing job. The updater plugin's start needs a `pubkey` in `tauri.conf.json`, which holds a placeholder that the feed's own key replaces at run time; if the plugin refuses it at start, that is the first thing to look at. No signing key exists yet: until the maintainer makes one, no update is published and the project's feed is off.

### Upgrading
- Nothing to do. An app without a feed behaves as before.

## V0.4.55 (2026-10-26)

A server behind a company's own certificate authority works in the app, and the same bundle trusts nothing else.

### Added
- **A `certificate` in the configuration file:** the name of a PEM file beside `repomemo-desktop.json` (or a full path) holding the authority that signed the server's certificate: one to ten certificates, at most 256 KB, and only with a `server`. A bundle that is missing, is not PEM certificates or is too large spoils the whole file, which is shown and not obeyed (V0.4.47).
- **A relay on the loopback (`trust.rs`):** the HTTP plugin has no way to add a root, so for the file's server the shell starts a small relay (hyper and reqwest) on `127.0.0.1` and lets the window call **only the relay**, not the server. The relay forwards the path and query of each request to that one origin over TLS trusting **the bundle alone** (`tls_built_in_root_certs(false)`), follows no redirect, drops the headers of one connection, streams the bodies both ways (uploads, downloads and the live event stream) and answers 403 to anything without the run's secret (a new random value each run, sent in `x-repomemo-route`) and 400 to a request for any other address. A certificate error comes back as a 502 that says so.
- **The client's side:** `desktopServer.route()` tells the page where the relay is; the transport (V0.4.44) sends the server's own address's requests there with the secret and leaves the page knowing the server by its own address. A request for any other address is not rewritten, and the shell's scope refuses it.
- **Only for that server:** the relay is started when the app talks to the file's server (typed, suggested or locked); to any other address the app asks the system's ordinary trust. `desktop:package --certificate ca.pem` checks the bundle (a key is refused), hands it out as `repomemo-ca.pem` beside the configuration file and names it there.
- **Tests:** four Rust tests in `trust.rs` (the bundle's size and shape, the one forwarded path, the headers), two in `config.rs` (the field, the bundle read from beside the file and a bad one spoiling it) and one in `connection.rs` (the bundle belongs to the file's server only), a public test certificate (`testdata/test-ca.pem`, no key), three transport tests (the relay's address and secret, nothing else rewritten, the whole client through the relay) and two packaging tests.

### Changed
- **Dependencies:** `hyper`, `hyper-util`, `http-body-util`, `bytes`, `futures-util` and reqwest's `stream` feature in the desktop crate; `cargo deny check` reports advisories, bans, licenses and sources ok. `ConfigState` holds the certificates. A test of the capabilities of `platform` that V0.4.53 and V0.4.54 left expecting three flags now expects five.
- **`container:check`** holds the relay's rules (the bundle alone, the secret, one origin, no redirect, the loopback), that the bundle is tied to the file's server and that the window may call the relay only, the field and the client's rewrite. **The documents:** the development commands, the parity audit (the certificate-authority row is closed), the operations guide, the technical mindmap, the roadmap and the release index. **The version** is `0.4.55` everywhere.

### Checks
- **Run:** `cargo check`, `cargo clippy -p repomemo-desktop --all-targets -- -D warnings`, `cargo fmt --all`, `cargo deny check`, `typecheck`, `desktop:test` (9 passed), the desktop page tests, `container:check`, `version:check`, `docs:check`. The whole web suite fails 22 tests in other files on this machine (the missing `ResizeObserver` and the like); the same tests fail on the commit of V0.4.43, before the desktop work began, and none fails that did not there.
- **Not run:** the Rust tests (compiled), the relay against a server with a private authority, a real streamed upload or the live event stream through it. The relay is the largest piece of shell code so far: the first real server behind an internal CA is its test.

### Upgrading
- Nothing to do.

## V0.4.54 (2026-10-26)

A page of the web client opens in the app: `repomemo://` links, with only the app's own pages taken.

### Added
- **The link:** `repomemo://open?server=<origin>&path=<path>`. A browser page offers **Open in the app** (in the account area, for the page being shown, and on each notice of the Notifications page), made by `appLinkFor` for the server the page talks to. The scheme is registered for the person by the installer (the per-person registry on Windows) and the app is told of a link by the system: the one running copy gets it (`tauri-plugin-single-instance` with its `deep-link` feature, `tauri-plugin-deep-link`), and a copy started by the link keeps it until the client is ready (`take_pending_link`).
- **What the shell takes (`deeplink.rs`):** only the client's own paths (`/workspaces/...`, `/dashboard`, `/profile`, `/notifications`, `/system/...`, with a query and fragment), and only for the server the app is connected to. Everything else is refused with a reason the app shows as a toast: another host or scheme, `//`, a backslash, a control character, a `..` or an encoded slash in a part, a path over 2048 characters, a link with a login or a port, an extra parameter, a server that is not this one (the message names both), no page at all, or no server chosen yet. The client's own copy of the rules (`lib/appLink.ts`) is checked against the same cases, and its list of areas against the shell's and the router's.
- **Tests:** four Rust tests (the paths, a good link, every refusal with its reason, no server) and `appLinks.test.tsx` (nine) and `desktopLinks.test.tsx` (four: the link the app started with, one opened while it runs, one turned away, listening stops); CI's Windows job requires the scheme registered for the person.

### Changed
- **`DesktopApp`** shows the page of a link once the client is up, and `platform` gains `opensInApp` (browser only). **The documents:** the user guide, the parity audit (links into the app are closed), the operations guide, the functional and technical mindmaps, the roadmap and the release index. **The version** is `0.4.54` everywhere.

### Checks
- **Run:** `cargo check`, `cargo clippy -p repomemo-desktop --all-targets -- -D warnings`, `cargo fmt --all`, `cargo deny check`, `typecheck`, `a11y:check`, `styles:check`, the desktop page tests (51 passed, 13 new), `container:check`, `version:check`, `docs:check`.
- **Not run:** the Rust tests (compiled), a link from a browser into an installed app, the registration on any system. A link refused while the app has no server (a cold start) has no page to tell, so it shows nothing. A click on a system notification does not open the page: the platforms do not report it.

### Upgrading
- Nothing to do.

## V0.4.53 (2026-10-26)

A notice reaches the desktop while the app is in the background, and the icon shows what waits.

### Added
- **`NoticeWatcher`:** shows nothing; while a person is signed in it looks at their notices every minute (the ones their own notification preferences let through, so the notices "a person chose" are exactly those), tells them of a new unread one with a **system notification** when the window is out of focus or out of sight (one notice as itself, several as a count and the first titles), and never twice. The first look after the app opens tells about nothing; it only counts.
- **A badge for what waits:** the app's icon is marked with the unread notices plus the items waiting in Review across the person's workspaces (counted every five minutes, at most 25 workspaces; what a person snoozed does not wait). A count where the system has one (macOS, Linux), a red dot on the taskbar button on Windows, which has none; nothing when zero, and removed when the person signs out.
- **The shell's side (`notice.rs`, `tauri-plugin-notification`):** `notify` asks for permission when it has not been given, shows the notification and answers whether it did (a person who refused gets none); the title and body are cleaned to one line without control characters, shortened and never empty. `set_badge` sets the count or the dot.
- **`Notifier`** (`lib/notifier.ts`): the seam; a browser installs none and the watcher does nothing there, the app installs the shell's. **Tests:** two Rust tests (cleaning, the dot) and `nativeNotices.test.tsx` (ten: what a look finds, how notices are worded, the count of the queues and the focus rule; the watcher counts without telling at first, tells once from the background, stays quiet in front, counts the queues every fifth look, takes the badge away on sign-out, does nothing without a notifier).

### Changed
- **`SharedWebApp`** shows the watcher by capability (`platform.nativeNotices`). **`docs/USER_GUIDE.md`** § 13 now tells what the app adds (the set-up file, the save dialog, the notices and badge, the one window, removal). **`container:check`** holds the plugin, the commands and the watcher's three rules. **The documents:** the parity audit (the notifications row is closed), the operations guide, the functional and technical mindmaps, the roadmap and the release index. **The version** is `0.4.53` everywhere.

### Checks
- **Run:** `cargo check`, `cargo clippy -p repomemo-desktop --all-targets -- -D warnings`, `cargo fmt --all`, `cargo deny check`, `typecheck`, the desktop and notice tests (36 passed), `container:check`, `version:check`, `docs:check`.
- **Not run:** the Rust tests (compiled), a real notification, the permission prompt, the badge on any system. Clicking a system notification does not open the page yet (the notice waits on the Notifications page); `repomemo://` links (V0.4.54) are what will let it.

### Upgrading
- Nothing to do.

## V0.4.52 (2026-10-26)

Exports and downloads save where the person chooses, in every webview.

### Added
- **A native save dialog** (`files.rs`, `tauri-plugin-dialog`): the command `save_file` takes the file's bytes as the raw body of the call (not JSON) and the suggested name in a header, opens the system's save dialog, writes the bytes where the person pointed and answers the path, or nothing when they cancelled. The name is only a suggestion: it is cleaned of folders, forbidden characters and the names Windows reserves, shortened and never empty. A write that fails says where and why.
- **`setFileSaver`** in `lib/saveBlob.ts`: `saveBlob` now answers whether the file was saved, and the app installs `saveThroughShell` for the length of its window; a browser keeps the anchor download. A cancelled dialog shows no "downloaded" toast.
- **Tests:** three Rust tests (percent-decoding of the name, safe suggestions, nothing empty, reserved or endless) and `fileSaver.test.ts` (four: the browser's anchor and its address let go; the bytes, not JSON, with the encoded name; a cancel answers no; the installed saver is used and removable; a refused save reaches the caller), one test for a cancelled template download and one parity test that no code but `saveBlob` sets `.download`.

### Changed
- **Every download goes through `saveBlob`:** the archive export, the memory card export, the document download (and its open-in-Word save), the template file, the audit export and a log file each had a copy of the anchor code, six in all; they now call the one function. **The documents:** the parity audit (the downloads row is closed), the operations guide, the technical mindmap, the roadmap and the release index. **The version** is `0.4.52` everywhere.

### Checks
- **Run:** `cargo check -p repomemo-desktop --tests`, `cargo clippy -p repomemo-desktop --all-targets -- -D warnings`, `cargo fmt --all`, `typecheck`, the download-related and desktop page tests (70 passed, 6 new), `container:check`, `version:check`, `docs:check`.
- **Not run:** the Rust tests (compiled), the dialog, a save of an archive, a memory card and a document in the app. A file is held in memory and passed whole to the shell; very large archives are the limit of that.

### Upgrading
- Nothing to do.

## V0.4.51 (2026-10-26)

The app has an identity of its own: the logo as its icon, its details in the executable, one window, and a title that names where you are.

### Added
- **The icon set, made from the logo** (`apps/desktop/public/RM-logo.svg`, by Tauri's icon command through `npm run icons`): `32x32`, `128x128`, `128x128@2x`, `icon.png`, `icon.ico` (several sizes), `icon.icns` and the Windows store sizes, all committed and named in `tauri.conf.json`. `npm run icons` drops the phone folders the command also writes. The earlier icon was a 32-pixel square drawn by a script.
- **The executable says what it is:** publisher, copyright, category, short and long description and homepage in `tauri.conf.json`, which Tauri writes into the Windows version resource, the installer's *Publisher* and the Linux package. CI reads the version resource of the built executable and requires the product name, the file version and a description.
- **One copy per person:** `tauri-plugin-single-instance`: a second start brings the first window to the front (unminimized, shown, focused) and ends. The uninstaller's `--forget-session` start (V0.4.50) is not held up by a running copy. CI starts a second copy and requires that it ends while the first stays.
- **The window comes back where it was:** `tauri-plugin-window-state` keeps the size and place and restores them at the next start.
- **A title that names the workspace:** `followPageTitle` makes the title bar the page's `document.title` ("Platform · Evidence · RepoMemo", which carries the workspace's name), with the `core:window:allow-set-title` permission, a `MutationObserver` and three tests (`windowTitle.test.ts`).

### Changed
- **Dependencies:** `tauri-plugin-single-instance` and `tauri-plugin-window-state` (and what the first needs on Linux); `cargo deny check` reports advisories, bans, licenses and sources ok.
- **`container:check`** holds the icon files, the executable's details, the two plugins and the order that keeps the uninstaller's start free, the title permission and its use, and CI's two new looks. **The documents:** the development commands, the parity audit (the window row is closed), the operations guide, the technical mindmap, the roadmap and the release index. **The version** is `0.4.51` everywhere.

### Checks
- **Run:** `cargo check -p repomemo-desktop --tests`, `cargo clippy -p repomemo-desktop --all-targets -- -D warnings`, `cargo fmt --all`, `cargo deny check`, the desktop page tests (36 passed, 3 new), `typecheck`, `container:check`, `version:check`, `docs:check`.
- **Not run:** the app, the second copy, the remembered size and place, the title bar. The window-state file (`.window-state.json` in the config folder) is not removed by *forget the sign-in*; it holds no account data.

### Upgrading
- Nothing to do.

## V0.4.50 (2026-10-26)

The Windows installer installs for one person, silently when asked, and offers to forget the session when it removes the app.

### Added
- **Per person, no administrator rights:** the NSIS installer is `installMode: currentUser`: it installs into `%LOCALAPPDATA%\RepoMemo`, records itself under the person's own uninstall list (`HKCU`) and has no language picker. The Start menu entry and the desktop shortcut are the installer's own, made also on a silent install; `/NS` leaves them out.
- **Silent install for IT:** `RepoMemo_<version>_x64-setup.exe /S` (and `/D=C:\path` last for another folder), documented with where the configuration file goes.
- **An uninstall that offers to forget the saved session:** interactively it asks (default No) whether to also forget the server and the sign-in saved on the computer; if yes, it runs the program with the new flag `--forget-session`, which, without opening a window, deletes the sessions in the Windows credential store for every server it knows (the packaged one, the configuration file's, the saved one) and the saved `server.json`, and leaves the administrator's file alone. A silent removal (`/S`) asks nothing and keeps them, so an update keeps the person signed in; `RepoMemo.exe --forget-session` first forgets on a silent removal.
- **Tests:** two Rust tests (the flag is the one argument after the program's name; every server with a session is forgotten once). CI's Windows job now installs the installer silently and checks that it left the program, the uninstaller, both shortcuts and the entry for the person (and none for the machine), opens the installed program, and removes it silently, checking that the program and the shortcuts are gone and the saved server kept.

### Changed
- **A release build has no console window** behind the app (`windows_subsystem = "windows"`); V0.4.49 started the app with one, which would also have confused its window-title check.
- **`container:check`** holds the per-person mode, the hooks, the flag, the subsystem and the CI steps. **The documents:** the development commands (*The Windows installer*), the parity audit, the operations guide, the technical mindmap, the roadmap and the release index. **The version** is `0.4.50` everywhere.

### Checks
- **Run:** `cargo check -p repomemo-desktop --tests`, `cargo clippy -p repomemo-desktop --all-targets -- -D warnings`, `cargo fmt --all`, `container:check`, `version:check`, `docs:check`. The installer's hook macros and defines (`MAINBINARYNAME`, `NSIS_HOOK_PREUNINSTALL`, the shortcut functions and `/NS`) were read in Tauri's installer template.
- **Not run:** the Rust tests (compiled, not run), the installer, the uninstaller, the workflow. The hook has not been through NSIS; CI's first build is its compile.

### Upgrading
- Nothing to do. An app installed by an earlier build (none was published) is not touched.

## V0.4.49 (2026-10-26)

CI builds the Windows desktop app and keeps the installer and the portable executable.

### Added
- **`.github/workflows/desktop-windows.yml`:** on a `v*` tag (the tag must be the version), on a pull request that touches the desktop app, and by hand. On `windows-latest` it builds the app with `tauri build --bundles nsis` and keeps the portable executable (`repomemo-desktop.exe`) and the per-person installer (`RepoMemo_<version>_x64-setup.exe`); builds the installer again with `tauri.offline.conf.json` and keeps it as `RepoMemo_<version>_x64-offline.exe`; **starts the portable executable for 25 seconds and requires that it is still running and that its window is titled RepoMemo**; writes one `SHA256SUMS`; and uploads the lot as the artifact `desktop-windows-x64`. It publishes nothing and has read access only.
- **WebView2 handled both ways:** the installer **fetches the runtime when it is missing** (`webviewInstallMode: downloadBootstrapper`, silent, in `tauri.conf.json`), and the **offline installer carries it** (`offlineInstaller`, in `tauri.offline.conf.json`, about 130 MB larger) for machines with no internet.
- `tauri.conf.json` names the icon (`icons/icon.ico`) the installer and the executable use.

### Changed
- **`container:check`** holds the runner, the NSIS build, the offline configuration, the bootstrapper setting, the start-up check, the artifact, the tag rule and that the workflow writes nothing. **The documents:** the development commands (where the artifact is and what is in it), the operations guide, the technical mindmap, the roadmap and the release index. **The version** is `0.4.49` everywhere.

### Checks
- **Run:** `cargo check -p repomemo-desktop` (it reads the new configuration), `container:check`, `version:check`, `docs:check`.
- **Not run:** the workflow, an installer, the offline installer, the app on a clean machine. `beforeBuildCommand` is `npm.cmd run build`, which is Windows-only; V0.4.59 will have to change it for Linux and macOS.

### Upgrading
- Nothing to do.

## V0.4.48 (2026-10-26)

One command from an administrator's keyboard to a file a team can use.

### Added
- **`npm run desktop:package -- --server https://memo.example.com --name "Acme Memo"`** (`scripts/package-desktop.mjs`; also `--support` for a help link and `--open` to leave the server unlocked). It writes, in `dist-desktop/`: `RepoMemo-<name>-portable.zip` (the program as `RepoMemo.exe`, `repomemo-desktop.json` beside it and a readme; the same bytes every time), `RepoMemo-<name>-setup.exe` when an installer exists, the same `repomemo-desktop.json` for IT to place in the config folder, and one `SHA256SUMS`.
- **With Rust:** it builds the app with the address compiled in (`REPOMEMO_PRESET_SERVER`, V0.4.46; `tauri build --bundles nsis`), so the executable alone talks to that server and no other. **Without Rust:** `--from FOLDER` takes `repomemo-desktop.exe` and `*-setup.exe` from the artifact CI uploads and adds the configuration file; nothing is built.
- **The checks of the app, before anything is written:** the address by the rules of a typed one (`https://`, or `http://` on your own network, nothing after the host), the name (one line, up to 60 characters) and the help link (`http(s)`, no login), in the app's own words; a mistake, a missing folder or a folder with no program stops the command with nothing written.
- **`package-desktop.test.mjs`** (seven tests): the configuration for the documented choices is the sample administrators are given; the address rules (the same lists as the Rust tests); the name and the help link; file names; the handout from an artifact (zip with the three files, installer, configuration, sums); the same zip bytes every time; a bad server or an empty folder writes nothing. CI runs it.

### Changed
- **`container:check`** requires the command to compile the address in, to take `--from` and to write the configuration file, the `desktop:package` script, and CI to run the tests. **The documents:** the development commands (*Hand a team the desktop app*, with the Rust and no-Rust ways and what the second does not give), the operations guide, the technical mindmap, the roadmap and the release index. **The version** is `0.4.48` everywhere.

### Checks
- **Run:** `desktop:test` (7 passed), `container:check`, `version:check`, `docs:check`.
- **Not run:** the build mode (it needs Rust and the Tauri CLI and builds an app), an installer, a handout on a clean machine. The artifact CI uploads does not exist until V0.4.49.

### Upgrading
- Nothing to do.

## V0.4.47 (2026-10-26)

A file beside the app says which server it talks to: the same executable with two files talks to two servers.

### Added
- **`repomemo-desktop.json`**, read once at start from the folder of the executable, else the app's config folder (`config.rs`). Four fields and nothing else: `server` (checked by the rules of a typed address), `name` (one line, up to 60 characters; the window's title and the page's), `locked` (default true when there is a server) and `support` (an `http(s)` address, no login). A server in the file that is locked overrides what a person typed and any saved address; unlocked, it only fills in the connect screen's field. An address compiled into the app (V0.4.46) still wins.
- **A bad file is named, not obeyed:** a file that is too long, is not JSON, names a field the app does not know, has a value that fails its rule or a `locked` with no `server` makes the app show the file's path and the reason in place of everything else (`DesktopConfigProblem`, on the same frame as the connect screen) and connect to nothing, not even to the saved server, so typing an address cannot go around it.
- **`deploy/desktop/repomemo-desktop.example.json`**, which a Rust test reads and checks. The page's *Help* link (connect screen, account area) opens in the system browser.
- **Tests:** seven Rust tests in `config.rs` (a valid file, an open server and a byte-order mark, the server's rules, every kind of bad file named, beside-the-executable first, a file read whole or not at all, the example) and two in `connection.rs` (the strongest place decides, a file with a problem starts nothing); `desktopConfig.test.tsx` (six: a bad file replaces everything and nothing connects, the name is the title, an unlocked server is filled in, a locked one opens at once, the help link, a shell that cannot say).

### Changed
- **`start`** picks the server with one function (`choose_start`: the packaged address, then the file's locked server, then the saved one). **`DesktopApp`** reads the settings first; `DesktopServerNote` shows the help link. **The documents:** the development commands (the file, with its places on each system; the heading of the versions section, which V0.4.46 had dropped, is back), the parity audit, the functional and technical mindmaps, the operations guide, the roadmap and the release index. **The version** is `0.4.47` everywhere.

### Checks
- **Run:** `cargo check -p repomemo-desktop --tests`, `cargo clippy -p repomemo-desktop --all-targets -- -D warnings`, `cargo fmt --all`, the desktop page tests (29 passed), `typecheck`, `a11y:check`, `container:check`, `version:check`, `docs:check`.
- **Not run:** the Rust tests (compiled, not run), the app with a file beside it.

### Upgrading
- Nothing to do. Without the file the app behaves as before.

## V0.4.46 (2026-10-26)

An app can be packaged for one server: it opens on that server's sign-in page and cannot be pointed elsewhere.

### Added
- **A preset server:** `REPOMEMO_PRESET_SERVER=https://memo.example.com` when the app is built (`build.rs` reruns when it changes) is compiled into the shell (`option_env!`). It is checked by the rules of a typed address (`https://`, or `http://` on your own network, nothing after the host) and normalized; a bad one stops the app at start with a message that names the address and where it came from.
- **A locked connection:** with a preset, the shell allows that server's address to the window at start, ignores the address saved by an earlier run, and refuses `prepare_server`, `remember_server` and `forget_server` ("This app is set up for ...; its server cannot be changed."). A new command, `server_locked`, tells the page.
- **Tests:** three Rust tests (no preset; a preset is checked like a typed address and the message names it; a locked connection refuses and an open one allows) and `desktopPreset.test.tsx` (five tests: the app opens on the server with no address field and asks the shell nothing; the server note shows the host with no *Change server* when locked, and offers none before the shell has answered; an app without a preset offers it as before, also when the shell cannot say).

### Changed
- **`DesktopServerNote`** asks the shell whether the server is locked and hides *Change server* when it is. **The documents:** the development commands (packaging for one server), the parity audit, the functional and technical mindmaps, the operations guide, the roadmap and the release index. **The version** is `0.4.46` everywhere.

### Checks
- **Run:** `cargo check -p repomemo-desktop --tests`, `cargo clippy -p repomemo-desktop --all-targets -- -D warnings`, `cargo fmt --all`, `desktopPreset.test.tsx` and the desktop app and parity tests (19 passed), `typecheck`, `container:check`, `version:check`, `docs:check`.
- **Not run:** the three Rust tests (compiled, not run), a packaged app, the app against a server.

### Upgrading
- Nothing to do. An app built without the variable behaves as before.

## V0.4.45 (2026-10-26)

The connect screen and the sign-in page are one frame with different contents.

### Added
- **`components/AuthFrame.tsx`:** the frame of a page that asks who you are or which server: the theme toggle, the brand and an introduction with a trust list on one side, the form on the other. Its classes (`shared-auth-canvas`, `-intro`, `-form-region`, `-form-shell`) are written there and nowhere else in the two pages.
- **`connectScreen.test.tsx`** (four tests): both pages use the frame and write none of its classes; both build their form from the web client's field, input, button and toast, with the heading and form classes of the sign-in page; the frame puts the introduction beside the form with the trust list; the connect screen is that frame, with its address field and *Connect* button.

### Changed
- **The sign-in page (`AuthCanvas`) and the desktop connect screen (`DesktopConnect`)** are made of `AuthFrame`; what each adds is its own heading, form and copy. The look did not change: the markup and classes are the same ones as before. The desktop app's own styles are now only the account area's server note (`rm-desktop-server`).
- **The documents:** the parity audit (the first-screen row is closed), the operations guide, the technical mindmap, the roadmap and the release index. **The version** is `0.4.45` everywhere.

### Checks
- **Run:** `connectScreen.test.tsx` (4 passed), the desktop app, set-up pages and accessibility tests (19 passed), `typecheck`, `a11y:check`, `styles:check`, `container:check`, `version:check`, `docs:check`.
- **Not run:** the app, a look at both screens in the browser and the window. The other pages that write the same classes (invitation, password reset, e-mail check, set-up) were left as they are.

### Upgrading
- Nothing to do.

## V0.4.44 (2026-10-26)

The client sends every request through one transport, and no component asks which window it is in.

### Added
- **`lib/transport.ts`, the one seam:** the browser's own `fetch` (`browserTransport`, looked up when called) and the shell's client (`shellTransport`: `tauri-plugin-http` with `maxRedirections: 0`, loaded only here). `sharedApi` sends every request, including its refreshes, logouts and the live event stream, through the transport it was given and uses no other `fetch`.
- **`lib/platform.ts`:** what the window can do, as capabilities (`opensTabs`, `choosesServer`, `installedApp`). A component asks for a capability and never which runtime it is in.
- **`lib/desktopConnection.ts`:** `pointAtServer`, which gives the client the shell's transport for the server the shell allowed (it was in the connect screen).
- **`transport.test.ts`** (four tests): the browser's transport is the page's `fetch`; the shell's passes `maxRedirections: 0`; a web client sends through the page's `fetch` and never the shell's, and once pointed at a server the app sends through the shell's and never the page's; the capabilities for the app and for a browser.

### Changed
- **No desktop branch inside a component:** `DocumentViewer`, `DesktopServerNote` and `SharedWebApp` ask `platform`; the entry (`App`, `main`) asks `isDesktopRuntime`, which is the only place the runtime is detected (`__TAURI_INTERNALS__` is in `lib/desktop.ts` alone). `DesktopConnect` no longer builds a client.
- **`desktopParity.test.ts`** (now seven tests) refuses the runtime test, the shell's plugin or a call of `fetch` anywhere but those places, and the parity page's list of files that know about the desktop is down from nine to seven. The rows for the transport and the version note are closed in the audit.
- **The documents:** the parity audit, the operations guide, the technical mindmap, the roadmap and the release index. **The version** is `0.4.44` everywhere.

### Checks
- **Run:** `transport.test.ts` (4 passed), `desktopParity.test.ts` (7 passed), `desktopApp` and `desktopSession` tests (13 passed), `typecheck`, `container:check`, `version:check`, `docs:check`; the whole web suite once: 9 tests fail in five files this release does not touch (`workspaceImport`, `templatePicker`, `templateAssistant`, `narrowScreens`, `reviewQueue`), the same 9 on the commit before.
- **Not run:** the app against a server; the desktop crate.

### Upgrading
- Nothing to do.

## V0.4.43 (2026-10-26)

Every difference between the desktop app and the web client is written down, with the release that closes it or the reason it stays.

### Added
- **`docs/DESKTOP_PARITY.md`:** the parity audit. A table of fourteen differences (transport, which server, the first screen, downloads, opening a PDF or an Office file, links to other sites, links into the app, notifications, shortcuts, the window, the version note, the server note, updates, a private certificate authority, installing), each with what the browser does, what the desktop does and either *Closed by V0.4.N* or *Stays:* with the reason. Under it, the list of files that know about the desktop (nine).
- **`desktopParity.test.ts`** (six tests): the page has one script, `src/main.tsx`, and there is no other page; the build has no second input and the shell serves the one `../dist` with no window of its own page; the client is mounted in one place (`createRoot` once) and `App` chooses between exactly `DesktopApp` and `SharedWebApp`, which the desktop start renders; the audit names the six areas (transport, downloads, links, notifications, shortcuts, window); every row says which release or why; and the files that branch on the desktop runtime or call the shell are exactly the ones the page lists, so a new desktop branch fails the test until it is written down.

### Changed
- **The documents:** the operations guide, the technical mindmap, the roadmap and the release index. **The version** is `0.4.43` everywhere. No application code changed.

### Checks
- **Run:** `desktopParity.test.ts` (6 passed), `typecheck`, `container:check`, `version:check`, `docs:check`.
- **Not run:** the rest of the web tests, the desktop crate, the app.

### Upgrading
- Nothing to do.

## V0.4.42 (2026-10-26)

Nothing the release pipeline runs can move under it: every workflow, every image.

### Added
- **`scripts/pins.mjs`** and **`scripts/pins.test.mjs`** (`npm run pins:test`): the pinning rules as functions with five tests. An action is a 40-character commit (a tag, a branch or no reference is refused; a local `./` action is fine); a Dockerfile `FROM` names a digest (an earlier stage and `scratch` need none, and a `--platform` flag is not mistaken for the image); and **an image pinned by digest anywhere in the project is refused when it is named with a tag and no digest anywhere else**, in a workflow, a compose file, a script or the Dockerfile, so a new `docker run caddy:2-alpine` in a new workflow is caught without anyone adding it to a list.
- **Dependabot watches the compose files' images** (`docker-compose`, from the root) beside the Dockerfile's base images and the actions; the actions arrive as one grouped pull request so the three workflows never disagree on `checkout`.

### Changed
- **`container:check`** runs the rules over every workflow found in `.github/workflows` (the new ones included), the Dockerfile, both compose files, the scripts in `deploy/`, the service files, the monitoring example and `.env.example`; it also requires Dependabot to watch Actions, the Dockerfile and the compose files from the root, and CI to run the pinning tests. It found nothing unpinned in the project as it stands. **The documents:** the operations guide, the technical mindmap, the roadmap and the release index. **The version** is `0.4.42` everywhere.

### Checks
- **Run:** `pins:test` (5 passed), `container:check` (also tried with an unpinned action and an unpinned image added: both refused), `version:check`, `docs:check`.
- **Not run:** Dependabot itself (it reads the file on GitHub; the `docker-compose` ecosystem and the group are its documented options).

### Upgrading
- Nothing to do.

## V0.4.41 (2026-10-26)

A clean machine with Docker goes from nothing to a running server with one fetched script and the init.

### Added
- **`deploy/quickstart.sh`:** finds the latest release (or the one given with `--version`), downloads `SHA256SUMS` and `repomemo-compose-<version>.tar.gz` from the release page, **checks the archive against the sums before it unpacks anything** (a mismatch, or a sums file that does not list the archive, stops with nothing written), unpacks it into an empty folder and pins `REPOMEMO_VERSION` in the unpacked `.env.example` to the release it fetched. It starts nothing. `--repo` and `--base-url` serve a mirror.
- **`deploy/package-compose.sh`** and **`deploy/compose-bundle.txt`:** the archive of the container files (compose file and its build file, `.env.example`, the init, upgrade, backup, restore and migrate scripts, the proxy and monitoring samples, the quick start page), the same bytes every time (sorted names, no owner, the commit's time, gzip without a name).
- **`docs/QUICK_START.md`:** the one command, the careful way (download, check, read, run), then the init, the first start and what to read next.
- **CI's `quickstart` job:** packs the files twice and compares them, fetches them with the script from a local release folder, runs the init in the result, reads the stack it describes (the released images, tag `9.9.9`), and tampers with the archive to see that nothing is unpacked.

### Changed
- **The release** (the archives workflow) packs the container files, copies the script, and writes one `SHA256SUMS` over the three server archives, the container files and the script; all are attached. **The release page** has a *Quick start* section with the command for that version.
- **`container:check`** holds the list (every file exists, the compose file's bind mounts are in it), the check before the unpack, the pin, the attached files and the CI job. **The documents:** the operations guide, the technical mindmap, the roadmap and the release index. **The version** is `0.4.41` everywhere.

### Checks
- **Run:** `container:check`, `version:check`, `docs:check`, `notes:test` (4 passed), shell syntax; the packer and the script by hand against a local release folder (checksum ok, unpacked, pinned to 9.9.9, the same bytes on a second pack; a tampered archive refused with nothing unpacked).
- **Not run:** the CI job, the release job, the script against github.com (no release exists yet), the init and the stack on the unpacked files. Windows has no script of its own: it unpacks the archive by hand and uses `init.ps1`.

### Upgrading
- Nothing to do.

## V0.4.40 (2026-10-26)

The compose file runs a released version: pull, then up, with no compiler.

### Changed
- **`docker-compose.yml` pulls images and builds nothing:** the four services name `ghcr.io/repomemo/repomemo-<name>:${REPOMEMO_VERSION:-latest}` (the registry is `REPOMEMO_REGISTRY`, so a mirror of your own is one setting; `REPOMEMO_CONVERTER_IMAGE` still wins for the converter). `REPOMEMO_VERSION` in `.env.example` is now `latest`, with the advice to pin a release; it was `local`, the tag of images built on the machine.
- **`deploy/upgrade.sh`** pulls the released images by default and builds only when `COMPOSE_FILE` (in the environment or in the env file) names the build file; `--build` and `--no-build` still force either. `deploy/init.sh` and `init.ps1` print `docker compose pull && docker compose up -d --wait`.
- **`container:check`** reads the build instructions from the build file, and refuses a build, a moving name or a missing registry setting in the compose file. **The documents:** the development commands, the upgrade notes, the operations guide, the technical mindmap, the roadmap and the release index. **The version** is `0.4.40` everywhere.

### Added
- **`docker-compose.build.yml`:** the one place a build is described (the four stages, the commit, date and tag as build arguments, local tags `repomemo-<name>:${REPOMEMO_VERSION:-local}`). `docker compose -f docker-compose.yml -f docker-compose.build.yml up -d --build --wait` builds this checkout; `COMPOSE_FILE` in `.env` does it for good.
- **`scripts/check-compose-images.mjs`** and a CI job, **Compose runs released images:** `docker compose config` of the compose file alone names the four released images with the tag asked for and has no build; another registry changes only the prefix; with the build file every service builds. The other CI jobs set `COMPOSE_FILE` to build the commit they test.
- **The release page** (V0.4.39) tells how to run the release: set the tag, pull, up.

### Checks
- **Run:** `container:check`, `version:check`, `docs:check`, `docker compose config` on the compose file alone and with the build file (read by `check-compose-images.mjs`: both passed), shell syntax of the two scripts.
- **Not run:** the stack, a pull (no image exists on the registry until the first tag), `deploy/upgrade.sh` and the CI jobs. `deploy/api.yml` (the API alone) still builds.

### Upgrading
- **A container install built from a checkout:** put `COMPOSE_FILE=docker-compose.yml:docker-compose.build.yml` in `.env` (on Windows `;` and `COMPOSE_PATH_SEPARATOR=;`) to keep building, or set `REPOMEMO_VERSION` to a release and run `docker compose pull && docker compose up -d` to run the released images. A `.env` that still says `REPOMEMO_VERSION=local` would try to pull a tag that does not exist: change it.

## V0.4.39 (2026-10-26)

The page of a release is the changelog's, with nothing typed by hand.

### Added
- **`scripts/release-notes.mjs`** (`npm run notes`): prints the changelog entry of a version (the one in `package.json` by default), then what the release holds: the four images on GHCR with their three names, the three server archives and `SHA256SUMS`, the Linux and Windows installers inside the archives, the upgrade guide and the page on verifying an image. It refuses a version with no entry or an empty one.
- **`scripts/release-notes.test.mjs`** (`npm run notes:test`): four tests (the entry is cut at the next release and CRLF is dropped, a missing or empty entry is refused, the page names every image, archive, sums file and installer, the real changelog has an entry for the current version); CI runs it with the other checks.

### Changed
- **The archives workflow** writes the page with the script and makes the release with `--notes-file`; when the release already exists (a re-run) it rewrites the title and the page the same way, so the page is always the changelog's. The sentence it used to carry is gone.
- **`container:check`** refuses a release page typed into the workflow, requires the re-run rewrite, the script's list and the CI test. **The documents:** the development commands, the operations guide, the technical mindmap, the roadmap and the release index. **The version** is `0.4.39` everywhere.

### Checks
- **Run:** `notes:test` (4 passed), `container:check`, `version:check`, `docs:check`.
- **Not run:** the workflow; no release page has been made. The images workflow (V0.4.37) does not touch the page, so the page lists the images even if their job fails.

### Upgrading
- Nothing to do.

## V0.4.38 (2026-10-26)

Each published image says who built it and what is in it, and anyone can check.

### Added
- **A signed build attestation per image:** after the two architectures are joined, the `release-images` workflow signs a SLSA provenance statement for the joined image's digest (`actions/attest-build-provenance`, keyless, with GitHub's workflow identity and Sigstore) and keeps it with the image on the registry. It names the repository, the workflow file, the commit and the tag.
- **A signed SBOM per image:** the scanner already used by CI (Trivy, pinned by digest) reads the pushed image and writes an SPDX document, which `actions/attest-sbom` signs and keeps with the image.
- **`docs/VERIFYING_IMAGES.md`:** how to verify an image with `gh attestation verify` (and `--predicate-type https://spdx.dev/Spdx` to read the SBOM), the same with `cosign verify-attestation`, what a valid attestation does and does not say, and to deploy by digest.

### Changed
- **Keyless signing is the signing:** the image itself is not signed with a second tool; the signed statements are bound to its digest. Only the join job has `id-token: write` and `attestations: write`.
- **`container:check`** holds the two pinned attestation actions, the registry copy of both, the scanner pinned by digest, the sole signing job and the guide's commands. **The documents:** the operations guide, the technical mindmap, the roadmap and the release index. **The version** is `0.4.38` everywhere.

### Checks
- **Run:** `container:check`, `version:check`, `docs:check`.
- **Not run:** the workflow, so no attestation has been made or verified. Attestations to a private repository's packages need a plan that supports them; the first tag shows it.

### Upgrading
- Nothing to do.

## V0.4.37 (2026-10-26)

A tag publishes the images to the registry.

### Added
- **`.github/workflows/release-images.yml`:** on a `v*` tag, and by hand (which builds and pushes nothing), never on a pull request or a push to a branch. A first job checks that the tag is `v` plus the version and runs the version, container, migration, documentation and template checks. Then the **four images** (server, converter, edge and web; the plan said three, but the Dockerfile has four stages that carry the labels) are built **natively** for `amd64` (`ubuntu-latest`) and `arm64` (`ubuntu-24.04-arm`), with the commit, date and version as build arguments, and pushed to `ghcr.io/<owner>/repomemo-<name>` under an architecture tag.
- **One name for both:** a second job per image, only on a tag, joins the two into one multi-architecture image named **`X.Y.Z`, `X.Y` and `latest`** (`docker buildx imagetools create`) and checks that both `linux/amd64` and `linux/arm64` are in it. The two architecture tags stay on the registry because the joined image refers to them.

### Changed
- **`container:check`** holds the tag rule, the five checks that run first, the two architectures on native runners, the four images, the three names, push on a tag only and `packages: write` in the two jobs that push and nowhere else. **The documents:** the operations guide, the technical mindmap, the roadmap and the release index. **The version** is `0.4.37` everywhere.

### Checks
- **Run:** `container:check`, `version:check`, `docs:check`.
- **Not run:** the workflow (a manual run from the Actions page builds the eight images and pushes none; pushing and naming happen only on the first tag). The package visibility on GHCR is the maintainer's to set after the first push.

### Upgrading
- Nothing to do.

## V0.4.36 (2026-10-25)

An older server refuses a database from a newer release, and a guide to upgrade and roll back.

### Added
- **A database from a newer release is refused, not opened.** Before it changes anything, the storage engine reads the file `repomemo.version` beside the database (the release that last opened it; every start leaves it and it never goes backwards) and the highest migration the database has applied. A marker newer than the program, or a migration the program does not know, stops the start with a message that **names both releases** and says nothing was changed. A database with no marker (made before this release) or a marker that is equal or older opens as before. Backup restores into a new environment are unaffected.
- **`docs/DIRECT_UPGRADE.md`:** the guide for direct installs. **It begins with a backup** (and says why it is the only way back across an upgrade), then upgrading with the installer of V0.4.32–V0.4.33 (check the archive, unpack in a new folder, run the installer, look), rolling back by moving the link or junction, what the refusal means when the old release will not start (restore the backup; what was written after it is in the folder moved aside), and what not to do (unpack over the running release, delete the marker, run two releases on one data folder).

### Changed
- **`container:check`** holds the guide's order and contents and that the refusal still comes before the migrations and names both versions. **The documents:** the operations guide, the technical mindmap, the roadmap and the release index. **The version** is `0.4.36` everywhere.

### Checks
- **Compiled and linted:** `cargo clippy -D warnings` over the storage, api and server crates with their tests, `cargo fmt --all --check`, `container:check`, `version:check`, `docs:check`.
- **Tests written, not run:** four unit tests of the rule (a newer marker, the ordinary cases, an unknown migration, a marker that never goes back) and a storage test that makes a database, sets a newer marker, sees the refusal name both versions with the database file unchanged, then removes the marker, adds a migration from the future and sees that refused too.
- **Not run:** an older binary on a newer database by hand (the release's own check).

### Upgrading
- Nothing to do. From now on, going back to an older release across a release that changed the database needs the backup; the server says so instead of damaging the data.

## V0.4.35 (2026-10-25)

The server is built for three platforms and attached to the release.

### Added
- **`.github/workflows/server-archives.yml`:** on a `v*` tag, and by hand (which only keeps the archives as artifacts), never on a pull request. For **Linux x64, Linux arm64 and Windows x64**, on native runners (no cross-compiling or emulation), it checks that the tag is `v` plus the version, builds the web app for the page's own host and the server, checks that the binary is for that platform, and packs it with `scripts/package-server.mjs` (V0.4.31), the commit's date being the time recorded.
- **Attaching to the release:** a second job, the only one with write access and only on a tag, downloads the three archives, requires exactly three, writes **one `SHA256SUMS`** over them and checks it, then attaches the archives and the sums to the GitHub release, making the release first when it is not there (existing files are replaced, so a re-run is safe). Its text is the changelog's from V0.4.39.

### Changed
- **`container:check`** holds the three platforms and runners, the tag rule, the tag-only release job with the only write permission and the checked sums. **The documents:** the operations guide, the technical mindmap, the roadmap and the release index. **The version** is `0.4.35` everywhere.

### Checks
- **Run:** `container:check`, `version:check`, `docs:check`.
- **Not run:** the workflow (it runs on a tag, or by hand from the Actions page: a manual run is the way to try it before a tag). The release step has never run; the first tag is its test.

### Upgrading
- Nothing to do.

## V0.4.34 (2026-10-25)

One page from a direct install to a working https address.

### Added
- **`docs/FRONT_DOORS.md`:** the one page for a direct install: the three server settings that go with any front door (`REPOMEMO_PUBLIC_URL`, `REPOMEMO_ALLOWED_ORIGIN`, `REPOMEMO_TRUST_PROXY`), then Caddy (an automatic certificate), nginx with certbot, or the server alone on a trusted network; Windows (Caddy for Windows, or IIS with response buffering off); the firewall for ufw, firewalld and Windows, with a note not to open 3020; and a check with `curl` and the journal. Because the server serves the web app itself (V0.4.20) and listens on the loopback, one proxy rule is enough.
- **`deploy/direct/Caddyfile` and `deploy/direct/nginx.conf`:** the two samples the page uses, for the server on `127.0.0.1:3020`: compression, HSTS, long uploads, the event streams unbuffered.
- **A CI job, `direct-front-doors`,** has Caddy validate the Caddyfile and nginx test the nginx file (a throwaway certificate; both images pinned by digest).

### Changed
- **`container:check`** holds the page's contents, what the two samples must do and the CI job. **The documents:** the operations guide, the technical mindmap, the roadmap and the release index. **The version** is `0.4.34` everywhere.

### Checks
- **Run:** `container:check`, `version:check`, `docs:check` (the page's links).
- **Not run:** Caddy or nginx with the files (CI does); a person following the page on a clean machine, which is the release's real test.

### Upgrading
- Nothing to do.

## V0.4.33 (2026-10-25)

Windows: the server runs as a service.

### Added
- **`repomemo-server service install | uninstall | run`:** the server registers itself as a Windows service. `install` registers it with the service manager to start when Windows does, as `NT AUTHORITY\LocalService` (`--account` changes it), with the working folder and the settings file it was given, and sets it to restart after a failure (5 s, 5 s, 30 s); `uninstall` stops and removes it; `run` is what the manager starts: it answers the manager, goes to the working folder, reads the settings file (`NAME=value` lines, comments and blank lines skipped, a BOM and a pair of quotes tolerated) into the environment, and runs the server until Stop or a Windows shutdown, then stops it cleanly (V0.4.6). It uses the `windows-service` crate on Windows only (MIT or Apache-2.0; `cargo deny` licenses and bans pass); on any other system the command says to use systemd.
- **`runner.rs`:** what `main` did to start and stop the server is now `run_server(shutdown)`, shared by `main` and the service, which returns an exit code instead of exiting from inside.
- **`service/install.ps1` and `service/uninstall.ps1`** (in the Windows archive): the PowerShell installer, safe to run twice, and its removal that never touches the data (see the operations guide).
- **A CI job, `server-archive-windows`,** builds the Windows archive on a Windows runner, installs it with the script, checks that the service runs and starts at boot (`Automatic`), asks `/health/ready` and the page, runs the installer again and compares the settings and the junction, stops the service within a minute, removes it and checks the settings stay. The archive is kept as an artifact.

### Changed
- **`container:check`** holds the Windows service's pieces (the installer's safeguards, the uninstaller not deleting the data, the service code's essentials, the Windows-only dependency and the CI job).
- **The documents:** the operations guide, the technical mindmap, the roadmap and the release index.
- **The version** is `0.4.33` everywhere.

### Checks
- **Compiled and linted** (on Windows, so the service code itself was compiled): `cargo clippy -D warnings` over the server with its tests, `cargo fmt --all --check`, `cargo deny check bans licenses`, the PowerShell parser on the three PowerShell scripts (no errors; none was run), `container:check`, `version:check`, `docs:check`.
- **Tests written, not run:** three unit tests (the command line, its mistakes, the env file).
- **Not run:** the service, the installer or the uninstaller. The `server-archive-windows` job is the test, and **it is the first time the code talks to the service manager**; a rejected account, access list or start type will show there. Reboot behaviour is not tested (only that the start type is automatic).

### Upgrading
- Nothing to do. A new install: unpack the zip and run `.\service\install.ps1 -GenerateSecrets -Start` as an administrator.

## V0.4.32 (2026-10-25)

Linux: a hardened systemd unit and an installer that is safe to run twice.

### Added
- **`service/repomemo.service`** (in the archive; `deploy/direct/service/` in the source): the server as a systemd service, run as the user `repomemo` with no capabilities and no new privileges, `ProtectSystem=strict` with only `/var/lib/repomemo` writable, private `/tmp` and devices, the kernel's tunables, modules, logs, control groups, clock and host name closed to it, only the `AF_UNIX`, `AF_INET` and `AF_INET6` address families, no namespaces, no real-time scheduling, no setuid files, no writes to executable memory, native system calls only and the `@system-service` set. It reads `/etc/repomemo/repomemo.env`, runs `/opt/repomemo/bin/repomemo-server` from `/var/lib/repomemo`, restarts on failure, sends SIGTERM and waits 45 seconds (the server's own grace is 30, V0.4.6).
- **`service/install.sh`:** run as root from the unpacked archive. It makes the `repomemo` user and group (no login) and `/var/lib/repomemo` and `/etc/repomemo`; copies the release to `/opt/repomemo-<version>` and points the link `/opt/repomemo` at it, so a new release is a new link and the old one stays (V0.4.36 uses it); makes `/etc/repomemo/repomemo.env` from the example **only if it is absent** (mode 0600, root), with new secrets under `--generate-secrets`; installs and enables the unit; `--start` starts it, and refuses while a secret is empty. **It is safe to run twice:** every step checks first, an existing settings file is never touched, a release folder that differs is never overwritten, and a link that is not a link is refused.
- **CI** (`server-archive`, V0.4.31) now also installs the unpacked archive as a service on its runner, runs `systemd-analyze verify` on the unit, waits for `/health/ready`, asks the page, checks the user, the file mode and that the process runs as `repomemo`, runs the installer a second time and compares the files and the link before and after, and stops the service cleanly.

### Changed
- **`container:check`** holds the unit's hardening, a stop time longer than the server's grace, what makes the installer safe to run twice, the settings example's web folder under the link, and the CI steps.
- **The documents:** the operations guide, the technical mindmap, the roadmap and the release index.
- **The version** is `0.4.32` everywhere.

### Checks
- **Run:** a syntax check of the installer, `container:check`, `version:check`, `docs:check`, the packager's tests.
- **Not run:** the installer, the unit under systemd or `systemd-analyze` (this machine runs neither). The hardening is the part most likely to need a first-run correction: a call the server makes that `@system-service` or `MemoryDenyWriteExecute` forbids shows in the journal, and CI's job is where it will show.

### Upgrading
- Nothing to do. A new install: unpack, `sudo sh service/install.sh --generate-secrets --start`.

## V0.4.31 (2026-10-25)

The server as a release archive, for a plain server.

### Added
- **`scripts/package-server.mjs`** (`npm run package:server`): packs the server for a direct install. The folder `repomemo-server-<version>-<os>-<arch>` holds `bin/` (the server, `repomemo-server.exe` on Windows), `web/` (the built web app), `config/repomemo.env.example`, `service/` (for the unit and scripts of V0.4.32–V0.4.33), `README.txt`, `VERSION` and `SHA256SUMS` (the checksum of every file in it). **The layout is the same on every system.** It is a `.tar.gz` for Linux (binary mode 0755) and a `.zip` for Windows, both **written by the script itself** (no archive tool is needed), in a fixed order and with a fixed time (`SOURCE_DATE_EPOCH`), so the same input makes the same archive. `<out>/SHA256SUMS` names each archive once.
- **`deploy/direct/repomemo.env.example`:** the settings of a direct install: the address, the public address and origin, trust of the front door, the data folder, `REPOMEMO_WEB_DIR` (V0.4.20), and the two secrets (as variables or as files).
- **`scripts/package-server.test.mjs`** (`npm run package:test`): five tests of the packager (the Linux layout and modes, the Windows zip, the inner checksums, reproducibility and the outer checksum file, the refusals). They read the archives back with readers of their own.
- **A CI job, `server-archive`,** builds the web app (for the page's own host) and the server, packs them, unpacks the archive in a clean folder, checks both checksum files, starts the server from it on the web app inside it and asks the page, `/config.json` and `/health`. The archive is kept as an artifact.

### Changed
- **`container:check`** holds the packager's contents, the settings example (only variables the server reads) and the CI job.
- **The documents:** the operations guide, the technical mindmap, the roadmap and the release index.
- **The version** is `0.4.31` everywhere.

### Checks
- **Run:** the packager's five tests (they pass), a real run of the packager on stand-in files with the system `tar`, `unzip` and `sha256sum -c` reading its archives, `container:check`, `version:check`, `docs:check`.
- **Not run:** a real build of the server or the web app, and the unpacked server started on a clean machine (the `server-archive` job is that test).

### Upgrading
- Nothing to do.

## V0.4.30 (2026-10-25)

An example of reading the server's metrics in containers.

### Added
- **A `monitoring` profile with an example Prometheus:** `docker compose --profile monitoring up -d` starts Prometheus (v3.8.1, pinned by tag and digest, read-only, every capability dropped, published on `127.0.0.1:9090` only, data in the `prometheus-data` volume, health-checked) that scrapes `/v1/system/metrics` every 15 seconds with a personal access token of the `system` scope. The token is a file, `deploy/monitoring/metrics-token` (`REPOMEMO_METRICS_TOKEN_FILE` names another), mounted read-only; `.gitignore` keeps it out of the repository. **It is never on by default.**
- **`deploy/monitoring/prometheus.yml`** is the scrape block, written to be copied into a Prometheus of one's own (target `server:3020` on the compose network), and **`deploy/monitoring/README.md`** walks through the token, the profile and a first query.
- **A CI job, `monitoring`,** starts a stack without the profile (and checks Prometheus is not among its services), makes an administrator and a `system` token through the API, starts Prometheus with the profile and waits until `repomemo_users` returns a value.

### Changed
- **`container:check`** holds the profile, the pin, the loopback port, the read-only mounts, the scrape block, the `.gitignore` line, the CI job and that the server still has the metric the example reads.
- **The documents:** the operations guide, the technical mindmap, the roadmap and the release index.
- **The version** is `0.4.30` everywhere.

### Checks
- **Run:** `container:check`, `version:check`, `docs:check`.
- **Not run:** Prometheus or the stack. The `monitoring` job is the test; its first run may name a response shape of the setup or token calls that differs from what was read in the code.

### Upgrading
- Nothing to do. The example does not start unless asked.

## V0.4.29 (2026-10-25)

In containers the server logs JSON to standard output, and every line parses.

### Changed
- **The compose files make the server log JSON and keep no log files:** `REPOMEMO_LOG_FORMAT=json` and `REPOMEMO_LOG_TO_FILE=false` in `docker-compose.yml` and `deploy/api.yml`. The server writes one JSON object per line to standard output (`timestamp`, `level`, `target`, `message`, `fields`), which was already possible (`REPOMEMO_LOG_FORMAT`) and is now the default in containers; the daily files in the data folder are off there, and `System > Logs` shows the in-memory buffer.
- **Every line parses.** What was written outside the logging now has the same shape under `json`: a refused configuration, a data folder that cannot be used, a start that failed (`stderr_line` in `logs.rs`, used by `main.rs`) and the entrypoint's lines when it gives a volume to its user or cannot (`say` in `docker/entrypoint.sh`, quotes and backslashes escaped).
- **The audit target is kept apart** by its `target`: audit events carry `"target":"audit"` in the same stream, so a collector separates them with a filter (an example is in the operations guide); they remain in the database too.
- **CI** checks that every line of the server's log is a JSON object with a timestamp, a level, a target and a message. **`container:check`** holds the two settings in both compose files, the entrypoint's JSON and that CI step.
- **The documents:** the settings reference (`REPOMEMO_LOG_FORMAT`), the operations guide, the technical mindmap, the roadmap and the release index.
- **The version** is `0.4.29` everywhere.

### Checks
- **Compiled and linted:** `cargo clippy -D warnings` over the server with its tests, `cargo fmt --all --check`, `container:check`, `version:check`, `docs:check`, a syntax check of the entrypoint and a try of its `say` with quotes and a backslash (the output parses).
- **Tests written, not run:** one unit test of the JSON line (level, message with quotes, a timestamp ending in `Z`, the target) and of the plain form.
- **Not run:** the stack. Caddy's lines are not the server's and are not part of the check.

### Upgrading
- Nothing to do. Anyone who read the container's logs as text sees JSON now; set `REPOMEMO_LOG_FORMAT=text` in the environment of the service to go back.

## V0.4.28 (2026-10-25)

Every service says it is healthy, and the stack waits for all of them.

### Changed
- **Health checks for every service:** the server (`repomemo-server --health`) and the web image (`/healthz`) had theirs in the image; the edge now reports healthy when something listens on port 443 (read from `/proc/net`, since the image has no tool for it), and the converter when its socket exists. `docker compose up --wait` therefore returns only when the whole stack is healthy; the web image waits for the server and the edge for the web image.
- **Time to stop:** `stop_grace_period` is 45 seconds for the server (longer than the 30 it gives running requests, V0.4.6) and 10 for the others, in `docker-compose.yml` and `deploy/api.yml`.
- **Log rotation:** every service logs to Docker's `json-file` driver, five files of ten megabytes each, through one `x-logging` anchor.
- **Limits are documented, not set:** the operations guide gives starting figures for memory and says what to add under a service to cap it; the compose file points there.
- **CI** reads the health of every service after `--wait` and fails when one is not `healthy`. **`container:check`** insists on a health check (in the file or the image), the restart policy, the grace period, the rotation and the dependencies' conditions for each service of both compose files, and on a server grace longer than 30 seconds.
- **The documents:** the operations guide (*Healthy, patient and quiet*), the technical mindmap, the roadmap and the release index.
- **The version** is `0.4.28` everywhere.

### Checks
- **Run:** `container:check`, `version:check`, `docs:check`.
- **Not run:** the stack. The edge's `/proc/net` check and the converter's `test -S` are the first to read in CI's output.

### Upgrading
- Nothing to do.

## V0.4.27 (2026-10-25)

The data of the previous release is checked through this one.

### Added
- **An upgrade test in CI, `upgrade-from-previous`:** the release named in `deploy/upgrade-from.txt` (V0.3.27, by commit; until images are published it is built from source) is built and run on a single volume as it ran for people. `scripts/upgrade-test.mjs seed` makes its first administrator, a workspace and an uploaded note and waits until the note is searchable. The container is stopped, `deploy/migrate-volumes.sh` moves the volume into the three of this release, the compose stack starts on them, and `scripts/upgrade-test.mjs verify` signs the same administrator in and finds the workspace, the note in its list, its file read back and its text by search. **A migration that loses data fails the job.**
- **`scripts/upgrade-test.mjs`** uses only API calls that exist in every release since 0.3.0, so the same script runs against both.
- **`deploy/upgrade-from.txt`** names the release to start from, with a note to move it forward when a release is published.

### Changed
- **`container:check`** insists on the job's steps, on a commit in the file and on the script's calls.
- **The documents:** the operations guide, the technical mindmap, the roadmap and the release index.
- **The version** is `0.4.27` everywhere.

### Checks
- **Run:** a syntax check of the script, `container:check`, `version:check`, `docs:check`.
- **Not run:** the job. The earlier release is built and run only in CI, the only place with both images; its first run may name an API shape of 0.3.27 that differs from what was read (the setup completion, the list of organizations).

### Upgrading
- Nothing to do.

## V0.4.26 (2026-10-25)

CI uses the stack: the smoke test runs over the containers.

### Added
- **`scripts/smoke-test.mjs` does the first hour.** With `REPOMEMO_SMOKE_SETUP_CODE` (the new server's `REPOMEMO_SETUP_CODE`) it runs the first-run setup (the administrator, then its completion), makes a workspace, uploads a Markdown note, waits for the indexing jobs, searches for text from the note until it is found, makes a backup and waits for it, exports the workspace as an archive and imports that archive as a new workspace. Every step is a check; any failure makes the script exit 1. Without the variable it is the read-only check it was.
- **The compose job of CI runs it** against the stack through the edge (TLS, the proxy's own certificate for `localhost`), after the stack is healthy and `/health/ready` answers: first-run setup through the API, a workspace, an upload, indexing, search, a backup, an export and an import, all in the containers.

### Changed
- **`container:check`** insists on the CI step and its setup code, on the script using each step, and on the server still having each of those routes.
- **The documents:** the operations guide, the technical mindmap, the roadmap and the release index.
- **The version** is `0.4.26` everywhere.

### Checks
- **Run:** a syntax check of the script, `container:check`, `version:check`, `docs:check`.
- **Not run:** the script against a server or the stack. **It is the first time these steps are chained,** so the first CI run may find a field name or a status code that differs from what was read in the code (the organization of a new administrator, the shape of the jobs list, the search body); fix what it names.

### Upgrading
- Nothing to do. The script changes data: use it on a test stack only.

## V0.4.25 (2026-10-25)

Backups and restores in containers, written as scripts and tried in CI.

### Added
- **`deploy/backup-volumes.sh`:** copies the volumes of a container install into archives to keep elsewhere. By default the server's own backups (`repomemo-backups`) and `repomemo-data` (the key file, the logs), which is safe while the stack runs; with `--all` also the database and the stored files, **with the stack stopped** meanwhile and started again after. A `backup.txt` says when and from what; a folder that has one is never overwritten. `--project` names another compose project.
- **`deploy/restore-volumes.sh`:** fills the volumes of a stack from such a folder, **empty volumes only**: a volume that holds anything or that a container uses is refused, so a restore can never mix two installs or overwrite a live one. It deletes nothing.
- **A CI job, `volume-restore`,** makes a stack with an account, copies its volumes with `--all`, checks a second copy into the same folder is refused, stops the stack, restores into the volumes of a second stack, starts it, signs the same account in, and checks that a restore over the now filled volumes is refused.

### Changed
- **The helper image of the volume scripts** (`alpine:3.20`) is pinned by digest, in the three scripts (`migrate-volumes.sh` too); `container:check` insists on it.
- **`container:check`** holds the volumes the scripts know, the stop and start, no overwriting, the refusals, no deletion and the CI job.
- **The documents:** the operations guide (why the console needs no container: it is an HTTP client), the technical mindmap, the roadmap and the release index.
- **The version** is `0.4.25` everywhere.

### Checks
- **Run:** a syntax check of the scripts, `container:check`, `version:check`, `docs:check`.
- **Not run:** the scripts against volumes (the `volume-restore` job is the test), the server's own backup through the API from a script (V0.4.24's script does it), and a sealed backup restored in a container.

### Upgrading
- Nothing to do.

## V0.4.24 (2026-10-25)

An upgrade script that comes back to the old version when the new one fails.

### Added
- **`deploy/upgrade.sh --version TAG`:** upgrades a container install in four steps. (1) **A backup through the API**: a personal access token of the `system` scope (`REPOMEMO_UPGRADE_TOKEN` or `--token-file`) makes the server back itself up, and the script waits for it to succeed; without a token it stops unless `--no-backup` says there is a backup of one's own. (2) Only then **`REPOMEMO_VERSION` in `.env`** is set to the tag. (3) The images are pulled where they can be (`--ignore-buildable`) and the stack is restarted with `docker compose up --wait`; then `/health/ready` is asked at the web image's loopback port. (4) If the new version does not turn healthy, the script prints the last log lines, **puts the old tag back in `.env` and starts the old images again** (no build), and exits 1.
- **It never deletes a volume,** and changes nothing before the backup is made. `--no-build` runs only images that exist or can be pulled, which is how a released tag is run once V0.4.40 publishes them; `--env-file`, `--url` and `--timeout` adjust it.
- **A CI job, `upgrade-script`,** installs on a tag `ci1`, runs the script with a tag that does not exist (it must exit 1 and leave `ci1` running, `/health` saying so through its `image`), runs a good one (`ci2`), and checks that without a token or `--no-backup` it stops before touching `.env`.

### Changed
- **`container:check`** holds the backup through the API, the wait, the way back, no deletion, and that the backup comes before the change of `.env`.
- **The documents:** the operations guide, the technical mindmap, the roadmap and the release index.
- **The version** is `0.4.24` everywhere.

### Checks
- **Run:** a syntax check and the argument handling of the script (no tag, a bad tag, `--help`), `container:check`, `version:check`, `docs:check`.
- **Not run:** the script against a stack. The `upgrade-script` job is the test; the backup through the API (a token, the polling of its status) is not in it and is for the maintainer's hands.

### Upgrading
- Nothing to do. The script is for the upgrades from here on; a 0.3.x install moves volumes first (V0.4.22).

## V0.4.23 (2026-10-25)

One command makes a working .env.

### Added
- **`deploy/init.sh` and `deploy/init.ps1`:** make the `.env` of a container install from `.env.example`: the host name (`--host`, `-HostName`; letters, digits, dots and hyphens only), two random 64-character secrets (`openssl rand -hex 32`, or `/dev/urandom` without it; .NET's random number generator in PowerShell), and `COMPOSE_PROFILES` set to `edge` or, with `--no-edge` / `-NoEdge`, to nothing. On Unix the file is readable by its owner alone; the PowerShell file is UTF-8 without a byte-order mark, which Compose would read as part of the first name. They print the command to start and remind you to keep a copy of `REPOMEMO_SECRET_KEY`.
- **They refuse to overwrite** a `.env` that exists, and say why: new secrets would sign everyone out and make the stored provider keys unreadable. `--output` / `-Output` names another file.
- **A CI job, `init-scripts`,** runs both: the host, the profile and two 64-hex secrets are in the file, the shell file is mode 600, a second run is refused, two runs share no secret, and `docker compose config` accepts what each wrote.

### Changed
- **`.gitattributes`** keeps `deploy/*.sh` on LF. `container:check` holds the scripts and the CI job.
- **The documents:** the operations guide, the technical mindmap, the roadmap and the release index.
- **The version** is `0.4.23` everywhere.

### Checks
- **Run:** the shell script itself (it makes a file, refuses a second run, rejects a bad host name and leaves secrets that differ; no server or container is involved), `container:check`, `version:check`, `docs:check`.
- **Not run:** the PowerShell script (no PowerShell session was opened for it) and `docker compose config` on the result; the CI job does both.

### Upgrading
- Nothing to do. An existing `.env` is never touched.

## V0.4.22 (2026-10-25)

The compose files keep the data in volumes of their own.

### Changed
- **Three volumes for the data:** `repomemo-db` (the database and its write-ahead log), `repomemo-files` (the stored files) and `repomemo-backups`, beside `repomemo-data`, which keeps the key file, the logs and the data folder itself. `docker-compose.yml` and `deploy/api.yml` mount them and set `REPOMEMO_DB_DIR`, `REPOMEMO_BLOB_DIR` and `REPOMEMO_BACKUP_DIR` (V0.4.4) to them, so each can be backed up, sized and put on its own disk. The compose files are otherwise shaped as V0.4.18 left them: `web` and the server always, `edge` and `previews` as profiles. The service is still called `server` (it is the API).

### Added
- **`deploy/migrate-volumes.sh [project]`:** moves a 0.3.x install's single volume into the three. It refuses to run while a container uses the old volume, when the old volume holds no database, or when a new volume is not empty; it copies the database (with its log), the stored files and the backups, checks the number of files and the database's size, and **deletes nothing**: the old copies stay under `workspace-data/main` in `repomemo-data` until the operator removes them, with the command printed.
- **A CI job, `volume-migration`,** runs the server image once on a single volume the way 0.3.x ran it, makes an account, stops it, runs the script, starts the compose stack and signs the same account in, checks the database is in `repomemo-db`, and checks that a second run of the script is refused. (V0.4.27 repeats it with the real 0.3.27 images.)

### Changed (checks and documents)
- **`container:check`** holds the three volumes and variables in both compose files, that the script knows all four volumes and deletes nothing, and the CI job.
- **The documents:** the operations guide, the technical mindmap, the roadmap and the release index.
- **The version** is `0.4.22` everywhere.

### Checks
- **Run:** `container:check`, `version:check`, `docs:check`, a syntax check of the script.
- **Not run:** the script and the stack. The `volume-migration` job is the test and its first run is the one to read; a Windows host runs the script from WSL or Git Bash.

### Upgrading
- **Stop the stack, run `sh deploy/migrate-volumes.sh`, start the stack,** sign in and check your data, then remove the old copies as the script says. A new install needs nothing. Without the move, a server started on the new files would find no database in `repomemo-db` and begin a new empty one: the old data is not lost, it is simply not used.

## V0.4.21 (2026-10-25)

The client says plainly when the server is not its release.

### Added
- **A version check at start:** the web app (and the desktop window, which runs the same app) reads the `version` in `/health` and compares its major and minor numbers with its own, which the build now knows (`__REPOMEMO_VERSION__`, from `package.json`). The patch number never matters. A server on an earlier minor, or one that does not report a version (earlier than 0.4.3), is **too old**; one on a later minor is **too new**. Instead of a screen that half works, a page (`VersionMismatch`) shows both versions and what to do: update the server, reload or clear the cache, or install the desktop app that matches the server.
- **The desktop connect screen** applies the same rule: it does not remember a server of another release line and says why in the message.

### Changed
- **`lib/versionCheck.ts`** holds the rule and the advice; the page uses the existing sign-in layout, so no stylesheet changed.
- **The documents:** the operations guide, the technical mindmap, the roadmap and the release index.
- **The version** is `0.4.21` everywhere.

### Checks
- **Run:** `typecheck`, `styles:check`, `a11y:check`, `version:check`, `docs:check`.
- **Tests written, not run:** seven web tests (`versionCheck.test.tsx`: same minor, older, newer, no version, an unknown own version, the advice, the page) and one for the desktop connect screen; the existing desktop tests now give their stand-in server the client's version.
- **Not run:** an app against a real server of another release.

### Upgrading
- A browser that cached the old web app may say *This app is too old for this server* until it reloads, after the server moves to a new minor release.

## V0.4.20 (2026-10-25)

The server can serve the web app itself.

### Added
- **`REPOMEMO_WEB_DIR`:** the absolute path of the built web app, which the server then serves itself, for a single-process install: one program, one port, no web server in front. Off by default; the folder must hold an `index.html`. Everything under `/v1/` and `/health` stays the API's. Any other path is a file of the folder, or the page for a client route; a missing file with an extension is a 404 and not the page. Hashed files under `/assets/` are cached for a year and the rest revalidates; the page carries the security headers (`nosniff`, `X-Frame-Options: DENY`, no referrer, HSTS behind an https public address) and the content security policy the build wrote. Only `GET` and `HEAD` are answered.
- **`/config.json` is made by the server** in that mode: the API is this host (`apiUrl` empty) and `productName` is the new **`REPOMEMO_PRODUCT_NAME`** (one line, 60 characters or fewer, no quote or backslash). `--check-config` says whether the web app is served.
- **`web_files.rs`:** the path rules (no `..`, backslash, NUL or drive; a link out of the folder names nothing), the media types and the answer.

### Changed
- **The documents:** the settings reference (two rows), the operations guide, the technical mindmap, the roadmap and the release index.
- **The version** is `0.4.20` everywhere.

### Checks
- **Compiled and linted:** `cargo clippy -D warnings` over the server with its tests, `cargo fmt --all --check`, `version:check`, `docs:check`.
- **Tests written, not run:** four unit tests (the API's paths, the files found and the paths refused, the config for this host, the media types) and a server test that serves a page, a client route, a hashed file, a missing file, `/config.json` and still answers `/v1` and `/health/ready`.
- **Not run:** the server with a real build of the web app on one port (the release's check, for the maintainer's hands).

### Upgrading
- Nothing to do. The feature is off until `REPOMEMO_WEB_DIR` is set.

## V0.4.19 (2026-10-25)

Sample front doors for nginx and Traefik.

### Added
- **`deploy/proxies/`:** a sample nginx configuration and a sample Traefik dynamic configuration (file provider) for the web image on `127.0.0.1:8080`, with a README of what every front door must do. Each names the server's settings that go with it (`REPOMEMO_TRUST_PROXY=true`, `REPOMEMO_TRUSTED_PROXIES`, `REPOMEMO_PUBLIC_URL`, `REPOMEMO_ALLOWED_ORIGIN`), passes the client's address in `X-Forwarded-For`, sends HSTS, and **does not buffer the live event streams**: nginx has a `location` for `/v1/workspaces/<id>/events` and `/v1/system/events` with `proxy_buffering off` and a one-hour read timeout, Traefik flushes at once. nginx also allows 1 GiB uploads without request buffering.

### Changed
- **`container:check`** reads both samples: the trusted-proxy and address settings named, buffering off, the client's address set from the connection and not appended to, HSTS, Traefik not trusting forwarded headers from anyone, and that the two event routes still exist in the server.
- **The documents:** the operations guide (a bullet and *behind your own proxy*), the technical mindmap, the roadmap and the release index.
- **The version** is `0.4.19` everywhere.

### Checks
- **Run:** `container:check`, `version:check`, `docs:check`.
- **Not run:** nginx or Traefik with these files; they are samples to adapt (the host name, the certificate paths, the entry point and resolver names).

### Upgrading
- Nothing to do.

## V0.4.18 (2026-10-25)

TLS is a piece of its own, in front of a web image that speaks plain HTTP.

### Changed
- **The compose file's front door is two services.** The `proxy` service and image are gone. `web` (the `webapp` image of V0.4.15–V0.4.17) always runs: the app over plain HTTP, `/config.json`, and `/v1` and `/health` forwarded to the server (`REPOMEMO_API_UPSTREAM: server:3020`). It is published on `127.0.0.1:8080` (`REPOMEMO_WEB_PORT`) for a proxy of one's own. `edge` runs only with `COMPOSE_PROFILES=edge`.
- **`edge` is TLS and nothing else:** a new `edge` stage and image (`docker/Caddyfile.edge`, `docker/edge-entrypoint.sh`) in front of `web`, with three ways to a certificate: Caddy's automatic ones for a public name (Let's Encrypt), its own authority for `localhost`, or **the operator's own**: `tls.crt` and `tls.key` in `deploy/certs` (or `REPOMEMO_TLS_DIR`), mounted read-only at `/certs`; one of the two files without the other stops it with a message. It drops every capability but `NET_BIND_SERVICE`, and HSTS is its header. `.gitignore` keeps keys out of the repository.
- **The client's address crosses two hops:** the web image keeps the `X-Forwarded-For` the edge sends (`trusted_proxies`, private ranges by default, `REPOMEMO_WEB_TRUSTED_PROXIES` to change) and adds to it; the server trusts both addresses (`REPOMEMO_TRUSTED_PROXIES: 172.29.0.2,172.29.0.4`).
- **CI:** the `compose` job asks for the `edge` profile; new jobs bring the stack up **without** the edge (the web image answers on 8080 in plain HTTP, no edge service runs) and **with a certificate of one's own** (the one served is the one supplied); the arm64 job builds the web and edge images; the image scan reads the edge and the web image.
- **`container:check`** is rewritten for the new shape (stages, targets, labels, the two trusted addresses, the edge's profile, capabilities and mount, the entrypoint and the three CI jobs).
- **The documents:** the operations guide (services table, *behind your own proxy*), UPGRADING, the technical mindmap, the roadmap and the release index.
- **The version** is `0.4.18` everywhere.

### Checks
- **Run:** `container:check`, `version:check`, `docs:check`, a syntax check of the entrypoint.
- **Not run:** any image or the stack. The three CI jobs are the test and their first run is the one to read (Caddy's `trusted_proxies static private_ranges` global option and the `tls` file form in particular).

### Upgrading
- **Add `COMPOSE_PROFILES=edge` to your `.env`** to keep HTTPS on 80 and 443; without it the stack answers on `127.0.0.1:8080` only. Certificates already obtained stay in the `caddy-data` volume. See UPGRADING.md.

## V0.4.17 (2026-10-25)

The web app and the API can have two addresses.

### Added
- **Cross-origin mode:** the web image is told the API's address (`REPOMEMO_WEB_API_URL`), hands it to the app through `/config.json` (V0.4.14) and **adds its origin to `connect-src`** of the content security policy it sends. The entrypoint works the origin out when the container starts and refuses a value with a character outside letters, digits and `. : / _ - [ ]` or without `http://` or `https://`. The build's own policy in the page is taken out of the image, since a policy in the page can only narrow the header's.
- **`REPOMEMO_COOKIE_SAMESITE`:** `strict` (the default), `lax` or `none` for the sign-in cookie. `none` is refused at start unless `REPOMEMO_PUBLIC_URL` is `https://`, since a browser takes `SameSite=None` only with `Secure`. `--check-config` prints it.
- **A third origin is refused:** under `lax` or `none`, a request that presents the refresh cookie with an `Origin` that is not in `REPOMEMO_ALLOWED_ORIGIN` gets 403 before the token is read; requests with no `Origin` (the console, scripts) are unaffected, and so is the default `strict` mode, where a browser sends the cookie to no other site. The CORS answer already named listed origins only, with credentials.
- **`scripts/cross-origin-test.mjs`** makes the requests a browser makes from the web app's origin and from a third one against a new server: register (the cookie, not the body), refresh, the third origin's 403, the two preflights and the opening of the live event stream. CI's `cross-origin` job runs it; the `web-image` job now also reads the policy's `connect-src` and that the page carries none.

### Fixed
- **The web image's entrypoint (V0.4.16) checked its values with a bracket expression that did not close where meant,** so a value was refused or accepted wrongly. It now reads `[!]A-Za-z0-9.:/_[-]`; `container:check` insists on that form.

### Changed
- **The documents:** the settings reference (two rows), the operations guide, the technical mindmap, the roadmap and the release index.
- **The version** is `0.4.17` everywhere.

### Checks
- **Compiled and linted:** `cargo clippy -D warnings` over the server with its tests, `cargo fmt --all --check`, `container:check`, `version:check`, `docs:check`, a syntax check of the entrypoint and a try of its bracket expression on seven values.
- **Tests written, not run:** server unit tests (the `SameSite` attribute follows the setting; the origin rule under `Strict`, `Lax` and `None`, with and without an `Origin`; the setting's parsing and its https rule). The script and the CI job are the test of the two origins.
- **Not run:** any image or server; the sign-in through a real browser on two addresses.

### Upgrading
- Nothing to do. To put the web app elsewhere, follow *Cross-origin mode* in the operations guide.

## V0.4.16 (2026-10-25)

The web image can forward the API, so one address serves both.

### Added
- **Same-origin mode:** the web image (V0.4.15) forwards `/v1/*`, `/health` and `/health/*` to `REPOMEMO_API_UPSTREAM`, which is read when the container starts. The live event streams are not buffered (`flush_interval -1`) and the first byte may take 180 seconds, as in the compose file's proxy. One address then serves the app and the API, and the sign-in cookie stays simple.
- **`docker/web-entrypoint.sh`,** the image's entrypoint: it takes `host:port` or `http(s)://host:port`, refuses a character outside letters, digits and `. : / _ - [ ]`, a path or a missing port before it reaches the configuration, writes the rule into `/tmp/api.caddy` and starts Caddy. With the variable unset the image serves the app alone and answers 502 on the API paths, saying why.
- **A CI job, `same-origin`,** builds the server and the web image, runs them on a network with the web image read-only and without capabilities, and asks `/health/ready`, `/health`, an anonymous `/v1/workspaces` (401) and the page through the web image alone.

### Changed
- **`container:check`** holds the snippet, the unbuffered streams, the entrypoint and the CI job.
- **The documents:** the operations guide (including that the web image's address belongs in the API's `REPOMEMO_TRUSTED_PROXIES`), the technical mindmap, the roadmap and the release index.
- **The version** is `0.4.16` everywhere.

### Checks
- **Run:** `container:check`, `version:check`, `docs:check`, a syntax check of the entrypoint script.
- **Not run:** any image build or the two containers. The `same-origin` job covers health, refusal and the page; **sign-in, search and live events through the web image** are the release's real test and are for the maintainer's hands now and for V0.4.26's smoke test in CI.

### Upgrading
- Nothing to do. The compose file still uses the `proxy` image.

## V0.4.15 (2026-10-25)

The web client is an image of its own.

### Added
- **`repomemo-web`:** a `webapp` stage in the Dockerfile (`docker build --target webapp`) and `docker/Caddyfile.web`. The built web app behind Caddy over **plain HTTP on port 8080**, with no API, no certificate and no database to start. It runs as the unprivileged user 10001 with Caddy's file capability removed, so it runs read-only and with every capability dropped.
- **What it serves:** the app with the single-page fallback; hashed files under `/assets/` with `Cache-Control: public, max-age=31536000, immutable` and the page itself with `no-cache`; zstd and gzip; the content security policy the build wrote and `X-Content-Type-Options`, `X-Frame-Options: DENY`, `Referrer-Policy: no-referrer` and a restrictive `Permissions-Policy`, with no `Server` header; `/config.json` from `REPOMEMO_WEB_API_URL` and `REPOMEMO_PRODUCT_NAME`; and `/healthz`, its health check. `/v1` and `/health` answer 502 with a plain sentence until V0.4.16 forwards them.
- **The image has the same labels** as the others and is read by the image scan of V0.4.13.
- **A CI job, `web-image`,** builds it and runs it alone, read-only and without capabilities, with the two settings in its environment, and reads the page, the fallback, the headers, a hashed file's cache header, `/config.json`, compression and `/healthz`.

### Changed
- **`container:check`** holds the Caddyfile's promises, the unprivileged user, the health check and the CI job; the label rule covers the fourth image. `.gitattributes` keeps the new Caddyfile on LF.
- **The documents:** the operations guide, the technical mindmap, the roadmap and the release index.
- **The version** is `0.4.15` everywhere.

### Checks
- **Run:** `container:check`, `version:check`, `docs:check`.
- **Not run:** any image build; the `web-image` job is the test, and the first run is the one to read (Caddy's `setcap -r`, the `wget` health check and the header names in particular).

### Upgrading
- Nothing to do. The compose file still uses the `proxy` image; V0.4.22 builds it around the two.

## V0.4.14 (2026-10-25)

The web app reads its server's address when it starts.

### Added
- **Runtime configuration of the web app:** before it draws anything, the built web app asks the host that serves it for `/config.json` and reads `apiUrl` (the API's address; empty is the page's own host; absent keeps the address it was built with) and `productName` (the browser tab's title). Only an empty or plain `http(s)` address with no login, query or fragment is taken, and a one-line name of at most 60 characters; anything else is ignored. A host with no such file, or the development server (which answers with the page), leaves the build's own values. The desktop window does not ask: its shell tells it the server.
- **The build writes a default `config.json`** beside the page with the address it was built for. **The compose file's proxy serves its own** from `REPOMEMO_WEB_API_URL` and `REPOMEMO_PRODUCT_NAME` (new compose settings, in `.env.example`), uncached, so one built image can be given another name or address by its environment alone.

### Changed
- **`sharedApi`** has `configureWebApi`, the web counterpart of the desktop's `configureDesktopApi`; `main.tsx` starts the app after the configuration is read.
- **`container:check`** insists on the Caddyfile's `/config.json`, the two settings in the compose file and the build's default file.
- **The documents:** the settings reference, the operations guide, the technical mindmap, the roadmap and the release index.
- **The version** is `0.4.14` everywhere.

### Known limit
- The page's **content security policy** is still the one the build wrote, which allows the page's own host and the address it was built for. A runtime `apiUrl` on another origin is therefore blocked by the browser until the policy is built from the configuration at start (V0.4.17, cross-origin mode). Same-origin use, the default, is unaffected.

### Checks
- **Run:** `typecheck`, `container:check`, `version:check`, `docs:check`.
- **Tests written, not run:** seven web tests (`runtimeConfig.test.ts`): valid and refused addresses, the name, non-object files, the loader's missing, failing and non-JSON answers, and the address and title applied.
- **Not run:** any build of the web app or the images.

### Upgrading
- Nothing to do.

## V0.4.13 (2026-10-25)

The images are scanned for known vulnerabilities in CI.

### Added
- **An `image-scan` job in CI:** it builds the server, the converter and the proxy images and reads each with Trivy 0.75.0, run as a container pinned by digest so that a moving tag cannot change the scanner. A **high or critical** finding that has a fix fails the job once all three have been read (so one report is not hiding another). The JSON and table reports of each image are kept as the `image-scan-reports` artifact.
- **`.trivyignore`:** the findings that are accepted, each with a reason and a date to look again; empty today.

### Changed
- **`container:check`** insists on the job, on the scanner's digest, on the severity and the exit code, on the three images and on the kept reports.
- **The documents:** the operations guide, the technical mindmap, the roadmap and the release index.
- **The version** is `0.4.13` everywhere.

### Checks
- **Run:** `container:check`, `version:check`, `docs:check`.
- **Not run:** any image build or scan, so what the images contain is **not known yet**; the job's first run is the one to read, and it may fail on a finding that needs a decision (a base image to move, or an entry in `.trivyignore`). The release's check, that a known-bad base image fails the job, was not tried.

### Upgrading
- Nothing to do.

## V0.4.12 (2026-10-25)

The converter image is built, tagged and tried on its own.

### Added
- **A CI job for the converter image on its own:** `converter-image` builds the Dockerfile's `converter` stage alone and tagged, checks that it runs as the unprivileged `repomemo` user with the listener as its entrypoint, starts it as the `previews` profile does (no network, a read-only root, every capability dropped, `no-new-privileges`) and waits for its socket, and writes the LibreOffice version it carries into the job summary.
- **`REPOMEMO_CONVERTER_IMAGE`** (compose, not the server): the converter service starts the image it names (such as `repomemo-converter:0.4.12`) instead of the one built here; unset, nothing changes. V0.4.40 uses it for released images.

### Changed
- **`container:check`** insists on both: the compose service's `image:` variable and the CI job that starts the converter alone.
- **The documents:** the operations guide, the technical mindmap, the roadmap and the release index.
- **The version** is `0.4.12` everywhere.

### Checks
- **Run:** `container:check`, `version:check`, `docs:check`.
- **Not run:** any image build; the new CI job is the test and its first run is the one to read (the LibreOffice version it prints is what the server will accept or refuse, 26.2.5 or later).

### Upgrading
- Nothing to do.

## V0.4.11 (2026-10-25)

The API can run alone.

### Added
- **`deploy/api.yml`:** a compose file with the server and its volume and nothing else: no web client, no proxy, and one port published on the machine's loopback (`REPOMEMO_API_PORT`, 3020). It takes the same two secrets, builds the same server image with the same build arguments, is locked down like the full stack's server (read-only root, `cap_drop: ALL` with the five of V0.4.10, `no-new-privileges`) and expects the front door of the person who runs it: `REPOMEMO_PUBLIC_URL` (the address people use), `REPOMEMO_ALLOWED_ORIGIN` (the web client's origin, the public URL by default) and `REPOMEMO_TRUSTED_PROXIES`. Start it with `docker compose -f deploy/api.yml --env-file .env up -d --build --wait`.
- **A CI job, `api-only`,** starts it with throwaway secrets, waits for it to be healthy, asks `http://127.0.0.1:3020/health/ready`, checks that the server is the only service and takes it down.

### Changed
- **`container:check`** reads `deploy/api.yml`: the server alone, its image, the lock-down, one published port on the loopback, no web or proxy service, only variables the server reads, the two required secrets, and the CI job.
- **`.env.example`** names the three variables that only this file uses.
- **The documents:** the operations guide, the technical mindmap, the roadmap and the release index.
- **The version** is `0.4.11` everywhere.

### Checks
- **Run:** `container:check`, `version:check`, `docs:check`.
- **Not run:** the stack itself. The `api-only` job is the test and the maintainer's first run of it is the one to read (nested defaults such as `${A:-${B:-c}}` need a current Docker Compose).

### Upgrading
- Nothing to do. The full stack is unchanged.

## V0.4.10 (2026-10-25)

A volume that belongs to root no longer stops the server.

### Fixed
- **Volumes that belong to someone else:** a bind-mounted folder owned by root (or by whoever made it on the host) used to fail on the first write. The server image now starts `repomemo-entrypoint` (`docker/entrypoint.sh`), which, started as root, gives the data folders to the unprivileged user 10001 when they or something shallow inside them belong to another account, and runs the server as that user (`setpriv`), so the server never runs as root. The stored files are not walked (that would take minutes at every start of a large install). Started as another user (`user:` in compose, `runAsUser` in Kubernetes) it cannot change owners, and refuses with a message naming the folder and the `chown` to run on the host. A folder it cannot change (a share that refuses `chown`) is refused with the reason.

### Changed
- **The compose file** keeps `cap_drop: ALL` for the server and adds back only `CHOWN`, `DAC_OVERRIDE`, `FOWNER`, `SETUID` and `SETGID` for the entrypoint; they are gone once the server runs as user 10001, and `no-new-privileges` stays. The Dockerfile's server stage has no `USER` line any more.
- **`container:check`** insists on the entrypoint, on its drop to the same user the Dockerfile makes, on the `chown`, on those five capabilities and no more, and on no `USER root`.
- **The documents:** the operations guide, the technical mindmap, the roadmap and the release index.
- **The version** is `0.4.10` everywhere.

### Checks
- **Run:** `container:check`, `version:check`, `docs:check`.
- **Not run:** any image build and both cases of the release's test (a fresh named volume, a root-owned bind mount); they are for the maintainer's hands and for CI once V0.4.26 starts the stack.

### Upgrading
- Nothing to do for the compose file. If you run the image yourself with `--cap-drop ALL`, add `--cap-add` for the five above, or run it with `--user 10001` on folders that user owns.

## V0.4.9 (2026-10-25)

Every image says what it is.

### Added
- **OCI labels on the three images:** `repomemo-server`, `repomemo-converter` and `repomemo-proxy` each carry `org.opencontainers.image.title`, `description`, `source`, `licenses`, `version`, `revision` and `created`. The version, the revision and the creation date come from the build arguments of V0.4.3 (`REPOMEMO_IMAGE_TAG`, `REPOMEMO_BUILD_COMMIT`, `REPOMEMO_BUILD_DATE`), the source from a new `REPOMEMO_SOURCE_URL` (the repository's address by default, for a fork to change). A build without them says `unknown`. The labels are the last lines of their stage, so a new commit does not rebuild the layers above.
- **The compose file passes the three build arguments to the converter and the proxy** as it already did to the server.

### Changed
- **`container:check`** asserts, for each of the three images, the seven labels, the three `ARG`s of the stage and the compose file's build arguments for it.
- **The documents:** the operations guide, the technical mindmap, the roadmap and the release index.
- **The version** is `0.4.9` everywhere.

### Checks
- **Run:** `container:check`, `version:check`, `docs:check`.
- **Not run:** any image build, so `docker inspect` has not been read; that is for the first CI build.

### Upgrading
- Nothing to do. `docker inspect` on an image built from this release names the release and the commit.

## V0.4.8 (2026-10-25)

The server image is smaller and needs no curl.

### Added
- **`repomemo-server --health`:** asks the server on this machine for `/health/ready` (the address in `REPOMEMO_SERVER_ADDR`, an all-interfaces address asked on the loopback), with a few lines of HTTP by hand and a 4-second limit, and exits 0 on a 200 and 1 otherwise, saying why on standard error.

### Changed
- **The server image:** the health check is `CMD ["repomemo-server", "--health"]`, `curl` is no longer installed, and the binary is copied out of the build stripped (`strip --strip-unneeded`). `socat` stays, since `repomemo-soffice-wrapper` reaches the converter's socket with it.
- **`container:check`** insists on the stripped binary, on the `--health` check and on no `curl` in the server stage; its rule that the health check asks the listening port now reads `REPOMEMO_SERVER_ADDR` of the image against the compose file's.
- **The documents:** the settings reference (the health check), the operations guide, the technical mindmap, the roadmap and the release index.
- **The version** is `0.4.8` everywhere.

### Checks
- **Compiled and linted:** `cargo clippy -D warnings` over the server with its tests, `cargo fmt --all --check`, `container:check`, `version:check`, `docs:check`.
- **Tests written, not run:** three unit tests of the probe (200 and 503 from a stand-in server, a closed port and a non-HTTP answer as errors, an all-interfaces address asked on the loopback).
- **Not measured:** the size of the image. No image was built, so the saving is not recorded; the first CI build is where to read it.

### Upgrading
- Nothing to do. A `docker run` with its own health check that used `curl` in the image should use `repomemo-server --health`.

## V0.4.7 (2026-10-25)

The server image builds its dependencies in a layer of their own.

### Changed
- **A cached layer of dependencies:** the Dockerfile's server build is now three stages. `chef` installs `cargo-chef` (pinned at 0.1.71), `planner` writes a recipe that holds only the manifests and the lock file, and `build` cooks the dependencies of `repomemo-server` from that recipe in a layer that stays cached until a dependency changes, then copies the code and builds it. A change of code no longer rebuilds the dependencies. The cache mounts over the registry and the build folder are removed: a mount over `/build/target` would hide the cooked layer, and the layer is what CI and a fresh machine can keep.
- **CI measures it:** a `server-image-cache` job builds the server image whole on a clean runner, appends a line to the server's `main.rs`, builds again, writes both times to the job summary and fails when the second took more than 60% of the first.
- **`container:check`** insists on the order (`cargo chef cook` before `COPY . .`), on a pinned `cargo-chef`, on no cache mount over `/build/target`, and on the CI job.
- **The documents:** the operations guide, the technical mindmap, the roadmap and the release index.
- **The version** is `0.4.7` everywhere.

### Checks
- **Run:** `container:check`, `version:check`, `docs:check`.
- **Not run:** any image build, so the saving is **not measured yet**: the new CI job is the measure, and the maintainer should read its first run (and that `cargo chef cook -p` accepts the workspace as it is).

### Upgrading
- Nothing to do. The first build of an image after this release builds the dependencies once; the ones after reuse them.

## V0.4.6 (2026-10-25)

The server stops cleanly on SIGTERM and Ctrl+C.

### Added
- **A clean stop, in order:** on SIGTERM or Ctrl+C the server stops listening (a new connection is refused by the system), ends the live event streams at once (they used to be cut at their next check, up to 30 seconds later), lets running requests finish within `REPOMEMO_SHUTDOWN_GRACE_SECONDS` (30; it was a fixed 30), stops the background work, marks the jobs that were still running as interrupted, and folds the write-ahead log into the database before closing it. The next start opens a database that needs no recovery and finds no job running forever.
- **`repomemo_server::start`** returns a `RunningServer` (`router`, `begin_closing`, `finish`); `router` stays for the tests. The event streams wake on a signal (`closing_signal`) as well as on their timer.

### Changed
- **`serve`** takes the grace as a parameter.
- **The documents:** the settings reference (one setting), the operations guide (§4, *Stopping it cleanly*), the technical mindmap, the roadmap and the release index.
- **The version** is `0.4.6` everywhere.

### Checks
- **Compiled and linted:** `cargo clippy -D warnings` over the server with its tests, `cargo fmt --all --check`, `version:check`, `docs:check`.
- **Tests written, not run:** a server test that registers an account, stops cleanly and finds the write-ahead log empty, then starts again on the same folder. `docker stop` itself (the signal, the 10-second default, V0.4.28's `stop_grace_period`) is for the maintainer's hands.

### Upgrading
- Nothing to do. If a service manager stops the server, give it a wait longer than `REPOMEMO_SHUTDOWN_GRACE_SECONDS`.

## V0.4.5 (2026-10-25)

A bad data volume is named when the server starts.

### Added
- **A check of the folders at start:** before storage is opened, the data folder and the folders split from it (V0.4.4) are tried. A folder that **cannot be written** (a read-only mount, a volume owned by another account) or is **full** (under 16 MiB free) refuses the start, and the reason is the first log line, also written to standard error; on Linux it says which user owns the folder and which user the server runs as. A folder **under the disk floor** (`REPOMEMO_DISK_FLOOR_MB`) or on a **network file system** (NFS, SMB/CIFS, 9p, sshfs, a Windows share, from `/proc/self/mountinfo` or the shape of a UNC path) is a warning; SQLite's write-ahead log does not work safely there.
- **`--check-config`** prints the same findings (free space and file system included).

### Changed
- **The documents:** the operations guide (§4, *The check of the folders at start*), the technical mindmap, the roadmap and the release index.
- **The version** is `0.4.5` everywhere.

### Checks
- **Compiled and linted:** `cargo clippy -D warnings` over the server with its tests (the Unix-only code is compiled by CI on Linux, not here), `cargo fmt --all --check`, `version:check`, `docs:check`.
- **Tests written, not run:** the naming of a network file system by the longest mount point (NFS under `/srv`, a local volume inside it, SMB), a Windows share path, and the warning under the disk floor. A read-only or full volume is for the maintainer's hands.

### Upgrading
- Nothing to do. A server that starts on a read-only or nearly full volume now refuses with the reason; one on a network file system warns and starts.

## V0.4.4 (2026-10-25)

The data folder can be split across volumes.

### Added
- **Three folders that can sit on their own volumes:** `REPOMEMO_DB_DIR` (the database file and its write-ahead log), `REPOMEMO_BLOB_DIR` (the stored files) and `REPOMEMO_BACKUP_DIR` (the backups). Each is an absolute path and defaults to its place under the data folder (the folder itself, `blobs/`, `backups/`), so nothing changes until one is set. They need `REPOMEMO_SERVER_DATA_DIR` (they belong to one environment, never to the many a managed server can switch between).
- **`StorageConfig`** gains `db_dir`, `blob_dir` and `backup_dir`, and `StorageEngine` `db_dir()` and `backup_dir()`. The environment a server is started on is recognised from the database folder (`inspect_environment_split`).
- **Files being received stay on the files' volume:** with a separate files folder they wait in `.incoming` inside it, so storing one is still a rename; the sweep of untracked files skips that folder.
- **`--check-config`** tries each folder that is set and names it in its errors (exit code 2 as for the data folder).

### Changed
- **Backups** are made in, listed from and measured for free space in the backups folder; the system overview and the metrics read the database file's size where it is.
- **The documents:** the settings reference (three variables), the operations guide (§4, *Splitting the data folder*), the technical mindmap, the roadmap and the release index.
- **The version** is `0.4.4` everywhere.

### Checks
- **Compiled and linted:** `cargo clippy -D warnings` over the storage crate and the server with their tests, `cargo fmt --all --check`, `version:check`, `docs:check`.
- **Tests written, not run:** a storage test that opens an engine with three folders, finds the database and the files where they were put and the received file beside them, leaves a waiting file alone in the sweep and recognises the environment. The backup and restore of a split install, and three mounts in a container, are for the maintainer's hands (V0.4.22 gives compose its volumes).

### Upgrading
- Nothing to do. To move an install onto separate volumes, follow *Splitting the data folder* in the operations guide: stop, move, set the variable, start.

## V0.4.3 (2026-10-25)

The server says which build it is.

### Added
- **Build information:** `/health` now carries `version`, `commit`, `built_at` and `image` (also in the detached answer a server gives before an environment is attached), and `repomemo-server --version` (or `-V`) prints the same on one line. The commit, the date and the image tag are `REPOMEMO_BUILD_COMMIT`, `REPOMEMO_BUILD_DATE` and `REPOMEMO_IMAGE_TAG`, read **at build time**; a build that was not given them says `unknown`.
- **The image is told what it is:** the Dockerfile's build stage takes the three as `ARG`s, the compose file passes the commit, the date and `REPOMEMO_VERSION` as the image tag, and `container:check` fails when either side lacks one.
- **The web client's type for `/health`** (`SharedHealth`) names the new fields, which the version check of V0.4.21 will read.

### Changed
- **The documents:** the settings reference (the three build-time variables), the operations guide, the technical mindmap, the roadmap and the release index.
- **The version** is `0.4.3` everywhere.

### Checks
- **Compiled and linted:** `cargo clippy -D warnings` over the server with its tests, `cargo fmt --all --check`, `typecheck`, `container:check`, `version:check`, `docs:check`.
- **Tests written, not run:** two unit tests (an unset build argument reads `unknown`; the version line names the package version) and a server test that `/health` names the version and carries the three other fields. No image was built.

### Upgrading
- Nothing to do. `/health` answers more fields; nothing that read it breaks.

## V0.4.2 (2026-10-25)

A configuration can be checked without starting the server.

### Added
- **`repomemo-server --check-config`:** reads the environment the way a start does (the same validation, so the same error messages), prints what the server would use and exits without binding a port or opening the database. The listening and public addresses, the allowed origins, proxy trust, where each secret comes from (the environment with its length, or the `_FILE` path) and never its value, the data folder, the backup and templates folders, backups and sign-up. The data folder is tried with a small file that is removed at once; a missing folder inside a writable one is "will be created". On Unix, a secret file that other accounts can read is a warning.
- **Exit codes:** `0` the configuration can start (warnings do not change it), `1` it is invalid (the error goes to standard error), `2` the data folder is not a folder, not writable, or cannot be created. In a container: `docker compose run --rm server --check-config`.

### Changed
- **The documents:** the settings reference, the operations guide (§4), the technical mindmap, the roadmap and the release index.
- **The version** is `0.4.2` everywhere.

### Checks
- **Compiled and linted:** `cargo clippy -D warnings` over the server with its tests, `cargo fmt --all --check`, `version:check`, `docs:check`.
- **Tests written, not run:** four unit tests (a valid configuration described without its secret, an invalid one with exit code 1, a data folder that is a file with exit code 2, a missing folder that will be created).

### Upgrading
- Nothing to do.

## V0.4.1 (2026-10-25)

Every secret can be given as a file, for Docker and Kubernetes secrets.

### Added
- **Secrets from files:** `REPOMEMO_JWT_SECRET`, `REPOMEMO_SECRET_KEY`, `REPOMEMO_BACKUP_KEY`, `REPOMEMO_BACKUP_KEY_PREVIOUS` and `REPOMEMO_SETUP_CODE` may each be replaced by the same name ending in `_FILE` (`REPOMEMO_JWT_SECRET_FILE=/run/secrets/jwt`), the path of a file holding the secret. The file's surrounding white space and line ending are dropped; a file that cannot be read or is empty is named in the error. A variable **and** a file for the same secret is refused at start, so a stale variable cannot hide the file meant to replace it. The same rules apply to the restore, which reads `REPOMEMO_SECRET_KEY` too.
- The SMTP password is not an environment variable (it is saved, sealed, from System › Mail), so it has no file form.

### Changed
- **The documents:** the settings reference (a paragraph on `_FILE` before the table), `.env.example`, the operations guide and the technical mindmap, the release index and the roadmap.
- **The version** is `0.4.1` everywhere.

### Checks
- **Compiled and linted:** `cargo check` and `cargo clippy -D warnings` over the server with its tests, `cargo fmt --all --check`, `version:check`, `docs:check`.
- **Tests written, not run:** four unit tests of the secret reading (a variable alone, a file alone without its line ending, both refused, a missing or empty file named).

### Upgrading
- Nothing to do. A secret given as a variable keeps working.

## V0.4.0 (2026-10-25)

The 0.4.x line is opened: preparation for deployment, on the server and on the client.

### Changed
- **The theme of the next line is chosen:** RepoMemo should deploy **directly on a server or in containers, containers preferred and tested**, with the API (Rust, its database and its files) and the web client (React) as separate containers, and the **desktop app resumed** as the web client's own UI and components, talking to the API directly, installable, an administrator being able to hand a team an executable already set for their server, Windows first, then Linux and macOS. Single sign-on, recorded as a proposal in V0.3.27, stays the candidate for the line after; its plan is set aside.
- **The plan and the index are written:** [mindmap/plans/2026-10-25-deployment.md](mindmap/plans/2026-10-25-deployment.md) (where deployment stands, the shape aimed at, decisions 33–40, the tracks, the desktop app in detail, how the line is tested, the risks) and [mindmap/RELEASES_0.4.md](mindmap/RELEASES_0.4.md): **67 single-purpose releases in eight tracks** (configuration and secrets; the server image; the web image and the topology; compose and running containers; direct install; registry and release pipeline; the desktop app on the web client; documents, security and closing). The roadmap carries the same tables and its decision table decisions 33–40.
- **The documents check knows the new index:** `docs:check` reads `RELEASES_0.4.md` for the current version and for its releases, and `CLAUDE.md` lists it.
- **The version** is `0.4.0` everywhere. No code changed.

### Checks
- **Run:** `version:check` and `docs:check` (the links of the two new documents included).
- **Not run:** nothing else applies: no code, test, build or server was touched.

### Upgrading
- Nothing to do.

## V0.3.27 (2026-10-24)

The 0.3.x line is closed: the checks are repeated, and the theme of V0.4.0 is recorded with its plan.

### Checks (repeated)
- **Pass:** `version:check`, `docs:check` (220 routes, 80 settings, 29 documents), `migrations:check` (43 files, every one locked), `container:check`, `templates:check`, `styles:check`, `a11y:check`, `types:check` (154 types), `typecheck`, `cargo fmt --all --check`, `cargo clippy --workspace --exclude repomemo-desktop --all-targets --locked -- -D warnings`, `cargo check -p repomemo-desktop --tests --locked`, `cargo deny check bans licenses advisories` and `npm audit --omit=dev --audit-level=high` (0 vulnerabilities).
- **Not run, on purpose:** the Rust tests, the web tests, the template checker's tests, the two container jobs of CI and any build or server: the maintainer runs them. The tests written in the 0.3.x line are counted in the technical mindmap (web 386, server 214, storage 47, api 106, domain 4, and the others as they were).

### Changed
- **An advisory is recorded:** RUSTSEC-2026-0387 (timing variability in `crypto-bigint` 0.5.5) was published since the last check and fails the blocking advisories job of CI. The crate arrives only through `jsonwebtoken`'s elliptic-curve backends (`p256`, `p384`, `ecdsa`); RepoMemo signs with HS256 and loads no elliptic-curve key, so none of those operations runs on a secret, and the fix (0.7.6) is a major version that `elliptic-curve` 0.13 cannot take. It is ignored in `deny.toml` with that reason, as RUSTSEC-2023-0071 is, to be revisited by 2027-01-15 or when `jsonwebtoken` moves on.
- **Decision 33 is recorded, as a proposal:** the theme of V0.4.0 is **single sign-on (OIDC) first**, as the roadmap recommended. It is the maintainer's to confirm: nothing of it is built, and the plan ([mindmap/plans/2026-10-24-single-sign-on.md](mindmap/plans/2026-10-24-single-sign-on.md)) lists decisions 34–38, ten single-purpose releases (V0.3.28–V0.3.37) and the risks.
- **The line is closed in the documents:** the roadmap's table and checklist, the 0.3 release index, and the technical mindmap's scope line.
- **The version** is `0.3.27` everywhere.

### Upgrading
- Nothing to do. `V0.4.0` is the tag the maintainer cuts after the tests of the clean install, as `V0.3.0` was.

## V0.3.26 (2026-10-24)

Security pass over what 0.3.x added.

The seventh review ([mindmap/security/2026-10-24-0.3-security-review.md](mindmap/security/2026-10-24-0.3-security-review.md)) read, in the code, what V0.3.1–V0.3.25 added: restoring a version, mentions, `CODEOWNERS` matching and review requests, the all-workspaces search, archive format 2, the import job, references and approvals, webhook deliveries and sending one again, the backup passphrase change, and the desktop shell without its local edition. **0 high, 1 medium, 6 low and 5 informational findings; every medium and low finding is fixed and none is open.**

### Fixed
- **A text of endless addresses could keep the server busy (medium):** the references between tracker items looked for the end of an address from every `http` in a text, so a comment of `http://` repeated without a space, written by anyone who can comment on a connected repository, made the refresh after a sync do quadratic work. Each address is now read once, to 300 characters at most, and a repository name over 300 characters is not read as one.
- **A commit named many times:** the file mentions walked every file a commit last changed for each word that named it; reading now stops when an item has its fifty mentions.
- **The import's place is taken before the upload:** a second archive from the same person, or one more than the server takes at once, is refused before the body is read, and a failed upload gives the place back.
- **Finished import jobs are capped:** five for a person and a hundred in all, the oldest going first, so reports cannot fill the server's memory for an hour.
- **Reading an import job asks for what starting one asks for:** a session or an `admin` token, not any token of its owner.
- **An import takes the sizes from the stored files:** each file and each earlier version gets the size of the file it names, which the import has just read and checked, so an archive cannot understate its files to the storage allowance; the report warns how many were corrected.
- **Recent places are forgotten at sign-out:** the command palette's list no longer stays in the browser's storage for the next person at a shared computer.

### Changed
- **The documents:** the review itself, the technical mindmap (its scope line, its list of reviews and the test counts: api 106, server 214, domain 4), the roadmap and the release index.
- **The version** is `0.3.26` everywhere.

### Checks
- **Compiled and linted:** `cargo clippy -D warnings` over the domain, api and server crates with their tests, `cargo fmt --all --check`, `typecheck`, `docs:check`, `version:check`.
- **Tests written, not run:** the reading of endless texts, the cap on finished jobs, the corrected sizes, and the import job's route among those that refuse tokens without the `admin` scope.
- **Not done:** the review was a reading; nothing was built, started or attacked. A pass with the running server (the load of an import, a hostile tracker text) is still the maintainer's.

### Upgrading
- Nothing to do.

## V0.3.25 (2026-10-24)

Three accessibility gaps are closed, and the checker holds them.

### Added
- **A skip link in dialogs:** a dialog that has a body starts with *Skip to the buttons*, which is out of sight until it has focus. It comes before the body, so going back from the first field reaches it, and it moves focus to the dialog's first button without closing it. A dialog without a field no longer risks starting on it: the first control takes the focus, as before. A question with no body has no link.
- **Toolbars that work from the keyboard:** the note editor's formatting bar and the bulk bar of the file list (`role="toolbar"`) are now one stop in the tab order, with the arrow keys, Home and End moving between the buttons (a disabled one is skipped, the end wraps), the way the WAI-ARIA toolbar pattern asks, and they say they are horizontal. The behaviour is one hook, `useToolbarKeys`.
- **Buttons of at least 24 px:** the one that was smaller (the remove button on a memory card's evidence, 20 px) is 24 px, from a new `--target-min` token.
- **`a11y:check` covers all three:** it fails on a `role="toolbar"` without a name, an orientation or a key handler, on a dialog built outside the shared one or a shared one without its skip link, and on a stylesheet rule that gives a button, an icon button or a close or remove control a box under 24 px in both directions (a 1 px label for screen readers is not a target). A new rule name for exceptions: `toolbar`.

### Changed
- **The documents:** the technical mindmap's account of the accessibility check and its test count (web 386), the functional mindmap, the development commands, the clean-install record and the roadmap. The X1 gap "not covered: a skip link inside dialogs, the note editor's toolbar roles, the target size" is closed; the retired local edition is still outside the check.
- **The version** is `0.3.25` everywhere.

### Checks
- **Run:** `a11y:check` with the new rules (126 components, 94 colour pairs, 88 button rules), `typecheck`, `styles:check` (its baseline loses the 20 px entry), `docs:check`, `version:check`.
- **Tests written, not run:** tests of the dialog's skip link (before the body, to the buttons, not the first focus, absent from a question, pointing at its own footer) and of the toolbar keys (one tab stop, arrows, Home, End, wrap, modifiers and other keys left alone, a disabled button skipped, focus given some other way, vertical).
- **Not done:** a pass with a screen reader (NVDA or VoiceOver) and a browser audit (axe or Lighthouse) are still the maintainer's to run; the checks are static.

### Upgrading
- Nothing to do.

## V0.3.24 (2026-10-24)

The backup passphrase can be changed.

### Added
- **`REPOMEMO_BACKUP_KEY_PREVIOUS`:** the passphrase before the last change. Put the new one in `REPOMEMO_BACKUP_KEY` and the old one here (16 or more characters, different from the new one, and only with the new one set) and restart. New backups are sealed with the new passphrase; a backup made with the old one still restores, because the server tries the new passphrase and then the old (the restore says when it used the old one).
- **Which key opens a backup:** System › Backups marks each sealed backup *previous key* (the old passphrase opens it) or *key not available* (neither does), and the listing carries `sealed_with` (`current`, `previous`, `neither`). Deriving a key is slow, so it is worked out once for each way of sealing and remembered.
- **Sealing again:** *Seal again with the current key* on a backup the previous key made, `POST /v1/system/backups/{name}/reseal` and `backup reseal <backup> --yes` in the console (system administrators). The files are read with the old passphrase and written afresh with the new one, a chunk at a time, so the plain text is never whole in memory or on the disk, into `<name>.resealing.partial` beside the original; the original is replaced only when the new folder is whole, and a failure leaves it as it was. It needs about as much free disk as the backup takes (the server checks first) and is refused for a backup the current key already opens, a backup that is not sealed, or when `REPOMEMO_BACKUP_KEY_PREVIOUS` is not the key it was made with.

### Changed
- **A second copy of a backup that was sealed again is sent afresh:** the copy is trusted only when its manifest is the backup's own, so a copy made before the change is replaced instead of being counted as the new backup's copy.
- **The mismatch message** now names both variables.
- **The documents:** the production guide's account of changing the passphrase, the development commands (the new variable), the compose file and `.env.example`, the console guide, the API reference, the technical mindmap (routes, scope line, test counts: server 213, web 379) and the roadmap. The O6a gap "no rotation of old backups" is closed; the passphrase is still the only way back into a backup.
- **The version** is `0.3.24` everywhere.

### Checks
- **Compiled and linted:** `cargo clippy -D warnings` over the server crate with its tests, `cargo fmt --all --check`, `typecheck`, `types:check`, `container:check`, `docs:check`, `version:check`.
- **Tests written, not run:** unit tests of sealing a file again (every size, the new key only, a wrong key, a changed byte and a cut end failing and leaving nothing), of opening with either key and telling which, of sealing a whole backup folder again, and of a second copy sent afresh; a server test over the router (a backup made on the old passphrase given to a server on the new one: shut without the old one, listed as previous and restored with it, sealed again, no longer needing it, and refused a second time) and web tests of the badges and the action.

### Upgrading
- Nothing to do unless you change the passphrase. To change it: set `REPOMEMO_BACKUP_KEY` to the new one and `REPOMEMO_BACKUP_KEY_PREVIOUS` to the old one, restart, seal the backups you keep again, then remove `REPOMEMO_BACKUP_KEY_PREVIOUS`.

## V0.3.23 (2026-10-24)

The container is tried in CI, and built for arm64.

### Added
- **`docker compose up` in CI:** a `compose` job writes a `.env` with throwaway secrets, builds and starts the stack with `docker compose up --build --detach --wait` (so the server must turn healthy by its own health check), asks `/health/ready` through the proxy (the certificate Caddy makes for `localhost` is not checked), prints the stack's logs when it fails and takes the stack down with its volumes.
- **Images for arm64:** an `images-arm64` job builds the `server` and `proxy` stages for `linux/arm64` on GitHub's arm64 runner, so nothing is emulated, checks that both images are arm64 ones and that the server image has its git. Nothing is pushed anywhere: there is still no published image or registry.

### Changed
- **`container:check` insists on both:** it refuses a CI file without the compose job (the build with `--wait`, the request for `/health/ready` through the proxy, taking the stack down) or without arm64 builds of the two stages on an arm64 runner. It still reads files only and needs no Docker.
- **No new action is used:** both jobs run `docker` and `curl` on the runners themselves, with the `checkout` action already pinned to a commit.
- **The documents:** the production guide's account of `container:check`, the development commands' account of CI, the technical mindmap's scope line and the roadmap. The gaps "no `arm64` job" and "no `docker compose` test in CI" are closed; the images are still never built by the sessions that write the code, so the first run of the two jobs is the maintainer's to watch.
- **The version** is `0.3.23` everywhere.

### Checks
- **Run:** `container:check` with the new assertions, `docs:check`, `version:check`, `migrations:check`.
- **Not run:** the two jobs themselves (they need Docker and GitHub's runners) and the tests; the arm64 job needs a runner type that GitHub offers free for public repositories and to paid plans for private ones.

### Upgrading
- Nothing to do. If the repository is private on a plan without arm64 runners, remove the `images-arm64` job or give it a runner label of your own; `container:check` then names what is missing.

## V0.3.22 (2026-10-24)

Webhook deliveries are listed and can be sent again.

### Added
- **A list of the calls a webhook received:** each call to a repository's or a connector's webhook that was signed right is kept with when it came, the sender (GitHub, GitLab or another), the event, the sender's own delivery id and what became of it: `queued` (a sync was queued), `absorbed` (one was already running), `ignored` (an event or branch that asks for nothing, with the reason), `pong` (a ping) or `failed` (the sync could not be queued, with the reason: the tracker's pause, no room in the allowance). The newest hundred of a source are kept, and they go with the source. Migration 0043.
- **Send again:** a writer can send a call that asked for a sync (queued, absorbed or failed) again from the webhook's dialog in *Settings › Repositories* and *Settings › Issues and pull requests*. The same sync is asked for once more, whatever came of it the first time; the answer says whether it was queued, and the resend is kept as a call of its own (sender "sent again from this page") while the original shows when it was sent again. A ping or an ignored event asks for nothing, so it has no button.
- **Routes:** `GET /v1/connectors/{id}/webhook/deliveries`, `POST /v1/connectors/{id}/webhook/deliveries/{delivery}/resend` and the same two for `/v1/repositories/{id}`, for people who can write to the workspace.

### Changed
- **Only calls signed right are kept:** a call that cannot be verified says nothing about the source and is not recorded, and neither is a repeat of a delivery id the source already received within the hour, so a captured call cannot push the real ones out of the list.
- **Resends are limited** to ten a minute per source.
- **The documents:** the API reference and the technical mindmap (the routes, the migration, the scope line, test counts: storage 47, web 376), the functional mindmap, the user guide and the roadmap. The F17e gap "webhook deliveries are not listed or redelivered from RepoMemo" is closed.
- **The version** is `0.3.22` everywhere.

### Checks
- **Compiled and linted:** `cargo clippy -D warnings` over the storage and server crates with their tests, `cargo fmt --all --check`, `typecheck`, `types:check`, `styles:check`, `a11y:check`, `migrations:check`, `docs:check`, `version:check`.
- **Tests written, not run:** a storage test (newest first, one source's own, clipped text, the newest hundred, resent marks, removal with the source), the connector webhook test over the router (the calls and their outcomes, a ping and an unknown call refused, a resend, a failure while the tracker has the connector paused, its resend failing alike and going through once the pause is over, a stranger refused), the repository push webhook test (seven calls kept, none for the repeat, a resend, a member reading the list) and web tests (the sentences, who gets the button, the list, an empty list, a resend and its toasts).

### Upgrading
- Nothing to do. The migration adds one table; calls received before this release are not in the list.

## V0.3.21 (2026-10-24)

References between issues and pull requests, and the approvals of a pull request.

### Added
- **References between items:** the text of a connected issue or pull request (description, comments, reviews, commits) can refer to another item, and the reference is kept as a mention. `#12` means the item with that number in the same repository (on GitHub an issue or a pull request, on GitLab an issue), `!12` a GitLab merge request, `owner/repo#12` an item of another connected repository (also written after a host) and the address of an item, as the tracker shows it, the item itself. An issue's page lists the pull requests that refer to it under *Mentioned in*, and the pull request lists it under *Mentions*, both as "refers to #12".
- **Approvals:** a pull request keeps who approved it and who asked for changes (each reviewer's latest word; comments change nothing and a dismissed review takes it back) in its data (`approved_by`, `changes_requested_by`), in a line of its text ("Review: approved by @ana · changes requested by @bo", so search and citations see it) and on its page. GitLab merge requests get theirs from GitLab's approvals.

### Changed
- **Nothing is guessed:** a reference is kept only when exactly one item has that number in that repository; a number nobody has, a name without an owner (`other#4`), a colour (`#123456` where no item has that number), `page#12`, `#12abc` and an item's reference to itself are left alone. The references are worked out again, with the file mentions, after a sync that changed something, so the order the items arrive in does not matter.
- **GitLab approvals are optional:** if a project does not offer them or the tracker refuses the request, the merge request is stored without approvals and the connector is not paused.
- **The documents:** the technical mindmap (the mentions route, the scope line, test counts: api 105, web 370), the functional mindmap, the user guide and the roadmap. The F17d gap about `#12` links is closed; mentions in comments written inside RepoMemo are still not read.
- **The version** is `0.3.21` everywhere.

### Checks
- **Compiled and linted:** `cargo clippy -D warnings` over the api and server crates with their tests, `cargo fmt --all --check`, `typecheck`, `types:check`, `docs:check`, `version:check`.
- **Tests written, not run:** unit tests of the references (a bare number, never the item itself, another repository, GitLab's `#` and `!`, addresses, and prose that is not a reference) and of the approvals (latest word, comments, dismissal, the text line); the server tests for GitHub and GitLab now also expect the approvals in the data, and the mentions test a pull request that says "Fixes #1" listed on the issue and linking to it; web tests of the review sentence and of the "refers to" mentions.

### Upgrading
- Nothing to do. Items already stored get their references at the next sync that changes something, and their approvals when the tracker next reports a change to the pull request.

## V0.3.20 (2026-10-24)

Import runs as a background job with a progress bar.

### Added
- **Background import:** `POST /v1/workspaces/import` takes `background=true`. The request carries the archive as before; once it is received, the server answers `202` with `{"job_id": …}` and does the checking and the writing on its own. `GET /v1/workspaces/import/jobs/{job_id}` gives its `status` (`running`, `done`, `failed`), its `stage` (`checking`, `storing_files`, `writing_records`, `finished`), `progress_current` and `progress_total` (bytes of stored files), then the `report` or the `error`. A failed import leaves nothing behind, as before.
- **A progress bar** under *Import a workspace* on the dashboard: sending the archive, checking it, storing the files with the share done, writing the workspace. Both the check and the import show it.

### Changed
- **The browser no longer holds the archive:** the file chosen is sent as the request body, which the browser reads from disk as it sends; the page keeps nothing of it. Closing the page does not stop an import that has begun on the server.
- **Limits:** a person has one import running at a time (`409` with the reason otherwise) and the server two (`429`). Only the person who started a job, or a system administrator, can read it (`404` for anyone else); a finished job is kept an hour, and a restart of the server ends the jobs in memory without creating anything.
- **Without `background`** the request behaves as before, so scripts and older clients are unaffected; the web client also still understands an older server that answers the request with the result.
- **The documents:** the API reference, both mindmaps (the route, the scope line, test counts: server 208, web 367), the user guide and the roadmap; the O2 gaps "no progress while a large archive uploads" and "no background import job" are closed, except that the bytes sent are not counted (the bar is indeterminate while the file travels).
- **The version** is `0.3.20` everywhere.

### Checks
- **Compiled and linted:** `cargo clippy -D warnings` over the server crate with its tests, `cargo fmt --all --check`, `typecheck`, `types:check`, `styles:check`, `a11y:check`, `docs:check`, `version:check`.
- **Tests written, not run:** two unit tests of the job registry and its progress (one import per person and two per server, owner-only reads, finishing frees the room, stages and the bar), a server test over the route (a dry run and a real import as jobs, a stranger and an unknown id refused, a bad archive failing the job with its reason and creating nothing, the request without `background` unchanged) and web tests (the stages followed to the report, the dry run flag, the failure and lost-job messages, the answer of an older server, the refusal, the stage sentences, and the bar shown and removed).

### Upgrading
- Nothing to do. If a proxy sits in front of the server, it need not change: the archive still travels in one request, now answered sooner.

## V0.3.19 (2026-10-24)

Archives carry earlier file versions and saved searches.

### Added
- **Earlier versions of files:** a workspace's archive now holds, for each uploaded file that took new versions, the content of every earlier version (each stored once under `blobs/`) and its record (number, size, type, when it became current and when it was replaced). An import writes them back with the same numbers and dates, so *Version history* shows the same list, each version readable and comparable. The size of the earlier versions counts in the archive and against the importing workspace's allowance like any other file.
- **Saved searches:** the archive carries them (name, words, kinds, languages, the sources they keep to, the limit). An import puts them in the new workspace under the person who imports, with the sources replaced by the new ones; a source the archive does not hold is dropped from the search.
- **The import report** says how many earlier versions and saved searches an archive holds (the dry run too).

### Changed
- **Archive format 2.** `format_version` is now 2 and `workspace.json` has `versions` and `saved_searches` (optional when reading, so an archive of format 1 still imports, with neither); a server that only reads format 1 refuses a format 2 archive with its usual message, rather than importing it without its history. The manifest's counts gain `versions` and `saved_searches`.
- **Nobody is credited by account,** as for every other record: the person who uploaded each later version is kept as a name in the archive and not written back (the import has no account to give it to), so an imported history shows no uploader.
- **Checks on the way in:** an earlier version of a file the archive does not hold is left out with a warning; a version number below 1, listed twice or with a bad content hash is a problem; every stored file a version names must be listed in the manifest and present in the archive, with the checksum it is named by.
- **The documents:** the functional mindmap's archive row, the technical mindmap's test counts (storage 46, server 205, web 360). The O2 gap about versions and searches is closed; assistant chats, notifications, the activity feed, secrets and the links to a repository or a tracker are still not carried.
- **The version** is `0.3.19` everywhere.

### Checks
- **Compiled and linted:** `cargo clippy -D warnings` over the domain, storage, api and server crates with their tests, `cargo fmt --all --check`, `typecheck`, `types:check`, `docs:check`, `version:check`.
- **Tests written, not run:** a storage test (versions and searches exported, their blobs named among the stored files, an import restoring the numbers, hashes and contents, crediting nobody, moving the search to the importer and to the new source, and an archive of format 1 still importing), a server test over the whole route (a file with three editions and a search, the manifest and counts, a dry run, the import, the history and the texts and the search in the new workspace) and a web test of the report sentence; the existing export test now expects format 2.

### Upgrading
- Nothing to do. Archives made by earlier releases import as before; archives made now are format 2, which only 0.3.19 and later read.

## V0.3.18 (2026-10-24)

The Overview cards the templates wanted and could not name.

### Added
- **Four pulse cards** a template can list in `overview.cards` (now 15 ids; one to eight per template, as before):
  - `repositories_in_sync`: linked repositories whose last sync finished, *of N linked*.
  - `reviewed_90_days`: files whose lifecycle was reviewed in the last 90 days, *of N files*.
  - `needing_review`: files marked *needs review* plus memory cards gone stale, *N files, M memory cards*.
  - `duplicates`: files that repeat the exact content of another file (the second and later copies; a repository's own overview is not counted).
- **Six figures in `GET /v1/workspaces/{id}/metrics`** carry them: `repository_count`, `repositories_in_sync`, `reviewed_last_90_days`, `needing_review_files`, `needing_review_cards`, `duplicate_file_count`. The page already loads this answer, so choosing a card still asks the server for nothing more.

### Changed
- **General's nine cards are unchanged**, and so are the four built-in templates, which keep the figures they chose; the four new cards appear only where a template names them. The template checker (`templates:check`) and the authoring guide know the 15 ids. A client that cannot draw an id still ignores it.
- **The documents:** the authoring guide, the templates page, the functional mindmap's gap row, the technical mindmap's metrics route and test counts (server 204, web 359). The V0.2.174 gap is closed for these four; documents by kind is still not a card.
- **The version** is `0.3.18` everywhere.

### Checks
- **Compiled and linted:** `cargo clippy -D warnings` over the templates, storage and server crates with their tests, `cargo fmt --all --check`, `templates:check`, `typecheck`, `docs:check`, `version:check`.
- **Tests written, not run:** a server test (an empty workspace has none of the six; three notes of which two repeat each other, one marked for review, one verified and a synced local repository give the repositories, the file needing review, the review of the day and the single duplicate) and a web test of the four cards (figures, notes, the dash before data, the template's list, and that none is a default).

### Upgrading
- Nothing to do. No migration; the figures are read from tables that exist.

## V0.3.17 (2026-10-24)

More code symbols.

### Added
- **Five kinds of symbol** (`constant`, `variable`, `field`, `property`, `type_alias`), found by the Code symbols box and the assistant's symbol lookup like functions and classes:
  - **Go:** package-level `const` and `var` (each name of `const Read, Write = 1, 2`, inside or outside a block), and named types that are neither a struct nor an interface (`type ID int`, `type Handler func()`) and aliases (`type Old = Handler`). Anything inside a function is left out.
  - **Java:** fields of a class, one for each name (`int a, b;`); locals are not fields.
  - **C#:** properties, including expression-bodied ones.
  - **PHP:** properties (`$count` is the symbol `count`, one for each in `$a = 0, $b = 1`) and constants, global and in a class.
- **`INDEXER_VERSION` 5:** stored indexes of Go, Java, C# and PHP files are refreshed in the background after the upgrade (chunks whose text is unchanged keep their identity, and repository syncs index again what is out of date). Nothing else is re-read.

### Changed
- A Go `type ID int` was left out on purpose until now (the test said so); it is now a type alias. The agent's symbol answer prints the kind, which now may be `Constant`, `Variable`, `Field`, `Property` or `TypeAlias`.
- **The documents:** the functional mindmap's symbols line, the technical mindmap's index versions and test count (indexer 28), the user guide. The V0.2.35 and V0.2.36 gaps about these symbols are closed; Java nested types of a method body, C# fields, local functions and delegates, and Razor and Blade are still not symbols.
- **The version** is `0.3.17` everywhere.

### Checks
- **Compiled and linted:** `cargo clippy -D warnings` over the whole workspace with its tests, `cargo fmt --all --check`, `types:check` (the symbol kind type regenerated), `typecheck`, `docs:check`, `version:check`. The shapes of the grammars' nodes were read from their `node-types.json`.
- **Tests written, not run:** four in the indexer (Go constants, variables and aliases, and that locals are skipped; Java fields; C# properties; PHP properties and constants without the `$`) and the existing Go test changed, which asserted that `ID` was not a symbol.

### Upgrading
- Nothing to do. After the upgrade the server indexes the Go, Java, C# and PHP files again in the background; the Code symbols box fills as it goes.

## V0.3.16 (2026-10-24)

Answers and citations name the last author and date of a repository file.

### Added
- **Beside each citation:** in an answer to a question, in the assistant, in a file summary and in the workspace overview, a citation of a repository file reads *changed by Ada on Oct 1, 2026* next to its path. A citation of a note, an upload or an answer saved before this release says nothing.
- **Where it comes from:** the citation carries `last_author` and `last_changed_at` (both optional, absent from older saved answers), filled from the search result that was cited (the same data as the file list and the search results since V0.2.24) or, for a summary of one file or of a workspace, from what the last repository sync recorded in that file's metadata.

### Security
- Nothing new is stored or fetched. The author's name and the date are **not** added to the text sent to the AI provider (the passages and the prompt are unchanged), so a cloud provider learns nothing it did not already receive; they are shown only to people who can read the workspace, as in the file list.

### Checks
- **Compiled and linted:** `cargo clippy -D warnings` over the whole workspace with its tests, `cargo fmt --all --check`, `typecheck`, `types:check` (153 types), `styles:check`, `a11y:check`, `docs:check`, `version:check`.
- **Tests written, not run:** a unit test of reading the recorded author and date from a file's metadata, and three web tests (the phrase with an author and a date, with one of them, with neither or a date that cannot be read; the evidence list showing it for a repository file and nothing for a note).

### Changed
- **The documents:** the functional mindmap's gap row, the technical mindmap's test counts (api 100, web 358), the user guide. The V0.2.24 gap about answers and citations is closed. An answer's own text still does not mention the author.
- **The version** is `0.3.16` everywhere.

### Upgrading
- Nothing to do. No migration.

## V0.3.15 (2026-10-24)

`CODEOWNERS` owners matched to workspace members.

### Added
- **Matching:** an owner that `CODEOWNERS` writes as an address (`ada@example.com`) shows on the file's page as the member whose account has that address, ignoring case. A `@handle`, a `@org/team`, an address nobody has and an anonymized account are never matched: handles and teams belong to the code host, so guessing (a handle that looks like a member's name) would put the wrong person on a file.
- **Asking for a review:** beside a matched owner, a person who can change the file sees *Ask to review*. It assigns that member a task *Review <path>* for the file, which reaches them like any assignment (the app, and mail as they chose, under their *task assigned* preference) and shows in Tasks and the activity feed. The same person is asked once while the task is open; once it is done they can be asked again, with an optional note.
- **`GET /v1/artifacts/{id}/code-owners`** (anyone who can read the file) answers each owner with its member or none; **`POST /v1/artifacts/{id}/review-requests`** takes `{user_id, note?}`.

### Security
- Asking is open only to people who can change the file, and only for a member that the file's own CODEOWNERS names: the server recomputes the matches and refuses any other `user_id` (`400`), so the route cannot be used to assign work to an arbitrary member. A note is limited to 1,000 characters. The member's name is shown to readers of the workspace, who already see members in comments and activity; their address is the owner text the repository itself published.

### Checks
- **Compiled and linted:** `cargo clippy -D warnings` over the domain, storage, api and server crates with their tests, `cargo fmt --all --check`, `typecheck`, `types:check` (153 types), `styles:check`, `a11y:check` (124 components), `docs:check`, `version:check`.
- **Tests written, not run:** a unit test of the matching (case, handles, teams, strangers, an account with no address), a server test over a real local repository whose `CODEOWNERS` names a member, a handle, a team and a stranger (what is matched, a file of handles only, an outsider refused, a viewer and an outsider cannot ask, an unnamed member and a bad id and an over-long note are refused, the task and the member's notice, asking again is refused while open and allowed once done), and six web tests (nothing without owners, hints then the member, the server failing, no button for a reader, asking once, a refusal).

### Changed
- **The documents:** the functional mindmap's gap row, the repository sources page, the API reference, the technical mindmap's routes and test counts (api 99, server 203, web 355), the user guide. The V0.2.25 gap about matching is closed; owners in the file list and in search results are not shown.
- **The version** is `0.3.15` everywhere.

### Upgrading
- Nothing to do. No migration; owners are read from what the last sync stored.

## V0.3.14 (2026-10-24)

*View at commit* from memory cards and repository summaries.

### Added
- **On a memory card:** each linked file of a git repository whose citation named a commit shows *View at `0123456`*, which opens the cited text as it was at that commit, labelled as such, with the cited lines marked and a note when it is not the current version. A card made from an assistant answer keeps the commit the answer named; evidence picked by hand names none and shows no button.
- **On a repository's page:** each file the AI summary is based on offers the same button, at the commit the summary describes.
- **How a card remembers it:** the commit is kept in the link's own `metadata_json` (no migration), only when it looks like a commit id (7 to 64 hexadecimal characters, stored in lower case), read back as `MemoryEvidence.commit`, and sent again when a card is edited so saving a card does not lose it.

### Security
- The commit is data the card's writer supplied, shown and sent to the existing `GET /v1/artifacts/{id}/at/{commit}`, which reads the file of that artifact in its repository's own copy under the reader's workspace rights; anything that is not a hexadecimal id is dropped before it is stored.

### Checks
- **Compiled and linted:** `cargo clippy -D warnings` over the domain, storage, api and server crates with their tests, `cargo fmt --all --check`, `typecheck`, `types:check` (152 types), `styles:check`, `a11y:check`, `docs:check`, `version:check`.
- **Tests written, not run:** a storage test (a named commit kept in lower case and read back, kept when the citation says it again, none stored when it is left out or is not a commit) and three web tests of the button (nothing without a commit, the label and the text with its lines marked and the current-version note, the error). The two pages' wiring is not covered by a rendering test.

### Changed
- **The documents:** the functional mindmap's gap row, the technical mindmap's route row and test counts (storage 45, web 349). The V0.2.26 gap about cards and summaries is closed.
- **The version** is `0.3.14` everywhere.

### Upgrading
- Nothing to do. A card made before this release names no commit, so it shows no button until its evidence is cited from an answer again.

## V0.3.13 (2026-10-24)

The command palette keeps recent places and offers *New note*.

### Added
- **Recent places:** the palette lists the last places a person opened from it (up to eight, newest first) as a *Recent* group while nothing is typed: files, memory cards, workspace sections and workspaces. They are kept in the browser, so they survive a reload, and for that account only (the key carries the account id), and a place is listed only while its workspace is still one the account can read.
- **New note:** in the open workspace, for anyone who can add evidence (not a viewer), *New note* opens the Evidence section with the note form scrolled into view and its title field focused. The request is held until the page has mounted, taken once, and only for its own workspace.

### Security
- What is read back from the browser's storage is treated as untrusted: an entry must be a plain path of the app (no scheme, no `//`, only path characters) of a bounded size, or it is dropped; the palette navigates only through the app's router. Nothing is sent to the server, and signing in as someone else on the same browser shows none of the earlier account's places.

### Checks
- **Compiled and checked:** `typecheck`, `styles:check`, `a11y:check`, `docs:check`, `version:check`. No Rust changed.
- **Tests written, not run:** a new file for the recents (order, once each, the cap, a reload, one account's own, refusal of addresses that are not app paths and of damaged storage) and six in the palette's tests (the *New note* entry and who gets it, recents first and only for readable workspaces, remembering a chosen file across a new page and per account, opening the note editor from *New note*).

### Changed
- **The documents:** the functional mindmap's palette row, the technical mindmap's palette row and web test count (346), the user guide. The V0.2.34 gap about recents and creating is closed.
- **The version** is `0.3.13` everywhere.

### Upgrading
- Nothing to do.

## V0.3.12 (2026-10-24)

Search across workspaces takes filters and finds memory cards.

### Added
- **Filters on the dashboard search:** a kind of file, a language, and the days evidence was updated from and to. The server passes them to each workspace's own search, so every result is still read under that workspace's role and AI policy.
- **Memory cards:** the search also looks in the memory cards of the workspaces the caller can read, by keywords (at most ten in all, in the order the workspaces were searched), and lists them under the files with their workspace, an excerpt and whether they need review; each opens its card. A card is not a file, so a kind of file or a language leaves cards out, and the days filter applies to when a card was last written. An *Include memory cards* box turns them off.
- **`POST /v1/search`** takes `artifact_types`, `languages`, `modified_after`, `modified_before` (`YYYY-MM-DD`; another form is `400`) and `memory_cards`, and answers a new `memory_cards` list; a client that sends none of them gets what it got before, plus the cards.

### Security
- Nothing new is reached: cards are looked up only in workspaces the caller's role already lets them read (the same check that admits their files), by keywords, so a workspace's AI policy has nothing to keep back; a workspace the caller cannot read contributes neither files nor cards.

### Checks
- **Compiled and linted:** `cargo clippy -D warnings` over the domain and server crates with their tests, `cargo fmt --all --check`, `typecheck`, `types:check` (152 types), `styles:check`, `a11y:check`, `docs:check`, `version:check`.
- **Tests written, not run:** a server test (files and cards of readable workspaces only, a language, a kind nobody has, days from the future and up to the past, today, cards off, a bad day refused, a named workspace) and four web tests (files and cards with no filter, opening each, the language and days sent, cards left out for a language or when turned off).

### Changed
- **The documents:** the functional mindmap's gap row, the technical mindmap's route and test counts (server 202, web 337), the user guide. The V0.2.33 gap about filters and memory cards is closed.
- **The version** is `0.3.12` everywhere.

### Upgrading
- Nothing to do.

## V0.3.11 (2026-10-24)

Assistant lookups and searches read the turn before.

### Changed
- **"Search for it" means something:** a file lookup, a symbol lookup or a content search whose only subject is a reference ("search for it", "find that file", "look for those", "where is it defined?") is about what the latest earlier turn looked for. After "Find the file about login", "search for it" searches for *login*; the reply's subject says so. The earlier turn's subject is read by the router's own rules (a lookup, a symbol, a search, a summarized file or the question itself); turns that name nothing (an overview, another "search for it") are skipped, a turn that listed a file falls back to its title, and a message that names its subject itself, points at nothing, or has no earlier turn to read is left as typed.
- **Where it applies:** the rules, the template's phrases, the model's routing and an explicit capability all end in the same step, so the subject is resolved after routing and before the lookup runs. The model's own classification still reads a message alone.
- **No new setting, route or migration.** The history is the one the server already builds from the stored conversation (V0.2.32).
- **The documents:** the functional mindmap's gap row and the technical mindmap's conversation memory (test counts: api 98, server 201), the user guide. The V0.2.32 gap about lookups is closed.
- **The version** is `0.3.11` everywhere.

### Checks
- **Compiled and linted:** `cargo clippy -D warnings` over the api and server crates with their tests, `cargo fmt --all --check`, `docs:check`, `version:check`.
- **Tests written, not run:** two unit tests (what a pointing lookup resolves to for each kind of earlier turn, layered turns, and what is left as typed) and a server test (after "Find the file about login", "search for it" finds the login files and not the deploy runbook; the same words in a chat of their own are searched as typed; a message that names its subject keeps it).

### Upgrading
- Nothing to do.

## V0.3.10 (2026-10-24)

A count in the Review tab and a snooze for an item.

### Added
- **A count on the Review tab:** the number of items waiting for the person, shown when there are any (and as *Review (n)* under *More*). It is read again every minute, shares its answer with the Review page, and follows the page when an item is snoozed or brought back.
- **Snooze:** each item of the review queue has a *Snooze* choice (until tomorrow, for a week, for a month). The item leaves the person's list and the count and comes back by itself at the start of that local day. A snooze is the person's own: a colleague's list does not change, and anyone who can read the workspace can snooze.
- **A *Snoozed* list** on the page, with the day each comes back and a *Bring back* button for an immediate return.
- **`PUT` and `DELETE /v1/workspaces/{id}/review/snoozes`:** `PUT {kind, id, until}` (a date such as `2026-11-01`, read as the start of that UTC day, or an RFC 3339 time; the web client sends the start of the chosen local day), `DELETE ?kind=&id=` to bring back; both `204`. `GET …/review` leaves the person's snoozed items out of `items` and of the counts and lists them in `snoozed`, each with `snoozed_until`.
- **Migration 0042 `review_snoozes`** `(user, workspace, kind, id) → until`: deleted with the person or the workspace; expired rows are ignored by reads and cleared when the person snoozes again.

### Security
- Only the caller's own rows are read or written; the route takes a kind from a closed list and an id of at most 200 characters without control characters, so nothing else reaches the table. A time in the past or more than 366 days ahead is refused, and a person holds at most 500 snoozes in a workspace, so the table cannot be grown without bound. Reading the queue is the right needed, as nothing in anyone else's view changes.

### Checks
- **Compiled and linted:** `cargo clippy -D warnings` over the whole workspace with its tests, `cargo fmt --all --check`, `typecheck`, `types:check` (types regenerated), `styles:check`, `a11y:check`, `migrations:check` (42 files, 0042 locked at 0.3.10), `docs:check`, `version:check`.
- **Tests written, not run:** a storage test (kept per person and workspace, soonest first, snoozing again moves the date, an expired row is not held and is cleared, bringing back, removal with the workspace), a server test (a snoozed item leaves the list and the count and is listed with its date, a colleague still sees it, a past date, more than a year, nonsense, another kind and an outsider are refused, bringing it back), two unit tests (`split_snoozed`, `parse_snooze_until`), and five web tests (the count on the tab and none for zero, snoozing through the list with the day computed from the local start of the day and the answer shared with the tab, bringing back, a refused snooze, the day arithmetic).

### Changed
- **The documents:** the API reference, the functional mindmap's review section, the technical mindmap's routes, migration table and test counts (storage 44, server 200, web 333), the user guide. The V0.2.37 gap about a count and snoozing is closed.
- **The version** is `0.3.10` everywhere.

### Upgrading
- Migration 0042 runs by itself on start. Nothing else to do.

## V0.3.9 (2026-10-24)

`@mentions` in task descriptions and memory cards.

### Added
- **Mentions beyond comments:** an `@address` in a task's description (created or saved) or in a memory card's text notifies that member, like in a comment. The notice is *You were mentioned in a task* (type `task_mention`, leads to the task board) or *You were mentioned in team memory* (type `memory_mention`, leads to the card), in the app and by mail as the person chose.
- **Suggestions while typing:** the task description and the memory card fields (new card and edit) are `MentionTextarea`s, so typing `@` suggests the workspace's members, as in a comment.

### Changed
- **Who is told:** direct members of the workspace only. An organization member who is not in the workspace is neither suggested (as before) nor notified; the author is never told; on a save, only addresses that were **not** in the text before are told, so editing a description does not tell everyone again.
- **One preference:** the choosable kind *You are mentioned in a comment, a task or a memory card* (`evidence_mention`) covers all three; nothing new to set, no migration.
- **Code:** `notify_artifact_mentions` is now a thin call of `notify_mentions`, which takes a `MentionedIn` place (a file, a task or a memory card) and the text before the save; `mentioned_addresses` reads the addresses.
- **The documents:** the functional mindmap's notifications row, the technical mindmap's routes and test counts (server 197), the user guide. The V0.2.39 gap about tasks and cards is closed.
- **The version** is `0.3.9` everywhere.

### Checks
- **Compiled and linted:** `cargo clippy -D warnings` over the server crate with its tests, `cargo fmt --all --check`, `typecheck`, `docs:check`, `version:check`.
- **Tests written, not run:** a server test (a task naming a member, an organization member outside the workspace and its author tells only the member; saving again tells nobody and a new name tells only that person; a memory card tells the member, an edit that names nobody new is quiet; the notices lead to the task board and the card) and a unit test of the address reading. No new web test: `MentionTextarea` is unchanged and has its own.

### Upgrading
- Nothing to do.

## V0.3.8 (2026-10-24)

Restore an earlier version of a file.

### Added
- **`POST /v1/artifacts/{id}/versions/{n}/restore`:** makes version n the file's current content as a **new version**. The version it replaces is kept like after any upload, so restoring version 2 of three makes a fourth with version 2's text and the history keeps all four. The current version (`409`), an unknown one (`404`) and one whose stored content is missing (`409`) are refused; a viewer and an outsider are refused (`403`); a file from a repository is refused (its history is git's). The restored size is checked against the workspace's storage allowance (it is not counted as an upload); the activity feed says *Restored version n of …*; the file is converted and indexed again.
- **A *Restore* button** on each earlier version in the artifact page's *Version history*, for people who can change the file. It asks first (*Version 2 becomes version 4 …*), then restores, shows a success toast and reloads the page.

### Security
- Same rights as uploading a version (`require_workspace_write` through the artifact, a session or a token that may change content); the route takes no body and no content from the caller, only a version number of the file named in the address, and reuses a stored blob by its hash, so it cannot reach another file's content. The allowance check stops a restore from growing a full workspace.

### Checks
- **Compiled and linted:** `cargo clippy -D warnings` over the storage, api and server crates with their tests, `cargo fmt --all --check`, `npm run typecheck`, `docs:check`, `version:check`.
- **Tests written, not run:** the storage test extended (version 2 restored: four versions, the replaced content kept, the current and an unknown version refused, the allowance, no blob collected), a new server test (the same, over the API, with a viewer and an outsider refused, a second restore, and the restored text found by search), and four web tests (no button for a reader, a button for each earlier version, the confirmation before any call, a cancelled and a refused restore).

### Changed
- **The documents:** the API reference, the functional mindmap's version history, the technical mindmap's routes and test counts (server 195, web 329), and the user guide. The V0.2.38 gap about restoring is closed.
- **The version** is `0.3.8` everywhere.

### Upgrading
- Nothing to do. No migration.

## V0.3.7 (2026-10-24)

The documents say one client. Documents only; no code changed.

### Changed
- **The functional mindmap:** the paragraphs on the standalone workspace and its move are gone from the desktop-app section, the gaps table loses its row on it and a sentence on it, and the scope line says the app is a window onto a server.
- **The technical mindmap:** the rows for the desktop shell (eight commands in two files), the archive writer (no caller since V0.3.6), accessibility and styling checks (no exemption, 13 stylesheets of about 2,700 lines, 89 baseline cases) and the risk table; the "local edition styles" row is removed.
- **The other documents:** the user guide, `PRODUCT.md`, `README.md`, the clean-install page and `repository-sources.md` no longer describe a standalone mode. `docs/DEVELOPMENT_COMMANDS.md` already described only the server-client app. `UPGRADING.md` (*From 0.3.0 onward*, written in V0.3.5) names V0.3.4 as the last release that can move a local workspace.
- **Left as the record:** the changelog, `UPGRADING.md`, the roadmap's release logs and checklists, the release indexes, the older decision records and architecture plans, and the technical mindmap's scope line, which lists what each release did.
- **The version** is `0.3.7` everywhere.

### Checks
- **Passing:** `docs:check` (every link of 28 documents) and `version:check`.

### Upgrading
- Nothing to do. Track P (V0.3.5–V0.3.7) is complete.

## V0.3.6 (2026-10-24)

The desktop shell holds only what a client of a server needs: its address, its session and its window.

### Removed
- **The local commands:** the 26 Tauri commands that called `RepoMemoCore` in-process (`list_workspaces`, `import_paths`, `search_workspace`, `ask_workspace` and the rest), `AppState`, `RepoMemoCore::boot` at start and the shell's own data folder.
- **The move:** `handoff.rs` and its four commands (`export_workspace_archive`, `read_export`, `copy_export`, `discard_export`).
- **The dialog plugin:** nothing used it once the move was gone, so `tauri-plugin-dialog`, the `dialog:default` permission and `@tauri-apps/plugin-dialog` are removed too.
- **Dependencies:** `repomemo-api`, `repomemo-domain`, `chrono`, `tokio` and `tauri-plugin-dialog` leave the shell's `Cargo.toml` (and `tauri-plugin-dialog` the workspace's); `Cargo.lock` loses 47 lines, `package.json` and `package-lock.json` one package.

### Changed
- **The shell** is `lib.rs` (the HTTP plugin, the connection state, the window built with `on_navigation`, eight commands: `current_server`, `prepare_server`, `remember_server`, `forget_server`, `save_session`, `load_session`, `clear_session`, `open_external`) and `connection.rs`. `cargo tree -p repomemo-desktop` reaches no `repomemo-*` crate.
- **The documents:** the technical mindmap's shell row and scope line; the roadmap's checklist.
- **The version** is `0.3.6` everywhere.

### Checks
- **Passing:** `cargo check -p repomemo-desktop --tests` (the shell's tests included), `cargo fmt --all --check`, `version:check` and `docs:check`.
- **Not run:** the tests and the app. `RepoMemoCore::export_workspace_archive` and `write_workspace_archive` in `repomemo-api` stay (the move was their only caller; they are tested and harmless, and are the first candidates if a later release prunes dead code).

### Upgrading
- Nothing to do for a server. The desktop app no longer creates or opens a local data folder; an old one is left on the computer untouched.

## V0.3.5 (2026-10-24)

The local edition is removed from the desktop client: the app opens on the server address screen only.

### Removed
- **The local app:** `LocalDesktopApp` (`App.tsx`, 2,757 lines), `lib/repomemoApi.ts` (607 lines), `LocalMoveToServer.tsx`, `lib/handoff.ts` and the *Use the local workspace* button of the first screen. `App.tsx` is now twelve lines: the browser opens the web client, and the desktop window opens `DesktopApp`.
- **The move's traces:** the notice of a workspace "ready to move" on the address screen, and the prepared-archive path of *Import a workspace* on the dashboard (it still imports any archive a person chooses, including one made earlier by the move).
- **Styles:** every rule that only the local app's classes selected (about 870 rules, and 120 selectors trimmed from shared rules), the empty `08-evidence-instrument-tail.css`, two unused keyframes and the `.rm-desktop-local`, `.rm-desktop-retired` and `.shared-import-handoff` rules: 4,785 lines of CSS in all. `styles:check` and `a11y:check` lose their exemptions for the local edition; the style baseline shrinks from 111 to 89 older cases.
- **Tests:** the nine tests of the move (`desktopHandoff.test.tsx`); the desktop test about the retired workspace is replaced by one that checks the address screen opens and the two old storage keys are forgotten.

### Changed
- **`DesktopApp`** opens the web client on the saved server or `DesktopConnect`, and removes the two storage keys the local app left (`repomemo.desktop.mode`, `repomemo.desktop.handoff`) at its next start.
- **The Tauri shell is unchanged:** its 26 local commands, the move's four and the `repomemo-api` dependency go in V0.3.6, so nothing calls them now.
- **The documents:** the functional mindmap's desktop section and gaps row say the local workspace is gone, and the technical mindmap's frontend, accessibility, styles and risk rows say so; the full pass over the documents is V0.3.7.
- **The version** is `0.3.5` everywhere.

### Checks
- **Passing:** `npm run typecheck`, `styles:check`, `a11y:check` (123 components), `version:check` and `docs:check`.
- **Not run:** the web tests (as asked) and the app itself.

### Upgrading
- A desktop window that had chosen the local workspace opens on the address screen. The data of a local workspace stays in its folder on the computer, untouched but no longer reachable from the app; V0.3.4 was the last release that could move one to a server. See UPGRADING.md.

## V0.3.4 (2026-10-24)

The record of the clean install, the performance runs and the browser pass prepared for the maintainer to fill. Documents only; no code changed, and no figure was invented.

### Changed
- **The clean-install page** ([clean-install.md](mindmap/technical/clean-install.md)): its result table records the maintainer's run of 2026-10-24 (reported good, per-step results not written down) and a row for the next run; §3 says, for every box, why it is still open and what closes it (CI on the host, the desktop crate's tests, release-build runs, a browser, a second computer, Docker); the browser pass has a record sheet (eight page groups by axe or Lighthouse, 375 px and 768 px in both themes, and keyboard only); and the open boxes of every 0.2.x checklist are triaged into those moved to a numbered 0.3.x release, those that are the maintainer's to try, and those accepted as stated. The box "the open items are read once more" is ticked, with that table as the proof.
- **The performance page** ([performance.md](mindmap/technical/performance.md)): a record sheet to compare *before track M* (`7291584`, V0.2.148), *after track M* (`457d094`, V0.2.162) and *now*, on one machine, with the machine's fields and one row for each measure of the targets, including the two rows that only exist from track M.
- **The version** is `0.3.4` everywhere.

### Checks
- **Passing:** `version:check`, `docs:check`, and the static `a11y:check` (122 components, 94 colour pairs), which the sheet names as not being a browser pass.
- **Not done, on purpose:** the performance runs (release builds), the browser pass and the desktop try, which need a running server or app; the roadmap's *Done when* for this release ("no box is left unticked without a reason") holds, and the figures are the maintainer's to enter.

### Upgrading
- Nothing to do.

## V0.3.3 (2026-10-24)

The code formatted and clippy made clean in one release, so the two steps that CI already blocks on pass on `main`. No behaviour changed.

### Changed
- **Formatting:** `cargo fmt --all` rewrote 45 files, hunks only (1,638 lines added and 529 removed over the whole workspace); line endings are kept (rustfmt's `newline_style` is `Auto`, so CRLF files stay CRLF), and a second `cargo fmt --all --check` changes nothing.
- **Clippy** (`cargo clippy --workspace --exclude repomemo-desktop --all-targets --locked -- -D warnings`) passed after these corrections, none of which changes what the code does: `map_or(true, …)` written as `is_none_or(…)` (the access, agent, meaning, backup, logs, security and streams code); `x % n == 0` written as `is_multiple_of` (the repository sync's progress and the embedding decoder); two redundant closures dropped in the workspace export; a type alias for a cached template answer and for a search outcome; `too_many_arguments` allowed on the one function that stores a repository blob; the test module of `review.rs` moved to the end of the file; and three test assertions written as `assert!(…)` and `is_empty()`.
- **CI** needed no change: the *rustfmt and clippy* job already blocks on both, and now has a tree to pass on.
- **The version** is `0.3.3` everywhere.

### Checks
- **Passing:** `cargo fmt --all --check`, `cargo clippy` as above (every crate, tests included), `version:check` and `docs:check`.
- **Not run:** the tests, as asked; the lint corrections are the sort a test would catch, so the maintainer's run of V0.3.2 should be made on this tree.

### Upgrading
- Nothing to do.

## V0.3.2 (2026-10-24)

Every test written since V0.2.46 compiled and handed over for the one run they have never had. No code changed in this release.

### Checks
- **Compiled:** `cargo check --workspace --exclude repomemo-desktop --tests --locked` finishes with no error or warning, so every Rust test of every crate and of the server builds. `npm run typecheck` passes, and so do the web tests' types.
- **Counted from the source** (test attributes and `it(` calls): 547 Rust tests (ai 12, api 96, domain 3, git 15, indexer 24, ingestion 21, mail 15, retrieval 21, storage 43, templates 89, server 194, console 14), 334 web tests and 9 for the template checker.
- **Not run, as asked:** the tests themselves. Nothing was started or built beyond the compile.

### For the maintainer
- Run once, in this order, and send back what fails: `cargo test --workspace --exclude repomemo-desktop`, `npm run test:web` and `npm run templates:test`. A failing test is corrected in the test or in the code and takes the next number if it needs a release of its own.

### Changed
- **The version** is `0.3.2` everywhere. The roadmap's *Done when* for this release (both commands pass) waits for that run.

### Upgrading
- Nothing to do.

## V0.3.1 (2026-10-24)

The fixes that the tests of 0.3.0 found: none. No code changed in this release.

### Checks
- **Reported by the maintainer on 2026-10-24:** the tests of the clean install came out good, with no step failing and no finding to correct. V0.3.1 therefore has nothing to fix; its row closes with the record of that, and the numbers after it do not move (the roadmap said a finding that needs a release of its own would push them down).
- **Run for this release:** `version:check` (version `0.3.1` everywhere). **Not run:** the tests and the other checks, which V0.2.193 last reported; V0.3.2 compiles the tests and hands them over.

### Changed
- **The version** is `0.3.1` in `Cargo.toml`, `Cargo.lock`, `package.json`, `package-lock.json`, the desktop `package.json` and `tauri.conf.json`.
- **The 0.3.x line is under way:** decisions 29 to 32 of the roadmap are taken as proposed, since the maintainer asked on 2026-10-24 for the releases to be built one by one up to V0.3.27 (no separate `v0.2.0` tag, the 0.3.x rules, the local edition removed in V0.3.5–V0.3.7, track Q as listed). Decision 33, the theme of V0.4.0, is still to be recorded in V0.3.27. The roadmap gains a *Release checklists (0.3.x)* section, and `mindmap/RELEASES_0.3.md` marks the row.

### Upgrading
- Nothing to do.

## V0.3.0 (2026-10-24)

*Connected.* The version is set to `0.3.0` after the maintainer's tests of the clean-machine install. No code changed in this release.

### What 0.3.0 delivers
- **Reaches remote code:** repositories over `https` and `ssh` with stored credentials, push webhooks, last author, `CODEOWNERS` hints and a citation at its commit (V0.2.19–V0.2.26).
- **Talks by mail:** invitations, "Forgot your password?", mentions, assignments, address verification, notification preferences and a daily digest (V0.2.9–V0.2.15).
- **Links the work around the code:** GitHub and GitLab issues and pull or merge requests as evidence, linked to the files and symbols they name (V0.2.27–V0.2.31).
- **Remembers the conversation:** an assistant that reads earlier turns, search across workspaces, a command palette, a review queue and file version history (V0.2.32–V0.2.39).
- **Portable:** workspace export and import, encrypted backups with a second copy, a container image, and an audit trail export (V0.2.40–V0.2.46).
- **One client:** the desktop app opens a server; the local edition is retired (V0.2.49–V0.2.51).
- **Still team-ready:** accessibility and narrow screens (V0.2.47–V0.2.48), and a security pass after each new surface (the fourth, fifth and sixth reviews).
- **Beyond the promise:** one look for every page (track K, V0.2.57–V0.2.129); security and dependency fixes and backend hardening (tracks L and M, V0.2.130–V0.2.162); workspace templates (track N, V0.2.163–V0.2.193).

### Checks
- **Reported by the maintainer on 2026-10-24:** the tests of the clean install came out good. The per-step results of the clean-install page were not recorded, so its table and the pre-tag list are not ticked; V0.3.4 records them.
- **Run for this release:** `version:check` (version `0.3.0` everywhere). **Not run:** the tests, `cargo deny`, `npm audit` and the other checks, which V0.2.193 last reported.

### Changed
- **The version** is `0.3.0` in `Cargo.toml`, `Cargo.lock`, `package.json`, `package-lock.json`, the desktop `package.json` and `tauri.conf.json`. The tag `v0.3.0` is the maintainer's to create (decision 29: there is no separate `v0.2.0` tag).
- **A user guide** for people, `docs/USER_GUIDE.md`: a light tour of what the product offers, for team members, workspace admins and system admins, with diagrams.
- **The plan after the tag:** the roadmap's *Road to V0.4.0* proposes V0.3.1–V0.3.27 in tracks O to T (fixes and the record, the retired local edition removed, small gaps closed, portable archives and connector depth, operations, a security pass), with decisions 29–33 to confirm. A new release index, `mindmap/RELEASES_0.3.md`, lists them.
- **The documents** say `V0.3.0`: the roadmap's status tables, the release indexes, both mindmaps' scope lines and the functional gaps table.

### Upgrading
- Nothing to do. See UPGRADING.md, *From 0.2.0 to 0.3.0*, for what stays off until turned on and what changed in behaviour. The desktop app's local workspace goes in V0.3.5–V0.3.7 as planned: move any you need to a server before then.

## V0.2.193 (2026-10-22)

Release checks repeated after workspace templates; the freeze restored.

### Checks
- **Passing:** `npm run typecheck`, `styles:check`, `a11y:check`, `types:check`, `version:check`, `docs:check`, `container:check`, `migrations:check` (41 migrations, every one locked and unchanged since its release) and `templates:check` (the four templates and the example, no template id in code). `cargo check` of every crate the track touched compiles with its tests.
- **In CI:** `templates:check` and the checker's own tests run in *Versions agree* since V0.2.188, with the whole history checked out and the pull request's base.
- **Dependencies:** the track added no package. Comparing `Cargo.lock` and `package-lock.json` with V0.2.162, where `cargo deny check` and `npm audit` with development packages were last reported clean, nothing is added, removed or changed (675 external Rust packages, 410 npm packages, the same versions). The two commands were **not run** here, because they reach the advisory database and the npm registry; CI runs both on every push, and the maintainer can run `cargo deny check` and `npm audit` before the tag.
- **Tests:** none was run, as asked. The track wrote them (templates 89, retrieval 21, ingestion 21, console 14, server 194, web 334, the checker 9) and compiled them.

### Changed
- **The documents match the code:** the test counts of the technical mindmap had drifted from the source (at V0.2.162 its server row said 156 where the source had 184, and the web row 182 where it had far more); they are now the number of test attributes and `it(` calls in the source, and say so. The gaps table of the functional mindmap lists what templates do not do (organizations' own templates, new modules, per-user layouts, different indexing, installing from a URL, AI-written templates, moving evidence, the four-figure dashboard headline, the closest-figure Overview cards, who can read a template). UPGRADING.md says workspaces stay on General and where custom templates go; `technical/workspace-templates.md`, the roadmap and the release index say the track is built.
- **The freeze is restored (decision 24):** from V0.2.193 until the tag, only the checks of the clean install and the fixes it finds go in. Track N is complete: V0.2.163–V0.2.193, one block of the format per release, the engine, the checker, the operator's folder, the console and System views, the download of a setup as a file, and the security pass.

### Upgrading
- Nothing to do. See UPGRADING.md for what the templates changed and what they did not.

## V0.2.192 (2026-10-22)

Security review of workspace templates with its fixes.

### Security
- **Review:** [mindmap/security/2026-10-22-workspace-templates-security-review.md](mindmap/security/2026-10-22-workspace-templates-security-review.md) reads who may read and change a template, the operator's folder (path handling, size, depth, links, shadowing), the ranking bounds, the activity records, what `GET` answers a viewer, the download, the console and the checker. **0 high, 2 medium, 2 low, 4 info; none medium or low is open.** Nothing was built, started or attacked.
- **T1 (medium), ranking patterns:** the matcher that applies a boost's folder pattern took time that grew with every `**`, so one template file could stall every search of the workspaces that use it. Its work is now the pattern's folders times the path's, whatever the pattern says.
- **T2 (medium), repository patterns:** the matcher of a repository's include and exclude patterns had the same shape with `*` and `**`, and a template now offers such patterns in the link form. A pattern gives up after 100,000 steps on one path and matches nothing; a sensible pattern is nowhere near the limit.
- **T3 (low), messages:** messages about a template file quote it (an unknown key, a word outside a list, the file's name). Control and reordering characters are now written out (`\n`, `\u{1b}`) and a text is cut at 300 characters, so a key cannot forge a log line, put an escape on an administrator's terminal or fill System › Overview.
- **T4 (low), the checker:** `npm run templates:check` writes out control characters and cuts long texts before it prints, so a template cannot put a line of its own in a CI log (a line that starts with `::` is a command to GitHub Actions).
- **T5 to T8 (info):** accepted or tracked. Every signed-in account can read every template and the `focus` line goes to the AI provider, so **a template holds no secrets, names or addresses**: said in `docs/TEMPLATES.md` and in the description of `REPOMEMO_TEMPLATES_DIR`. The operator's folder is trusted as its owner is, the resolver's cache of 256 answers stays, and the suggested questions an administrator saves are text like any message.

### Changed
- **Tests (written, not run):** four: a pattern of eleven `**` against a path of eighty folders, and one of many stars in a name (`repomemo-retrieval`); a pattern of many stars that gives up (`repomemo-ingestion`); a message that quotes a key with a line break, an escape and a reordering character, a word of five thousand characters and a file name with an escape (`repomemo-templates`); and a key that tries to forge an Actions command in the checker's output (`scripts/check-templates.test.mjs`).
- **Checks run on this release:** `cargo check` of the retrieval, ingestion and templates crates with their tests compiles; `rustfmt` shows nothing for the lines changed; `templates:check`, `types:check`, `typecheck`, `styles:check`, `a11y:check`, `docs:check` and `version:check` pass. **Not run:** the tests, as asked.

### Upgrading
- Nothing to do. A repository pattern that needs more than 100,000 steps to decide on one path (none that a person would write does) now matches nothing instead of taking the sync's time.

## V0.2.191 (2026-10-22)

A workspace's adjusted setup can be downloaded as a template file.

### Added
- **Download as template file:** in Settings › Template, for owners and administrators. The file is the template the workspace uses with the adjustments saved on it (the section it opens on, what is under More, the suggested questions) already folded in, under the id and name chosen (the form offers `<template>-custom` and `<name> (custom)`), at version 1, as JSON ready for the folder `REPOMEMO_TEMPLATES_DIR` names. After the download the page says where it goes: the templates folder of a server, then a restart.
- **`GET /v1/workspaces/{ws}/template/file?id=&name=`:** the route behind it, with `Content-Disposition: attachment; filename="<id>.json"`. The file is parsed with the rules of every template file before it is sent, so it passes `npm run templates:check`. A value a template file refuses (`meta.id`, `meta.name`) is a `400` that names it. The id of a template this server already has, `general` and the built-in and custom ones, is a `409`: the file would be refused when it is loaded here, so choose another id (the file is meant for another server's folder, or this one's under a new id).
- **Nothing of the workspace is in it.** The file holds what a template holds: tabs, names, cards, the ledger's view, the assistant's questions, phrases and focus, search weights, checks, review order, map and repository rules. There is no evidence, person, setting or right; the workspace's id, its name and addresses are not in the text.

### Changed
- **Tests (written, not run):** one in the api crate (the exported file passes the checks, carries the adjustments, has no trace of the workspace and, loaded, gives the same sections, labels, suggestions, ranking, checks and cards; a taken id, `general`, a bad id and a bad name are refused; a workspace on General exports General), one on the server (the headers, the adjustments, no trace of the workspace, a taken id `409`, a bad id or name `400`, a stranger and no sign-in refused) and three for the web (the file downloaded under the id and name typed with the note where it goes, the server's reason when it refuses, nothing without an id and a name).
- **Checks run on this release:** `cargo check` of the api and server crates with their tests compiles; `rustfmt` shows nothing for the lines changed; `templates:check`, `types:check`, `typecheck`, `styles:check`, `a11y:check`, `docs:check` and `version:check` pass. **Not run:** the tests, as asked.

### Upgrading
- Nothing to do. The route and the group in Settings › Template are new; no behaviour changes for a workspace that does not use them.

## V0.2.190 (2026-10-22)

Templates can be listed and checked from the console, and System shows which are loaded.

### Added
- **`templates list`:** the workspace templates the server offers, with their id, name, version and source (built into the server, or the custom file), and a row for each file of `REPOMEMO_TEMPLATES_DIR` it left out, with the place and the reason. `templates` alone runs it. It works in the web console as well.
- **`templates show <template>`:** what one template arranges, in one list of facts: its tabs and what is under More, where it opens, what it renames, the Overview cards and the dashboard headline, the ledger, the assistant, search weights, health checks, review order, map and repository rules. An id the server lacks is a `404`.
- **`repomemo-console templates validate <file>`:** checks a template file on your machine with the rules the server applies to its folder (the same code reads the file: 64 KiB, UTF-8, a link not followed, the format, the lists and bounds, a file named after its id, no built-in id). It prints the server's own messages, with the place in the template and the line and column of broken JSON, and exits 1 when the file would be left out. It needs **no server and no sign-in**. A name starting with `_` is checked for its content and said to be a file the server leaves alone. Sent to the server, as in the web console, the command answers that it runs in the terminal client.
- **System › Overview › Templates:** a panel that is always shown. It lists every template the server offers with its version and source, tags the custom ones, names the folder (or that none is set) and lists each file that was left out with where it went wrong and why; the answer of `GET /v1/system/overview` carries `template_files.templates` and the line and column of a problem.

### Changed
- **Tests (written, not run):** one in the templates crate (one file read the way the folder's files are, and what is named when it cannot be), five in the console (only a `templates validate` line is taken; a valid file; the server's messages for a misspelled section, another name, a built-in id, an unknown key and broken JSON; a file the server leaves alone; a file that cannot be read), three for the web (the list with sources and the Custom tag, files left out with the place and the reason, no folder set), and the server's template test now covers the list, the console commands and `templates validate` sent to the server.
- **Checks run on this release:** `cargo check` of the templates, server and console crates with their tests compiles; `rustfmt` shows nothing for the lines changed; `templates:check`, `types:check`, `typecheck`, `styles:check`, `a11y:check`, `docs:check` and `version:check` pass. **Not run:** the tests, as asked.

### Upgrading
- Nothing to do. The console gains three commands; nothing else changes for a server without `REPOMEMO_TEMPLATES_DIR`, whose Templates panel lists the built-in templates.

## V0.2.189 (2026-10-22)

Operators can add templates in a folder without a release; invalid files are named and ignored.

### Added
- **`REPOMEMO_TEMPLATES_DIR`:** a folder of workspace template files. The server reads its `*.json` files once, when it starts, and offers the valid ones next to the built-in templates; a change needs a restart. Files whose name starts with `_` are left alone, as in the built-in folder.
- **The same rules:** a file is held to what every template is. It is at most 64 KiB (checked on its size and again while reading), at most 64 files are read, JSON nests at most 8 deep, a link is never followed and a folder is not entered, a file is named after its id, and its id cannot be one of the built-in templates or `general`: `operations.json` cannot replace Operations.
- **A bad file never stops the server:** it is left out and named in System › Overview (a *Template files* panel with the file, the place in it, such as `sections.order[0]`, and what is wrong), in the server check (a *Template files* row, a warning when a file was left out) and in the log. A folder that cannot be read is one such problem.
- **Custom tag:** a template from the folder carries *Custom* in the picker. `GET /v1/templates`, a template's detail and a workspace's own template carry `custom`.
- **Settings:** `REPOMEMO_TEMPLATES_DIR` is described in the development commands, and the upgrade notes mention it.

### Changed
- **Tests (written, not run):** seven in the templates crate (files added are custom and sorted among the built-in ones, in any order; a file cannot take the id of a built-in template or `general`, must be named after its id and is named when refused; a folder is read in name order and the rest left alone; a folder that cannot be read is a problem and not an error; the size limit and bad text; at most 64 files; a link not followed, on Unix), one on the server (a folder with a valid file, a file named like a built-in, an unknown section and a text file: the picker, System › Overview and a workspace made from the custom template; a server with no folder says nothing about one) and one for the web (the Custom tag, on one template only).
- **Checks run on this release:** `cargo check` of the templates, api and server crates with their tests compiles; `rustfmt` shows nothing for the lines changed; `templates:check`, `types:check`, `typecheck`, `styles:check`, `a11y:check`, `docs:check` and `version:check` pass. **Not run:** the tests, as asked.

### Upgrading
- Nothing to do. Without `REPOMEMO_TEMPLATES_DIR` the server offers the templates it was built with, as before.

## V0.2.188 (2026-10-22)

A checker and a guide for writing a template.

### Added
- **The checker:** `npm run templates:check` (`scripts/check-templates.mjs`) reads every template file of `crates/templates/definitions/` and the example, and refuses what the server refuses, naming the file, the line, the column and the place in the template (`ranking.verified`, `sections.order[1]`): an unknown key (at the key), a word outside its closed list, a number outside its bounds (a factor of 1.6), a text that is empty, too long, padded or has a control character, an entry listed twice, a list that is too long, a pattern that is not a folder pattern. It also says where a file is not JSON. It reads the lists and bounds from the Rust source of `crates/templates`, so they are written in one place.
- **Files, ids and versions:** a file is named after its template's id, an id is used once, and `general` cannot be replaced by a file. Compared with the last commit (or `--base <ref>`), a changed file must have a higher `meta.version`, and a raised version must come with a change to the file; a base that cannot be found is said and skipped.
- **No template id in code:** a string equal to a template's id in a `.rs`, `.ts` or `.tsx` file outside the definitions, the tests and the documents is refused with its line. Templates are asked what they arrange; nothing is decided by name.
- **The guide:** `docs/TEMPLATES.md` for the person writing a template: what a template can and cannot do, copy the example, fill `meta` and the blocks, run the checker, try it in a workspace, raise the version, and a table of every block with its closed lists and limits. `crates/templates/definitions/_example.json` is a Customer support template that uses every block and is not registered (its name starts with `_`).
- **CI:** *Versions agree* runs the checker with the base of the pull request (or the previous commit of a push), with the whole history checked out, and then the checker's own tests.

### Changed
- **The plan's schema:** the plan proposed a JSON schema generated by a test from the Rust types. Generating one means running the tests, so the checker reads the lists and bounds from the Rust source instead; they are still written once, and the server's validator remains the final word.
- **Tests (written, not run):** eight for the checker in `scripts/check-templates.test.mjs` (the repository passes; a valid file passes; a misspelled section id, a factor of 1.6, an unknown key, a pattern like a regular expression with a headline it cannot count and a repeated entry, broken JSON and a file not named after its id are each refused with their place), and one in the templates crate (the example is valid, uses every block and is not a built-in template; it stops compiling when a block is added to the format and not to the example). The checker was run by hand on the four definitions and the example, on bad files, on a file with a template id added to a component, and on a changed file with and without a new version.
- **Checks run on this release:** `templates:check`, `docs:check`, `version:check`, `types:check`, `typecheck`, `styles:check` and `a11y:check` pass; `cargo check` of the templates crate with its tests compiles; `rustfmt` shows nothing for the lines changed. **Not run:** the tests, as asked.

### Upgrading
- Nothing to do. This release changes no behaviour of the server or the web client.

## V0.2.187 (2026-10-22)

Workspace cards name their template and show its headline figure.

### Changed
- **Headline:** `overview.headline` names the one figure a workspace's card shows beside its template's name. It is one of `memory_cards`, `active_tasks`, `code_symbols` and `sources`: the pulse cards one grouped count can answer for all the dashboard's workspaces at once. The other cards need each workspace's metrics, which are a request for each workspace, and the dashboard makes none. Anything else is refused, naming `overview.headline`.
- **The dashboard's request:** `GET /v1/dashboard` carries `template` for each workspace: `null` for General, so General shows no badge, and otherwise `{id, name, headline: {card, value}}`. The template is settled from the settings that came with the list of workspaces, so there is no read for each; a headline that needs a count the dashboard did not have (`code_symbols`, `sources`) is one more grouped query, for the workspaces that name it only. Nothing is asked for each workspace.
- **The cards:** a workspace's card on the Dashboard names its template next to its access, and shows the headline among its figures in bold (active tasks, which every card has, is that figure made bold). The organization's Workspaces page lists the same, sharing the dashboard's request. The narrow navigation rail does not: it is on every page, and the figures would add a request to each.
- **The four templates:** Software engineering shows code symbols, Operations and incidents active tasks, Product and decisions memory cards and the Knowledge base sources.
- **Settings › Template and the API:** the answer carries `overview.headline`, and the preview has a line "Workspace card shows".
- **Tests (written, not run):** two in the templates crate (a headline is one of four pulse cards, none by default and nothing written for it, anything else refused; the four templates' headlines), the dashboard's server test (a General workspace has no template, one made from Software engineering names it and its code symbols in the same single request), the server's template test and the web's template tests check the field, and three web tests in `dashboardFigures.test.ts` (the name, the headline in words with the pulse card's own name, the figures on a card).
- **Checks run on this release:** `cargo check` of the templates, storage, api and server crates with their tests compiles; `rustfmt` shows nothing for the lines changed; `types:check`, `typecheck`, `styles:check`, `a11y:check`, `docs:check` and `version:check` pass. **Not run:** the tests, as asked.

### Upgrading
- Nothing to do. A workspace on General looks as it did, and a client that ignores `template` keeps working.

## V0.2.186 (2026-10-22)

The repository link form starts with the include and exclude rules its template offers.

### Changed
- **Repositories block:** `repositories.include` and `repositories.exclude` are lists of up to sixteen patterns each, in the same small grammar as the ranking block's folders: names, `*` inside a name and `**` for any number of folders. A regular expression, a leading, trailing or doubled slash, `..` and a pattern listed twice in its list make the file invalid, naming the pattern. They mean what a repository's own include and exclude patterns mean, and the defaults every repository has still apply.
- **The link form:** Workspace Settings › Repositories fills **Add a repository link** with the rules, and says under Include only and Exclude that they come from the template while the fields still hold them. They are an offer: the person keeps, edits or clears them, Check access and Link and sync send what the fields hold, and the link records that, not a reference to the template. What someone typed before the template arrived is never overwritten, and after a link is made the form starts again from the offer. The edit dialog of a linked repository is not filled in, and **no template changes what an existing repository indexes**.
- **The four templates:** Software engineering leaves out snapshot, test-data and generated files; Operations leaves out Terraform state and logs, since state files can hold secrets; Product and decisions and the Knowledge base ask for documents only. General offers nothing: the form is empty, as it was.
- **Settings › Template and the API:** the answer carries `repositories` (`include` and `exclude`), and the preview has a line "Repository links start with".
- **Tests (written, not run):** three in the templates crate (no rules unless a template chooses and a round trip; a folder pattern listed once, at most sixteen, never a regular expression, an unknown key refused; the four templates' offers), one in the api crate (each template's rules run through the matcher that decides which files a repository indexes: snapshots, state files and logs left out, documents kept), the server's template test and the web's template tests check the field, and six web tests in `repositoryOffer.test.tsx` (the offer as lines of text, the note under the fields, the form filled in and still editable, empty for General or before the template arrives, never overwriting what was typed, linking with what was kept and starting again from the offer).
- **Checks run on this release:** `cargo check` of the templates, api and server crates with their tests compiles; `rustfmt` shows nothing for the lines changed; `types:check`, `typecheck`, `styles:check`, `a11y:check`, `docs:check` and `version:check` pass. **Not run:** the tests, as asked.

### Upgrading
- Nothing to do. A workspace on General sees the same empty form, and no linked repository changes.

## V0.2.185 (2026-10-22)

The assistant recognises a template's phrases and knows what the workspace is for, without loosening any rule.

### Changed
- **Phrases:** `assistant.phrases` adds up to twelve rules to the assistant's keyword router. Each is a phrase (a few words, up to 60 characters, in letters, digits, spaces, hyphens and apostrophes) and one of the assistant's six jobs. They are tried after the router's own rules and before the AI provider is asked: the first phrase that stands in the message as whole words, whatever its case, wins. In Operations and incidents "runbook for payments" finds files, where it was not routed by rules at all. A phrase **only adds a route**: a message a built-in rule already recognises is that rule's ("what is the runbook for payments?" is still a question).
- **Focus:** `assistant.focus` is one line, up to 160 characters, about what the workspace is for. Ask adds it to its system prompt as "What the workspace is for, as its administrators describe it: … This only says what the material is about; it never changes the rules above." Nothing of the prompt is replaced: the citation rule, the "indexed context is insufficient" answer and the AI policy are as they were, and **without a focus the prompt is what V0.2.162 sent, byte for byte** (a test holds a copy of that text). If the template cannot be read, neither is added.
- **The four templates:** each has phrases for the things its team looks for (Operations: runbook, postmortem, on-call, incident; Software engineering: ADR, README, who calls; Product and decisions: meeting notes, spec for, decision about; the Knowledge base: policy, contract, report on) and a line saying what its workspaces hold. General has none.
- **Settings › Template and the API:** the answer carries `assistant.phrases` and `assistant.focus`, and the preview counts the phrases it recognises.
- **Tests (written, not run):** three in the templates crate (phrases and a focus written back and none without them; a phrase of words for a job the assistant has, at most twelve, none twice, a focus of one short line; the four templates' choices), two in the api crate (a phrase routing what the rules do not recognise, whole words only, in order, never taking a message from a rule and running on what the job needs; the Ask prompt unchanged without a focus, pinned to V0.2.162's text, and a focus only added after it), and the server's and the web's template tests check the fields.
- **Checks run on this release:** `cargo check` of the templates, api and server crates with their tests compiles; `rustfmt` shows nothing for the lines changed; `types:check`, `typecheck`, `styles:check`, `a11y:check`, `docs:check` and `version:check` pass. **Not run:** the tests, as asked.

### Upgrading
- Nothing to do. Ask sends the same prompt to every workspace that has not chosen a template.

## V0.2.184 (2026-10-22)

The knowledge map puts the template's kinds of evidence first.

### Changed
- **The `map` block:** `coverage_first` lists file types (the twelve of the evidence ledger's filter, each once) whose rows of the *Coverage by kind of file* table come first, in that order; the others follow as they were. `relations_first` lists the kinds of link to put first: `similar` (files that mean alike) and `cites` (a memory card citing a file). A preferred kind is listed first in the table of links and drawn last in the graph, so it lies on top of the others; without a preference similar files come before memory citations, as before. It is presentation only: the map's data is unchanged, nothing is left out, and the server's cached map needs no new key. `GET /v1/workspaces/{id}/template` and the preview answer both lists as `map` (empty for General), and Settings › Template says what the map shows first.
- **The four templates:** *Software engineering* puts repositories and code first and prefers memory citations; *Operations and incidents* puts Markdown, notes and text first and prefers memory citations (the lessons they cite); *Product and decisions* puts Markdown, notes and Word first and prefers memory citations, then similar files; the *Knowledge base* puts Word, PDF, Excel and PowerPoint first and prefers similar files.
- **A difference from the plan:** the plan said Software engineering's map shows "repositories and symbols first". Symbols are not a kind of file on the map, so its coverage puts repositories and code first.
- **A guard on the kinds of link:** the kinds a template can put first are the map's edge kinds; a test in the api crate matches on every `KnowledgeEdgeKind`, so adding one stops it compiling until the list in `repomemo-templates` has it too.
- **Tests (written, not run):** three in the templates crate (nothing first without a block and a chosen emphasis written back; unknown or repeated file types and links refused; the four templates' choices), one in the api crate (the links a template can put first are the map's edge kinds), six in a new web test file (the coverage rows in their order, with the preferred kinds first in the template's order, with an unknown kind ignored and the rows untouched; the links' ranks, the table's order and the graph's drawing order), and the server's and the web's template tests check the field.
- **Checks run on this release:** `cargo check` of the templates, api and server crates with their tests compiles; `rustfmt` shows nothing for the lines changed; `types:check`, `typecheck`, `styles:check`, `a11y:check`, `docs:check` and `version:check` pass. **Not run:** the tests, as asked.

### Upgrading
- Nothing to do.

## V0.2.183 (2026-10-22)

The review queue lists what the template cares about first.

### Changed
- **The `review` block:** `review.order` names the kinds of item the queue lists first, from the three it has: files marked needing review (`evidence`), memory cards whose evidence changed (`memory_card`) and open health findings (`health_finding`). Each is a group: inside it the item that has waited longest comes first, and findings keep Health's order. A kind the template does not name follows. At least one is named and none twice. `GET /v1/workspaces/{id}/template` answers it, completed to the three kinds, as `review.order`.
- **The mixed list stays the default:** without the block the queue is what it has always been, all three kinds mixed with the oldest first and the findings, which have no date, after what has waited. General and *Software engineering* keep it. Rights are untouched: who may resolve an item is still decided by its kind and the caller's role.
- **The templates:** *Operations and incidents* lists open health findings first, then files, then memory cards; *Product and decisions* lists memory cards first; the *Knowledge base* lists files first.
- **The preview:** Settings › Template says what the queue lists ("open health findings, then files, then memory cards", or "everything mixed, the oldest first").
- **Tests (written, not run):** three in the templates crate (mixed without a block and unnamed kinds completed after the named ones, written back; an unknown, repeated or empty list refused; the four templates' choices), four on the server (the old mixed order; a template's groups with the oldest first inside each; a kind not named following and findings keeping Health's order; the kinds a template can name are the review item kinds, which stops compiling if one is added), and the server's and the web's template tests check the field.
- **Checks run on this release:** `cargo check` of the templates, api and server crates with their tests compiles; `rustfmt` shows nothing for the lines changed; `types:check`, `typecheck`, `styles:check`, `a11y:check`, `docs:check` and `version:check` pass. **Not run:** the tests, as asked.

### Upgrading
- Nothing to do.

## V0.2.182 (2026-10-22)

A new health check finds evidence nobody has touched for as long as the template allows.

### Added
- **Stale evidence:** a file is stale when more than `stale_after_days` whole days have passed since it was last changed and since it was last marked verified. A file verified within the limit is not stale, a repository's overview is never checked, and a report lists the 200 longest-untouched files at most. With a limit of 90 days a file is flagged at 91 days and not at 89. A finding is a normal one: a warning titled with the age, the file, and the actions Health already has (ask for a review, mark outdated, make a task, dismiss); to verify the file, open it from the finding. It appears in Workspace Health and in the Review queue like any other finding.
- **Only where a template asks:** the check is `stale_evidence` in the `health` block's list, with `health.stale_after_days` (7 to 3650) required beside it and refused without it. No workspace runs it unless its template lists it, so **General reports none**. *Operations and incidents* runs it first, at 90 days; the *Knowledge base* runs it third, at 180.
- **A new check, an old list:** `HealthDetector` has an eighth check. `HealthDetector::DEFAULT` stays the seven that have always run (it is what a workspace without a template runs) and `ALL` has all eight. The cached Health answer is keyed by the days as well as by the checks, so changing the limit is never answered from the cache.
- **Tests (written, not run):** two in the templates crate (the days required with the check, bounded to 7 to 3650 whole days and refused without it; which templates run it), four in the api crate (the plan carries the days and keys the cache; flagged at 91 days and not at 89 or exactly 90, the oldest first, with the usual actions; evidence verified within the limit is not stale, the age counts from the last verification, a repository is never checked, a time that cannot be read is left out; a report lists the oldest 200 only), one in storage (the time a file was last marked verified, for verified files only), and the server's and the web's template tests check the days. The earlier checks' tests follow the new list.
- **Checks run on this release:** `cargo check` of the templates, storage, api and server crates with their tests compiles; `rustfmt` shows nothing for the lines changed; `types:check`, `typecheck`, `styles:check`, `a11y:check`, `docs:check` and `version:check` pass. **Not run:** the tests, as asked.

### Upgrading
- Nothing to do. No workspace runs the new check until an administrator chooses a template that lists it.

## V0.2.181 (2026-10-22)

The health checks that run, and their order, follow the workspace's template.

### Changed
- **The `health` block:** `detectors` lists the checks that run and the order their findings are listed in, from the seven Workspace Health has: older versions, duplicates, removed code, outdated references, stale memory, indexing failures and unconnected files. At least one is named and none twice. Without the block every check runs in the usual order, so General is unchanged. `GET /v1/workspaces/{id}/template` and the preview answer it as `health.detectors`, and Settings › Template lists the checks.
- **A check that is left out is not run:** it costs nothing, its findings are not listed and it is not in the statistics. What an administrator already decided about its earlier findings stays recorded and returns if the check does. The findings that are listed sort by severity and then by the template's order, on the Health page and in the Review queue alike.
- **The cached answer follows the plan:** the server keeps Health between requests while a workspace's content is unchanged; the key now includes which checks run, so choosing another template is never answered from the cache. If the template cannot be read every check runs: a template must never be the reason Health says nothing.
- **The four templates:** *Software engineering* runs removed code, outdated references and stale memory; *Operations and incidents* runs outdated references and stale memory; *Product and decisions* lists outdated references first, then older versions and stale memory, and runs no code check; the *Knowledge base* runs older versions, duplicates and outdated references. All four also run the indexing-failure check, which the plan's table had not listed: a file that search cannot see should always be reported.
- **Tests (written, not run):** three in the templates crate (every check in the usual order without a block and a chosen list written back; an unknown, repeated or empty list refused; the four templates' checks, indexing failures included), two in the api crate (the checks a template can choose are the health detectors and their ids; a plan runs what the template names in its order, skips a name this server does not know and keys the cache), and the server's and the web's template tests check the field.
- **Checks run on this release:** `cargo check` of the templates, api and server crates with their tests compiles; `rustfmt` shows nothing for the lines changed; `types:check`, `typecheck`, `styles:check`, `a11y:check`, `docs:check` and `version:check` pass. **Not run:** the tests, as asked.

### Upgrading
- Nothing to do. Every workspace still runs every check until an administrator chooses a template.

## V0.2.180 (2026-10-22)

A template can prefer recent evidence, within the same bounds as other boosts.

### Changed
- **`ranking.recency`:** a half-life (`half_life_days`, 7 to 3650 whole days) and a maximum (`max_factor`, above 1 and at most 1.2). A file changed just now is favoured by the maximum, and the part above 1 halves every half-life, so with 90 days and 1.2 a file changed a month ago gets 1.16 and one changed a year ago 1.01: a month-old runbook edges out a year-old one of equal relevance, and a far better old match still wins. Off when the block is left out, so General and any template without it are unchanged. `GET /v1/workspaces/{id}/template` and the preview answer it as `ranking.recency` (`null` when off), and Settings › Template says "recent evidence by up to 20 %".
- **Cheap and bounded:** the files' times are read for the results of one search only, in one query over their ids (`StorageEngine::artifact_updated_times`), and a file whose time cannot be read is left as it is. The factor joins the product of the other rules, so the cap of 1.5 holds for it too.
- **The templates:** *Operations and incidents* prefers recent evidence (90 days, up to 20 %), because a runbook touched this quarter is more likely to be right; the other three do not.
- **Tests (written, not run):** three in the templates crate (off without the block and written back; the half-life 7 to 3650 whole days and a maximum above 1 up to 1.2; only Operations prefers recent evidence), two in the retrieval crate (the factor halves with the half-life and never goes below 1; a month-old runbook edges out a year-old one of equal relevance, nothing changes with the rule off, a far better old match still wins and a file of unknown age is not touched), one in storage (the times of the files asked about, not those of another workspace or of a missing file), the api test now builds the rule from a template, and the server's and the web's template tests check the new field.
- **Checks run on this release:** `cargo check` of the templates, storage, retrieval, api and server crates with their tests compiles; `rustfmt` shows nothing for the lines changed; `types:check`, `typecheck`, `styles:check`, `a11y:check`, `docs:check` and `version:check` pass. **Not run:** the tests, as asked.

### Upgrading
- Nothing to do.

## V0.2.179 (2026-10-22)

Evidence of the kinds, documents and folders a template cares about ranks a little higher, never above a clearly better match.

### Changed
- **Boosts:** `ranking.boosts` lists up to 16 rules for the ranking profile. Each names exactly one thing and a factor from 0.8 to 1.3: a **kind of evidence** (`artifact_type`: runbook, incident, decision, API specification…), a **kind of document** (`document_kind`: the ledger's file types, so a document format by its extension first, then the evidence's own type) or a **folder** (`path`). A passage that several rules match gets the product of their factors, and that product is held between 1/1.5 and 1.5, so a pile of boosts, all the 16 of them at 1.3 included, cannot bury a clearly better match: a passage that scores 0.6 cannot pass one that scores 1.0, and a passage that scores 0.50 with the full boost (0.75) still loses to one that scores 0.80. A boost can lower as well as raise.
- **Folder patterns, never regular expressions:** a pattern is written with `/` between folders, `*` inside a name and `**` as a whole folder name for any number of folders. It may start anywhere in the path (`docs/adr/**` finds `docs/adr/0001.md` and `payments/docs/adr/0003.md`; `README*` finds the README of any folder) and ignores case. Up to 120 characters and 12 folders; `( ) [ ] { } | ? ^ $`, a backslash, an empty folder name, `.` and `..` are refused when the template is read, and the matcher is a plain walk over the folders, so a pattern costs no more than its length.
- **The four templates:** *Operations and incidents* boosts runbooks and incidents by 20 % (and verified evidence by 15 %); *Software engineering* boosts code, API specifications and decisions by 10 % and `docs/adr/**` and `README*` by 15 %; *Product and decisions* boosts decisions, notes and Markdown by 15 %; the *Knowledge base* boosts Word, PDF, Excel and PowerPoint documents by 15 %. General has none.
- **Settings › Template and the API:** `GET …/template` and the preview answer the boosts as `ranking.boosts`, and the preview lists what search favours ("verified evidence by 15 %, Runbook by 20 %, docs/adr/** by 15 %").
- **No extra lookup:** the kind of evidence and the path are already on every search result, so a boost reads nothing more from the database.
- **Tests (written, not run):** four in the templates crate (the three selectors read and written back; exactly one selector, a known kind and a factor in bounds; at most sixteen boosts and a pattern that is never a regular expression; the four templates' boosts), nine in the retrieval crate (folder patterns wherever they start, name patterns, patterns with folders, case and slashes with nothing a regular expression; a runbook outranking an equally good note while a far better note wins; a document kind and a folder boosting what they match; the cap with sixteen rules that all match, up and down; rules that stack; the kind of document as the ledger classifies it), the api test now builds a profile from a template's boosts, and the server's template test checks the boosts of General and of the four.
- **Checks run on this release:** `cargo check` of the templates, retrieval, api and server crates with their tests compiles; `rustfmt` shows nothing for the lines changed; `types:check`, `typecheck`, `styles:check`, `a11y:check`, `docs:check` and `version:check` pass. **Not run:** the tests, as asked.

### Upgrading
- Nothing to do.

## V0.2.178 (2026-10-22)

Search ranking is described by a profile that comes from the workspace's template; General ranks exactly as before.

### Changed
- **A ranking profile:** after search has found passages by words and by meaning and fused the two lists, it weighs them with the workspace's **ranking profile**, a list of rules where each is a factor for the passages it matches. The product of the factors that match a passage multiplies its score, the scores are rescaled to at most 1 and the list is sorted again. `RankingProfile` and `apply_profile` replace the fixed "verified x 1.1" step (`favour_verified`, `apply_verified_boost`); `hybrid_search` takes the profile from its caller, and search, Ask, the assistant and the search across workspaces each pass their workspace's. The profile's first rule is *lifecycle*: passages of verified files by a factor.
- **The `ranking` block:** `ranking.verified` is that factor, from 0.8 to 1.3 (1.1 when left out). A factor can lower as well as raise a kind of evidence, but the bounds keep it a nudge: at 1.3 a passage scoring 0.6 still cannot pass one scoring 1.0. `GET /v1/workspaces/{id}/template` and the preview (`TemplateDetail`) answer it as `ranking.verified`, and Settings › Template says it ("verified evidence by 15 %").
- **General is unchanged:** a workspace on General (or with no template) has the one rule that has always been there, verified x 1.1, and a test pins the profile's results against the figures V0.2.162 produced for the same lists. If a workspace's template cannot be read the usual profile is used, so a ranking is never the reason a search fails.
- **The four templates:** *Operations and incidents* and the *Knowledge base* favour verified evidence by 15 %; *Software engineering* and *Product and decisions* keep 10 %.
- **A consequence in the code:** the template types hold a number with a fraction now, so they no longer derive `Eq` (they still compare with `==`).
- **Tests (written, not run):** three in the templates crate (the usual factor without a block and a chosen one written back, 0.8 to 1.3 allowed and anything else refused, the four templates' factors), four in the retrieval crate (a verified boost that stays within one, General ranking exactly as V0.2.162 did, another verified factor changing the order and the scores, what a profile changes), one in the api crate (a workspace ranking by its template's factor and by the usual one without), and the server's and the web's template tests extended with the factor.
- **Checks run on this release:** `cargo check` of the templates, retrieval, api and server crates with their tests compiles; `rustfmt` shows nothing for the lines changed; `types:check`, `typecheck`, `styles:check`, `a11y:check`, `docs:check` and `version:check` pass. **Not run:** the tests (the V0.2.162 search fixtures included), as asked.

### Upgrading
- Nothing to do. Search results are in the order they were in for every workspace that has not chosen a template.

## V0.2.177 (2026-10-22)

Owners and administrators choose and adjust a workspace's template in Settings.

### Added
- **Settings › Template:** the panel shows the template the workspace uses (and why General is in use when its template cannot be used on this server), lists every template as a card, and previews the one picked in lines a person can read: the tabs, what is under More, where the workspace opens, what is renamed, the Overview cards and how many questions are suggested. Choosing one asks first ("Nothing stored is moved, created or deleted, and nobody gains or loses access"); the change shows on the next navigation and the open pages pick it up through the live event. Errors and successes are toasts.
- **`GET /v1/templates/{id}`:** a template as a person looks at it before choosing it (`TemplateDetail`): the same resolved blocks as a workspace's own answer without any workspace's choice. Any signed-in caller may ask; an id the catalogue lacks is `404`.
- **Adjustments:** an administrator may save three things on top of the template, and only what differs from the template is saved, so its later improvements still reach every workspace for what was not adjusted: the section the workspace opens on (`landing`, not People or Settings), exactly which sections sit under More (`more`) and the suggested questions (`suggestions`, replacing the template's, up to six). `PUT /v1/workspaces/{id}/template` takes them as `overrides`: left out they stay when the same template is chosen again, an empty object clears them (*Reset to template*), choosing another template or returning to the default drops them, and a bad one is `400` naming its key (`overrides.landing`). They use the rules of the blocks they adjust.
- **How they are applied:** the resolver lays the adjustments over the template before the answer is made, ignores them when the workspace fell back to General (they belong to a template this server cannot use) and drops a stale key by itself instead of failing a page.
- **In the feed:** "Changed the workspace template to…", "Adjusted the workspace template." and "Reset the workspace template's adjustments." Choosing the same template with the same adjustments records nothing.
- **Tests (written, not run):** four in the templates crate (adjustments laid over General and over Operations, replacing only their own key, refused when invalid, stale ones dropped one by one), three in the api crate (adjustments checked, saved, laid over, kept, cleared and dropped with the choice; ignored on a fallback; a template looked at before it is chosen), one on the server (the preview and its 404, an adjustment saved and read by a member, a bad one refused with 400, a viewer refused, the reset, the feed), thirteen in the web client in a new file (the preview lines, the adjustments to save, the questions typed, the panel before the template arrives, with its preview, changing only after the confirmation and not when cancelled, saving only what differs, six questions at most, resetting, a fallback explained, the error as a toast), and the earlier template calls follow the new argument.
- **Checks run on this release:** `cargo check` of the templates, api and server crates with their tests compiles; `rustfmt` shows nothing for the lines changed; `types:check`, `typecheck`, `styles:check`, `a11y:check`, `docs:check` and `version:check` pass. **Not run:** the tests, as asked.

### Upgrading
- Nothing to do.

## V0.2.176 (2026-10-22)

The assistant suggests questions that fit the workspace, and Retrieval suggests the kinds of evidence a team looks for.

### Changed
- **The `assistant` block:** a template may list the assistant's jobs to show first (`order`, from the six it has, each once) and up to six suggested questions of up to 120 characters (`suggestions`). The assistant's buttons and the command palette's assistant commands put the template's jobs first in its order and keep the others after them in the order the server gave. The suggestions appear as buttons above the message box while it is empty and no chat has begun; **choosing one fills the box and sends nothing**, and the person sends the message. The citation guard, the "insufficient context" answer and the AI policy are not touched: a suggestion is text for a box.
- **The `retrieval` block:** a template may suggest up to six kinds of evidence (`type_chips`) as chips above the Retrieval search, only for kinds the workspace has files of. Choosing a chip filters the search to that kind and choosing it again clears the filter; nothing is applied before. It is a block of its own because the plan's table had no key for it. `GET /v1/workspaces/{id}/template` answers both blocks as `assistant` and `retrieval`.
- **The four templates:** *Software engineering* looks up symbols first and suggests questions about where things are implemented and who calls the billing API, with chips for code, Markdown and API specifications; *Operations and incidents* asks and searches first ("What is the runbook for…", "What happened in the last incident?", "Who is on call for…") with chips for runbooks, incidents and notes; *Product and decisions* asks first ("What did we decide about…", "Which specs mention…") with chips for decisions, notes and Markdown; *Knowledge base* finds files first ("Which contracts mention…", "What does our policy say about…") with chips for files and Markdown. General suggests nothing.
- **A guard on the list of jobs:** the jobs a template can order are the assistant's capabilities; a test in the api crate matches on every `AgentCapability`, so adding one stops it compiling until the list in `repomemo-templates` has it too.
- **Tests (written, not run):** four in the templates crate (the usual buttons and no suggestion without a block; an unknown or repeated job and empty, long, padded, repeated or too many suggestions refused; the Retrieval chips; the four templates' choices), one in the api crate (the jobs are the capabilities), nine in a new web test file (the buttons in the server's order and in the template's, an unknown job ignored, the suggested questions trimmed, the real assistant panel in the template's order with suggestions that fill the box and send nothing and with none for a workspace without a template, the Retrieval chips, the palette's assistant order), and the server's template test now checks the assistant and Retrieval of General and of each of the four. The Retrieval and Evidence section tests' mocks of the data hooks gained `useTemplate`, which those sections have read since V0.2.173 (the mocks were missed there).
- **Checks run on this release:** `cargo check` of the templates, api and server crates with their tests compiles; `rustfmt` shows nothing for the lines changed; `types:check`, `typecheck`, `styles:check`, `a11y:check`, `docs:check` and `version:check` pass. **Not run:** the tests, as asked.

### Upgrading
- Nothing to do.

## V0.2.175 (2026-10-22)

The evidence ledger opens the way its template says and lists its preferred file types first.

### Changed
- **The `evidence` block:** a template may choose the view the ledger opens in (`default_view`, `grid` or `list`) and the file types its *File types* filter lists first (`types_first`, each of the twelve once). Without the block the ledger opens as a grid with the usual order, so General is unchanged. `GET /v1/workspaces/{id}/template` answers both as `evidence`.
- **A person's own choice wins:** the template's view is only where the ledger starts. Once someone picks a view themselves it stays for the visit, whatever the template says. The preferred file types move to the top of the filter in the template's order and the others follow in the usual order; a type this client does not list is ignored.
- **The four templates:** *Knowledge base* opens the ledger as a list and lists Word, PDF, Excel and PowerPoint first; *Operations and incidents* opens it as a list and lists Markdown, Note and Text first; *Product and decisions* lists Markdown, Note and Word first; *Software engineering* lists Code, Markdown and Repository first.
- **A difference from the plan:** the plan said "the types offered first in *Add*"; what the ledger offers by type is its file-type filter, so the order applies there.
- **Tests (written, not run):** three in the templates crate (the grid and no preference without a block and a chosen view and types written back; an unknown view or type and a repeat refused; the type list equal to the web client's and the four templates' choices), four in the web client (the view without a template and when asked, the usual order, the preferred types first in the template's order, an unknown or repeated type ignored), and the server's template test now checks the ledger defaults of General and of each of the four.
- **Checks run on this release:** `cargo check` of the templates, api and server crates with their tests compiles; `rustfmt` shows nothing for the lines changed; `types:check`, `typecheck`, `styles:check`, `a11y:check`, `docs:check` and `version:check` pass. **Not run:** the tests, as asked.

### Upgrading
- Nothing to do.

## V0.2.174 (2026-10-22)

The Overview shows the pulse cards its template asks for, in its order.

### Changed
- **The `overview` block:** a template lists the cards of the Overview's *Workspace pulse* (`overview.cards`): one to eight of eleven, each once, from the figures the page already loads (index coverage, indexed storage, fresh evidence, knowledge density, code symbols, sources, memory cards, team access, active tasks, at risk, team discussion). A card not listed is not shown, the charts below stay, and nothing new is counted or requested. `sources` and `memory_cards` are new cards, drawn from numbers the page had already (the number of sources and of memory cards). Without the block the nine cards that have always been shown appear in their order, so General is unchanged. `GET /v1/workspaces/{id}/template` answers the resolved list as `overview.cards`.
- **The four templates:** *Software engineering* shows code symbols, sources, index coverage, tasks at risk and fresh evidence; *Operations and incidents* fresh evidence, active tasks, tasks at risk and memory cards; *Product and decisions* memory cards, active tasks, fresh evidence and index coverage; *Knowledge base* index coverage, indexed storage, fresh evidence and knowledge density.
- **A difference from the plan:** the plan's first proposal named figures the metrics do not carry (repositories in sync, evidence reviewed in 90 days, needing review, duplicates, documents by kind). The templates use the closest figures that exist; a figure the metrics gain later becomes a card with the release that adds it.
- **In the client:** the nine cards moved from the Overview into `pulseCards.tsx` with the same words; an id this client cannot draw is ignored, a repeated one is drawn once, and if nothing is left the usual nine appear.
- **Tests (written, not run):** three in the templates crate (the usual nine without a block and a chosen list written back; an unknown, repeated, missing or ninth card refused; the list equal to the web client's and the four templates' cards), five in the web client (the usual nine and their words, the template's order, unknown and repeated ids with the fallback, every card drawn from loaded figures, a dash before they arrive), and the server's template test now checks the cards of General and of each of the four.
- **Checks run on this release:** `cargo check` of the templates, api and server crates with their tests compiles; `rustfmt` shows nothing for the lines changed; `types:check`, `typecheck`, `styles:check`, `a11y:check`, `docs:check` and `version:check` pass. **Not run:** the tests, as asked.

### Upgrading
- Nothing to do.

## V0.2.173 (2026-10-22)

A template can name sections and kinds of evidence the way a team says them.

### Changed
- **The `labels` block:** a template may give a section a different name (`labels.sections`, for example Tasks → *Actions*) and a kind of evidence a different name (`labels.artifact_types`, for example `pr` → *Pull request*). A name is 1 to 24 characters on one line; an id that is not one of the twelve sections or twelve kinds of evidence, or a bad name, makes the template invalid. It is display text only: addresses, rights, the API and the documentation keep the product's own names. `GET /v1/workspaces/{id}/template` answers `labels` with only the names the template changes.
- **Where the names show:** on the tab (the usual name stays as its tooltip), in the page title and what a screen reader hears after a move, in the command palette (listed and matched under the new name), and on the type tags of the evidence ledger and the Retrieval page. A section's own heading, the cross-workspace search and the System area keep the usual words.
- **The four templates:** *Operations and incidents* calls Tasks *Actions*, Evidence *Runbooks and incidents* and Memory *Lessons*; *Product and decisions* calls Memory *Decisions and memory*; *Knowledge base* calls Evidence *Library*; *Software engineering* writes *API spec* and *Pull request* for two kinds of evidence. General renames nothing.
- **A guard on the list of kinds:** the kinds of evidence a template can name are the API's artifact types; a test in the api crate matches on every `ArtifactType`, so adding one stops it compiling until the list in `repomemo-templates` has it too.
- **Tests (written, not run):** three in the templates crate (renaming sections and kinds with the block written back, unknown ids and bad names refused, the four templates' names), one in the api crate (the kinds a template can name are the artifact types, and their ids are what the API writes), four in the web client (kinds of evidence renamed and the others unchanged, the page title, the palette's listing and matching, the tabs with the usual name as tooltip), and the server's template test now checks the names of General and of each of the four.
- **Checks run on this release:** `cargo check` of the templates, api and server crates with their tests compiles; `rustfmt` shows nothing for the lines changed; `types:check`, `typecheck`, `styles:check`, `a11y:check`, `docs:check` and `version:check` pass. **Not run:** the tests, as asked.

### Upgrading
- Nothing to do.

## V0.2.172 (2026-10-22)

A workspace opens on its template's landing section, and the command palette lists the sections in the template's order.

### Changed
- **`sections.landing`:** a template may name the section a workspace opens on when it is opened without naming one. It is any of the twelve sections except People and Settings (a member would be sent to a page they may not open), and may be one kept under More. Without it a workspace opens on Overview, as always. `GET /v1/workspaces/{id}/template` answers it as `sections.landing`.
- **Opening a workspace:** the dashboard, the workspace list, the side rail, the System area and the command palette now open a workspace by `/workspaces/<id>`, with no section. The page asks for the workspace's template and replaces that address with the landing section (so Back leaves the workspace and does not return to the redirect); if the template cannot be read it opens on Overview. An address that names a section, such as a bookmark of `/workspaces/<id>/map` or an invitation link, opens that section and is not redirected.
- **The command palette:** the sections of the open workspace are listed in the template's order, the sections kept under More after the tabs; People and Settings are still left out for anyone who does not administer the workspace.
- **The four templates:** *Operations and incidents* opens on Review, *Product and decisions* on Memory and *Knowledge base* on Documents; *Software engineering* and General open on Overview.
- **Tests (written, not run):** one in the templates crate (Overview by default, a named landing section, an unknown or administrators-only one refused, where each of the four lands), seven in the web client (the landing section and its fallbacks, the address that names no section and its parsing, the redirect after the template arrives and its fallback to Overview, the palette's order with and without a template and for a member, the palette opening a workspace on its landing section), the server's template test now checks the landing of General and of each of the four, and the router and palette tests follow the new address.
- **Checks run on this release:** `cargo check` of the templates and server crates with their tests compiles; `rustfmt` shows nothing for the lines changed; `types:check`, `typecheck`, `styles:check`, `a11y:check`, `docs:check` and `version:check` pass. **Not run:** the tests, as asked.

### Upgrading
- Nothing to do.

## V0.2.171 (2026-10-22)

The workspace tabs follow the template's order, and the lesser sections sit under a More menu.

### Changed
- **The `sections` block:** a template may list the sections it wants as tabs (`order`) and the ones to keep under *More* (`more`), drawing on the twelve sections a workspace has. A section is named once in all; an unknown or repeated name makes the template invalid. A section named in neither list follows the tabs in the usual order, so nothing disappears. `GET /v1/workspaces/{id}/template` now answers the arrangement as `sections: { primary, more }`; for General that is the usual twelve in the usual order and nothing under More.
- **The tabs:** the row of tabs shows the template's order and ends with a **More** menu holding the lesser sections. The section that is open is always a tab, even when its template keeps it under More, so a person never loses their place. Every section still opens by its address (`/workspaces/:id/map` in a Knowledge base workspace works), and People and Settings stay for owners and administrators whatever a template says; a section this client has no page for is ignored. While the template is still loading the tabs show the usual order.
- **The four templates arrange themselves:** *Software engineering* leads with Overview, Repositories, Retrieval and Evidence and keeps Documents under More; *Operations and incidents* leads with Overview, Evidence, Review and Health and keeps Map, Repositories and Documents under More; *Product and decisions* leads with Overview, Memory, Evidence and Retrieval and keeps Health, Map and Repositories under More; *Knowledge base* leads with Overview, Documents, Retrieval and Evidence and keeps Map and Repositories under More.
- **Workspaces made before templates are unchanged:** they are on General.
- **Tests (written, not run):** five in the templates crate (a template without the block arranges the twelve as always, the order leads and the rest follows, an unknown or repeated section is refused, the list equals the web client's, the four templates' tabs), eight in the web client (the arrangement without a template, in the template's order, for someone who does not administer, with the open section under More, with unknown and repeated sections; the tabs in the usual order, following the template with a More menu, and showing the open section), and the server's template test now checks the arrangement of General and of each of the four.
- **Checks run on this release:** `cargo check` of the templates and server crates with their tests compiles; `rustfmt` shows nothing for the lines changed; `types:check`, `typecheck`, `styles:check`, `a11y:check`, `docs:check` and `version:check` pass. **Not run:** the tests, as asked.

### Upgrading
- Nothing to do.

## V0.2.170 (2026-10-22)

Four templates ship with RepoMemo, as data.

### Added
- **Four templates:** *Software engineering* (code, repositories, design notes; starts with the folders Architecture, Decisions and Runbooks), *Operations and incidents* (runbooks, incidents, on-call knowledge; Runbooks, Incidents, Postmortems and On-call), *Product and decisions* (specs, decisions, research, meeting notes; Decisions, Specs, Research and Meeting notes) and *Knowledge base* (policies, contracts, reports, business documents; Policies, Contracts, Reports and Archive). Each is a JSON file in `crates/templates/definitions/` with its identity block and its starter folders, registered by the build with no Rust edited. Their other blocks arrive with the release that reads each (the tab order is V0.2.171).
- **The picker lists five:** General first, then the four by name. A new workspace made from one has exactly its folders at the top level; General and no template make none. The functional mindmap has a table of the templates.
- **Nothing is required:** existing workspaces stay on General, and a workspace made from a template opens as any workspace does; the folders can be renamed, moved or deleted like any others.
- **Tests (written, not run):** `the_first_four_templates_ship_as_data_with_the_folders_a_new_workspace_starts_with` (the catalogue lists General and the four in order, each validates, names and folders as listed, General has none) and `the_four_first_templates_are_offered_and_each_creates_its_folders` (the route lists five; a workspace made from each has exactly its folders at the top level, reads back its template and opens its overview, metrics and capabilities).
- **Checks run on this release:** `cargo check` of the templates and server crates with their tests compiles; `docs:check` and `version:check` pass. **Not run:** the tests, as asked.

### Upgrading
- Nothing to do. Existing workspaces are unchanged; a person who wants a template's folders in one can create them by hand.

## V0.2.169 (2026-10-22)

The create-workspace form and the setup wizard offer the templates a server has.

### Added
- **A template picker:** when the server offers a template besides General, the form that creates a workspace on the dashboard lists the templates as cards (icon, name and what it is for) with General chosen, and the first workspace of the setup wizard does the same once a name is typed. The workspace is created with the chosen template (`template_id`), so it starts with the template's folders. A server that offers General alone shows no picker, and the form sends no template, exactly as before.
- **Types and hooks:** `useTemplate(accessToken, workspaceId)` reads the template a workspace uses and where it came from, and `useTemplateCatalogue(accessToken)` the catalogue (kept for five minutes, because it changes only when the server restarts), both on the query cache so a page and its sections share one request. The client has `listSharedTemplates`, `getSharedWorkspaceTemplate` and `saveSharedWorkspaceTemplate`, and the types come from the Rust domain crate.
- **Nothing else changes yet:** no section reads the template (the tab order is V0.2.171), and the catalogue holds General alone until V0.2.170, so a person sees no difference.
- **Tests (written, not run):** five in the web client: the picker shows nothing for General alone, offers one radio card for each template with General checked, reports the choice and cannot be changed while busy; the hooks share one catalogue request and keep a workspace's template under its own key, and do not ask when the page has no use for the answer.
- **Checks run on this release:** `typecheck`, `styles:check`, `a11y:check`, `docs:check` and `version:check` pass. **Not run:** the web tests, as asked.

### Upgrading
- Nothing to do.

## V0.2.168 (2026-10-22)

A workspace can be created from a template, with the folders the template starts it with.

### Added
- **`template_id` when creating a workspace:** `POST /v1/workspaces` takes an optional `template_id`. With one, the choice is saved in the new workspace's settings and the template's starter folders are made; without one nothing is saved and no folder is made, exactly as before, and `general` (named on purpose) saves the choice and makes no folder. An id the server's catalogue lacks is refused with `400` before anything is created or counted against the quota.
- **The `starter_folders` block:** a template may list folders (`{"name": "Incidents", "children": ["2026", "Postmortems"]}`): at most 12 folders in all, two levels, names of up to 40 characters with no slash, backslash or control character and no leading or trailing space, and no two folders beside each other with the same name (case aside), which a workspace refuses too. Nothing is created in an existing workspace: choosing a template later never creates, moves or deletes evidence.
- **Made together or not at all:** `StorageEngine::create_shared_workspace_from` inserts the workspace, its saved choice, its starter folders and the "workspace created" activity in one transaction, with the checks a folder made by hand has, so a folder that cannot be made leaves no workspace behind. The feed reads "Created workspace Ops from the Operations and incidents template." for a workspace made from one.
- **In the core:** `RepoMemoCore::workspace_starter(template_id)` turns a catalogue entry into what the storage needs, and `with_template_catalogue` replaces the catalogue (the operator's folder in V0.2.189, tests).
- **No page uses it yet:** the create dialog's picker is V0.2.169 and the four templates that list folders arrive in V0.2.170, so no workspace changes yet.
- **Tests (written, not run):** four in the templates crate (the block read and written back, its bounds in number and levels, the rules for a folder name, no repeated name beside another), one in storage (the folders exist nested as listed, the choice is saved and the feed says where the workspace came from; no folder and no choice without a template; three setups that cannot be made leave no workspace), one in the core (a starter from a catalogue with folders, General makes none, an unknown id makes nothing) and one on the server (no template saves nothing, General saves the choice and makes no folder, an unknown template is `400` and creates nothing).
- **Checks run on this release:** `cargo check` of the templates, storage, api and server crates with their tests compiles; `rustfmt` shows nothing for the lines changed; `types:check`, `typecheck`, `docs:check` and `version:check` pass. **Not run:** the tests, as asked.

### Upgrading
- Nothing to do.

## V0.2.167 (2026-10-22)

Routes to list templates and to read and change a workspace's template.

### Added
- **`GET /v1/templates`:** the catalogue as a picker lists it, `general` first: id, version, name, summary and icon of each, none of the blocks. Any signed-in caller may ask.
- **`GET /v1/workspaces/{id}/template`:** the template in force for the workspace, where it came from (`default` when none was chosen, `workspace` for its own choice, `fallback` with the `reason` when the choice cannot be used on this server), the id the workspace chose and the adjustments saved on it. Any member may read it, as may a read-only token.
- **`PUT /v1/workspaces/{id}/template`:** `{"template_id": "…"}` chooses a template and `{"template_id": null}` returns the workspace to the default. It needs a workspace owner or administrator with a browser session or a personal access token with the `admin` scope (decision 26 with decision 23): a viewer, a member and a read-only or read-and-write token get `403`. An id the catalogue lacks is `404` and writes nothing. A change is `workspace_template_changed` in the activity feed ("Changed the workspace template to General."); choosing the template the workspace already has records nothing, and the adjustments saved on it stay.
- **A template never touches a right:** the routes call the same guards as every other workspace route and nothing in them reads the template; who may open a workspace and what they may do in it is as before.
- **Types:** `TemplateSummary`, `WorkspaceTemplateView` and `UpdateWorkspaceTemplateRequest` are domain types, so the web client's types carry them.
- **No page uses them yet:** the catalogue holds `general` alone until V0.2.170, and the picker is V0.2.169.
- **Tests (written, not run):** `templates_are_listed_read_by_members_and_changed_only_by_administrators` (the catalogue and its shape, a member reads and a stranger gets `403`, a viewer and a read-and-write token cannot change, an unknown id is `404` and writes nothing, the owner's choice is in the feed once and choosing it again adds nothing, an `admin` token resets it), two in the core (the catalogue check, one write, the adjustments kept, the reset; a fallback reported with the choice that could not be used), and the new route added to the list that read, read-and-write and system tokens must be refused on.
- **Checks run on this release:** `cargo check` of the api and the server with their tests compiles; `rustfmt` shows nothing for the lines changed; `types:check`, `typecheck`, `docs:check` and `version:check` pass. **Not run:** the tests, as asked.

### Upgrading
- Nothing to do.

## V0.2.166 (2026-10-22)

A workspace remembers its template, and one resolver gives every part of the product the same answer.

### Added
- **The choice:** a workspace's settings gain one key, `template` (`{ id, version, overrides }`; the domain type `WorkspaceTemplateRef`). It rides in the settings the workspace already has, so there is no migration. `StorageEngine::workspace_template_ref` reads it and `set_workspace_template_ref` writes or clears it, keeping the other settings. A value that is not shaped like a reference (an id that is not a short slug, a version of 0, adjustments that are not an object or are over 16 KiB) reads as no choice.
- **The resolver:** `Catalogue::resolve` settles a saved choice against the catalogue and never fails. No choice gives `general`; an id this server lacks gives `general` with the reason; a choice saved at a higher version than the server's template (a newer server) gives `general` with the reason; a lower saved version uses the server's current file. `RepoMemoCore::effective_template(workspace_id)` reads the choice and returns the template in force, where it came from (`Default`, `Workspace` or `Fallback` with the reason) and the choice as saved. The answer is kept per workspace and reused while the saved choice is the same.
- **Export and import keep it:** a workspace archive already carries the workspace's whole settings object, so an export holds the choice; an import now writes it back into the new workspace when it is well shaped (an id that is not a short slug is left behind). A server that lacks the template shows `general` with the reason, and the choice takes effect if the template is added later.
- **No change a person can see:** no route, setting or page uses the resolver yet; every workspace resolves to `general`.
- **Tests (written, not run):** four in the templates crate (no choice, a choice at an older and the current version, a missing template, a choice from a newer server), one in storage (saved with its adjustments next to other settings, malformed values read as none, an export and import keep the reference and a tampered id is dropped, cleared) and two in the core (General by default and a reason for a missing template, the answer kept until the choice changes).
- **Checks run on this release:** `cargo check` of the templates, domain, storage and api crates with their tests compiles; `rustfmt` shows nothing for the lines changed; `types:check`, `typecheck`, `docs:check` and `version:check` pass. **Not run:** the tests, as asked.

### Upgrading
- Nothing to do.

## V0.2.165 (2026-10-22)

The templates a server knows are the JSON files of one folder.

### Added
- **The built-in catalogue:** `crates/templates/definitions/` holds the built-in templates, one JSON file each, named after the template's id. `build.rs` lists the folder when the crate is built (every `*.json` whose name does not start with `_`, in file-name order) and includes each file in the binary, so adding a template is adding a file and no Rust is edited. `Catalogue::builtin()` is the result; `get(id)` finds a template and `list()` returns them with `general` first and the others by name.
- **A file that cannot be used is left out and named, never fatal:** `Catalogue::load` skips a file that does not validate, is not named after its id, repeats an id, or tries to replace `general` (which is defined in code, so no file can remove or replace it), and returns what was wrong with it. `Catalogue::builtin_problems()` shows those for the built-in folder; a test fails while there are any. The same loader will read the operator's folder (V0.2.189).
- **The folder is empty of templates for now:** the four first templates arrive in V0.2.170, so the catalogue holds `general` alone and nothing a person sees changes.
- **Tests (written, not run):** 5 more unit tests, 18 in all: the built-in catalogue has `general` first and every file of the folder registered; a valid file added to the folder's own files appears in the catalogue with no other edit; the order is by name whatever order the files arrive in; an invalid, misnamed, repeated or `general`-shadowing file is left out and named while the others stay; a catalogue of nothing is `general` alone.
- **Checks run on this release:** `cargo check` of the crate with its tests compiles; `docs:check` and `version:check` pass. **Not run:** the tests, as asked.

### Upgrading
- Nothing to do.

## V0.2.164 (2026-10-22)

The first code of track N: the crate that holds the template format, its limits and its validation.

### Added
- **`crates/templates` (`repomemo-templates`):** a file has a `format` number (`1`) and a `meta` block (`id`, `version`, `name`, `summary`, `icon`). `TemplateDefinition::parse` reads one: it refuses a file over 64 KiB or nested more than 8 levels, an unknown key anywhere (naming the key and its line), a missing key, and then checks the values. `validate` is also usable on a template built in code. Every problem found is returned at once, with its key (`meta.id`) and, when the JSON reader found it, its line and column.
- **Limits, in one module:** an id is 2 to 40 characters of `a-z`, `0-9` and `-` (not starting or ending with `-`, no `--`); a version is a whole number from 1; a name is up to 40 characters and a summary up to 160, one line with no control character; the icon is one of the 32 workspace icons; a format other than the one this server reads is refused with the side that is newer.
- **`general()`:** the template every workspace has today, defined in code and not in a file, so no catalogue or folder can lose it. It leaves out every block, which means "as today".
- **Nothing reads it yet:** no server, route or page uses the crate, so no workspace changes. The catalogue is V0.2.165, the workspace's choice V0.2.166.
- **Tests (written, not run):** 13 unit tests: a valid file; `general` valid and read back unchanged; an unknown key at the top and inside `meta` with its line; a missing block and key; text that is not JSON; the format number; each limit of the id, version, name, summary and icon (valid at the bound, refused one past it, characters counted and not bytes); all problems reported together; file size and depth; and the icon list equal to `web/workspaceIcons.tsx`.
- **Checks run on this release:** `cargo check` of the crate with its tests compiles; `docs:check` and `version:check` pass. **Not run:** the tests, as asked.

### Upgrading
- Nothing to do.

## V0.2.163 (2026-10-22)

Workspace templates are designed and written down before any code reads one. This opens track N (V0.2.163–V0.2.193).

### Changed
- **The design page:** [technical/workspace-templates.md](mindmap/technical/workspace-templates.md) defines a template as a JSON file that names modules RepoMemo already has (sections, health detectors, assistant capabilities, pulse cards, artifact types, ranking signals) and gives them an order, an emphasis or a weight. It lists the blocks of the format with their limits and the release that builds each, what a template can never do (grant or hide a right, run anything, lose evidence, make a clearly better match lose, be required), how the parts fit (one crate, one resolver, no template id in code), how a workspace's template is resolved and falls back to `general`, and how a template is added.
- **The decision record:** [0005](docs/decisions/0005-templates-are-data-over-a-closed-set-of-modules.md), *Templates are data over a closed set of modules*.
- **Decisions 24–28 are taken as proposed:** the track is numbered V0.2.163–V0.2.193 so the tag waits for it and the feature freeze is lifted for it (V0.2.193 restores it); templates are JSON built into the binary plus an optional operator folder; a template never touches rights; existing workspaces stay on `general`; `general` and four templates ship first.
- **No code changes:** no route, setting or migration changes, and no workspace looks or behaves differently.
- **Checks run on this release:** `docs:check` and `version:check` pass. **Not run:** the tests, as asked.

### Upgrading
- Nothing to do.

## V0.2.162 (2026-10-21)

The release checks are repeated after tracks L and M, the fifth security review is closed, and the feature freeze is back on until the tag.

### Changed
- **Checks run on this release:** `npm run typecheck`, `styles:check`, `a11y:check`, `types:check`, `version:check`, `docs:check`, `container:check` and `migrations:check` pass; `cargo deny check` reports *advisories ok, bans ok, licenses ok, sources ok* (it still warns about crates present in two versions, as the policy allows); `npm audit`, development packages included, finds 0 vulnerabilities; `cargo check` of the workspace's server, api, storage, git and ingestion crates with their tests compiled at each release of the track. **Not run in the whole of tracks L and M:** `cargo test`, the web tests, a build, the container and the performance runs, as this repository's rules ask; every test written in the track is listed in its release and waits for the maintainer.
- **The fifth review is closed:** every high, medium and low finding is fixed and its release is in the review's table (S5-1 to S5-10 and E1 to E19); the two informational findings that remain are tracked (S5-11, unmaintained crates with no fixed release) and accepted (S5-12, the access token in `sessionStorage`). The eleven performance findings (P12 to P21) were built in V0.2.149–V0.2.161; their before and after figures are the maintainer's runs.
- **The feature freeze is restored (decision 20):** from this release until the `v0.2.0` tag only the checks of the clean install and the fixes it finds go in.
- **The clean-install page** has the notes for the V0.3.0 changelog on tracks L and M (below) and one more line in its checklist for what only a run can show.
- **A tidy-up in the review:** the release cell of finding E1 was written twice by V0.2.148 and is written once.

### Notes for the V0.3.0 changelog: tracks L and M (V0.2.130–V0.2.162)
- **Security:** the TLS stack and a spreadsheet library were updated and the web toolchain moved to Vite 8, so `cargo deny` and `npm audit` are clean; LibreOffice previews are opt-in, need a fixed release, run with a locked profile and, in the container, in a container of their own with no network; the server container is read-only with no capabilities; AI provider addresses are limited to a host and a port; the `admin` token scope, activity by role, organization roles that reach only their own workspaces, refresh tokens that do not sign a person out everywhere, IPv6 limits by /64, stricter new addresses and names, escaped logs and a cleared git environment; links that leave the site say so; every artifact route checks rights before it reads a document; jobs are cancelled by their starter or an administrator; JSON bodies stop at 1 MiB.
- **Speed and memory:** uploads stream to disk; the file list pages in SQL and the web client holds one page; the artifact page uses the stored text; a repository sync with nothing new, and the last-author pass, read almost nothing; a search across workspaces runs in parallel with one embedding call; read connections cache and map more; JSON is compressed; a full server refuses at once.
- **Operations:** new settings `REPOMEMO_PREVIEW_ROOT`, `REPOMEMO_UPLOAD_INFLIGHT_MB`, `REPOMEMO_SQLITE_CACHE_MB`, `REPOMEMO_SQLITE_MMAP_MB`, `REPOMEMO_MAX_IN_FLIGHT` and `REPOMEMO_MAX_IN_FLIGHT_PER_ACCOUNT`; migrations 0037 to 0041; the `previews` compose profile.

### Upgrading
- Nothing to do. [UPGRADING.md](UPGRADING.md) lists, release by release, what changed for an operator in tracks L and M.

## V0.2.161 (2026-10-21)

When the server is full it says so at once, instead of letting requests queue until they time out.

### Changed
- **Load shedding (P21):** past the database's read connections and its one writer, requests queued until the request timeout, so a flood (or one script making hundreds of calls at once) made every other person wait. The server now counts the requests it is serving. Past `REPOMEMO_MAX_IN_FLIGHT` (16 for each read connection, 64 by default) a new request is refused at once with `503 overloaded` and `Retry-After: 1`; past `REPOMEMO_MAX_IN_FLIGHT_PER_ACCOUNT` (8 for each read connection, 32 by default) a new request of the same account or personal access token is refused at once with `429` and `Retry-After`, while the others are served. `0` turns either cap off.
- **What keeps room of its own:** health checks (`/health*`), sign-in and the other `/v1/auth/*` calls, setup, invitation links and the event streams are outside the count, so a busy server can still be checked, signed in to and watched. The refusal costs a counter and no database read; the account is read from the session token's signature (a forged token cannot spend another account's share) or the personal access token, as the request arrives.
- **Counted and shown:** `repomemo_requests_in_flight`, `repomemo_requests_shed_server_total` and `repomemo_requests_shed_account_total` in `GET /v1/system/metrics`, and both caps in System › Overview with the other concurrency settings.
- **Why not `tower`'s `ConcurrencyLimit`:** it makes the next request wait for a place; the point is not to wait, so a small counting middleware (`load.rs`) refuses instead.
- **Tests (written, not run):** `requests_past_the_cap_are_refused_at_once_and_health_and_sign_in_keep_room` (two uploads held open: a second request of the same account gets `429`, another account is served, a third account gets `503` once the server is full, health and sign-in still answer, and everything is served again when the uploads end, with the refusals in the metrics).
- **Checks run on this release:** `cargo check` of the server with its tests compiles; `rustfmt` shows nothing for the lines changed; `docs:check` and `version:check` pass. **Not run:** the tests, as asked.

### Upgrading
- Nothing to do. A script that makes more than 32 calls at once for one account retries after the `Retry-After` it is given, or the cap is raised (`REPOMEMO_MAX_IN_FLIGHT_PER_ACCOUNT`).

## V0.2.160 (2026-10-21)

JSON answers are compressed for a client that asks, without a proxy in front.

### Changed
- **Compressed answers (P20):** the server sent every answer as it was; only the container's Caddy compressed, so direct use and the nginx recipe moved large JSON (a search, a file list, a page) uncompressed. A client that sends `Accept-Encoding: gzip` or `br` now gets JSON, newline-delimited JSON, OpenMetrics and text answers of 1 KiB or more compressed (`Content-Encoding`, `Vary: Accept-Encoding`).
- **What is left alone:** event streams (each event must arrive at once), downloads (`Content-Disposition: attachment`), answers to range requests, files and formats that are compressed already (PDF, images, archives), and answers under 1 KiB. An answer that carries a `Content-Encoding` already is not compressed again, so behind Caddy nothing doubles.
- **Dependencies:** `tower-http`'s `compression-gzip` and `compression-br` features add three small crates, `async-compression`, `compression-codecs` and `compression-core` (the compressors themselves, `flate2` and `brotli`, were in the lock already). `cargo deny` was not run here.
- **Tests (written, not run):** `json_is_compressed_for_a_client_that_asks_and_streams_and_downloads_are_not` (the same long answer plain, with gzip and with Brotli, each at least four times smaller; a short answer, a download and an event stream carry no encoding).
- **Checks run on this release:** `cargo check` of the server with its tests compiles; `rustfmt` shows nothing for the lines changed; `docs:check` and `version:check` pass. **Not run:** the tests and `cargo deny check`, as asked.

### Upgrading
- Nothing to do. Behind nginx or another proxy that compresses, leave compression off for `/v1/` (the operations guide says so).

## V0.2.159 (2026-10-21)

The connections that read the database get a larger cache and memory-mapped reads.

### Changed
- **SQLite read tuning (P19):** the connections that serve reads ran with SQLite's defaults: a page cache of about 2 MiB, no memory-mapped reads and temporary tables on disk. Each read connection now has a page cache of 32 MiB, may map up to 256 MiB of the database into memory and keeps its temporary tables in memory. The connection that writes keeps SQLite's defaults.
- **Two new settings, both in System › Overview:** `REPOMEMO_SQLITE_CACHE_MB` (32; 0 keeps SQLite's own cache; at most 4,096) and `REPOMEMO_SQLITE_MMAP_MB` (256; 0 turns mapping off; at most 65,536). The cache is per read connection, so four readers hold up to four times the setting. Both are documented in the commands reference and the production guide.
- **The restore proof opens the restored database without them** (it reads one connection and closes it).
- **Tests (written, not run):** `read_connections_get_a_larger_cache_memory_mapped_reads_and_memory_temporaries` (the three pragmas as a reader reports them, the writer without them, the operator's values, and 0).
- **Checks run on this release:** `cargo check` of the storage and server crates with their tests compiles; `rustfmt` shows nothing for the lines changed; `docs:check` and `version:check` pass. **Not run:** the tests, and `performance_at_scale` in a release build with and without the settings, which is the maintainer's run; `performance.md` has the row to fill (below).

### Upgrading
- Nothing to do. On a host with little memory, lower `REPOMEMO_SQLITE_CACHE_MB` and `REPOMEMO_SQLITE_MMAP_MB`. Try: run `cargo test -p repomemo-storage --release performance_at_scale -- --ignored --nocapture` with both settings at `0` and again with the defaults, and record the figures in `performance.md`.

## V0.2.158 (2026-10-21)

Search across workspaces stops reading the workspaces one after another and stops embedding the query again in each.

### Changed
- **Workspaces searched in parallel (P18):** up to 30 workspaces were searched one after another. The caller's role and each workspace's AI policy are now settled for every workspace first, then the searches run four at a time (`CROSS_SEARCH_PARALLEL`). The answers come back in the order of the workspaces, so the merged list, its order and the per-workspace notes are what they were.
- **One embedding call for each provider (P18):** the query was embedded again in every workspace, so 30 workspaces on one provider made 30 calls to it. It is now embedded once for each provider the workspaces use (the same kind, address, embedding model and key), and the vector is handed to each workspace's search. A workspace whose AI policy keeps meaning search from the caller still never has the query sent to its provider; a provider that fails leaves its workspaces on keywords, as before.
- **Tests (written, not run):** `a_search_across_workspaces_embeds_the_query_once_per_provider` (five workspaces with the same embedding provider: one embedding call for the search, every workspace answered by meaning, and the same results as searching each workspace alone).
- **Checks run on this release:** `cargo check` of the api and server crates with their tests compiles; `rustfmt` shows nothing for the lines changed; `docs:check` and `version:check` pass. **Not run:** the tests, as asked.

### Upgrading
- Nothing to do.

## V0.2.157 (2026-10-21)

The last-author pass reads the new commits, not the whole history, on each sync.

### Changed
- **Incremental last authors (P17):** every sync read the history from the head (up to 20,000 commits) to learn who last changed each file. After an earlier sync it now reads only `previous..commit`, found ancestor by ancestor with `git merge-base --is-ancestor`, and lays those commits over the author, date and commit stored with each file: a one-commit change reads one commit. A file changed in the range takes the newest commit that lists it; the others keep what is stored.
- **When the full pass still decides:** there is no earlier sync, the previous commit is not behind the new one (a force-push or a rewritten history, or a commit that is no longer in the server's copy), or the new commits fill the 20,000-commit window so some might not be reached.
- **Tests (written, not run):** `the_last_authors_of_a_range_of_commits_are_read_without_the_rest_of_the_history` (git: one new commit gives one file; nothing new gives an empty answer; the newest of two commits wins; a full window, a rewritten history and an unknown commit give none) and `the_last_authors_after_a_new_commit_keep_what_was_stored` (api: the file the commit changed takes it and the other keeps the stored one; a squashed history is read in full).
- **Checks run on this release:** `cargo check` of the git and api crates with their tests compiles; `rustfmt` shows nothing for the lines changed; `docs:check` and `version:check` pass. **Not run:** the tests, as asked.

### Upgrading
- Nothing to do.

## V0.2.156 (2026-10-21)

A repository sync with nothing new stops after finding the commit.

### Changed
- **A sync with nothing to do stops early (P16):** every sync listed the whole tree, read up to 20,000 commits for the last authors, read `CODEOWNERS` and loaded every stored file of the repository, only to find nothing to change. A sync now resolves the commit first and stops there when the commit, the include and exclude rules (and the branch) and the indexer version are those recorded at the last complete sync, that sync was not stopped and failed no file, no file of the repository is waiting to be indexed and the item standing for the repository is indexed. It records the time and a report that says nothing changed, the job completes, and the web client shows *Nothing changed*. A pushed commit, a changed rule or branch, a new indexer version, an interrupted pass or a failed file make the sync a full one as before. The webhook and the poll go through the same path.
- **What is recorded:** the settings and the indexer version of each complete sync (`last_synced_settings`, `last_synced_indexer_version` in the source's metadata, no migration), and `RepoSyncReport.nothing_changed` (a new field, empty in older reports). A repository from before this release has no recorded settings, so its first sync after the upgrade is a full one.
- **Tests (written, not run):** `a_sync_with_nothing_new_stops_after_the_commit_is_resolved` (a second sync on the same commit changes nothing and says so; a changed rule is worked through, then nothing again; a new commit is worked through). `repo_sync_at_scale` already syncs a repository with nothing changed; its figure for 100,000 files (target under a second) is the maintainer's to record.
- **Checks run on this release:** `cargo check` of the api and server crates with their tests compiles; `rustfmt` shows nothing for the lines changed; `npm run typecheck`, `types:check`, `docs:check` and `version:check` pass. **Not run:** the tests and the release-build run, as asked.

### Upgrading
- Nothing to do. The first sync of each repository after the upgrade is a full one.

## V0.2.155 (2026-10-21)

The web client stops asking for a workspace's whole file list.

### Changed
- **A page holds the newest 500 files (P15b):** every workspace page, section, picker and the command palette called `GET …/artifacts` without a limit, and again after each live event, so a workspace of 100,000 files moved tens of megabytes on each refresh. The web client now reads one page of 500 files and the total (`listSharedArtifactWindow`, `useArtifacts`), and a live event fetches that one page again. The evidence section and the pickers no longer use that page: see *Completed afterwards (V0.2.155 Bis)* below.
- **The command palette searches the server (P15b):** it read the whole file list every time it opened. It now sends the longest word typed (after a short pause) and keeps the files that hold every word, at most 50.
- **`POST …/artifacts/query` answers like the list:** each file carries its folder and repository (`SharedArtifactSummary`, additive), `X-Total-Count` is on every answer, and a call without `limit` gets the first 2,000 matches with `X-RepoMemo-Truncated: true` when there were more, as the file list does since V0.2.154.
- **Completed afterwards (V0.2.155 Bis, two follow-up commits without a version of their own):**
  - *The pickers.* The memory card, task and replacement-evidence pickers search the server as the person types (`Combobox` gets `onSearch`, `useArtifactPicker` feeds them), so a file of any age can be chosen; the chosen file keeps its label when a later search no longer lists it, and a memory citation is made from any file chosen.
  - *The evidence section's views are cut by the server.* The files on screen are one page of 24 of the folder open (or of the whole ledger while a name or a file type is chosen), asked with `POST …/artifacts/query`, which now also takes `folder_id` or `top_level`, `ledger_only` (leave out the files a repository tracks) and `kinds` (alternatives of: artifact types, types left out, extensions, extensions left out). `lib/evidenceView.ts` turns each file type of the page into the filter that finds what `fileCategory` would name that type. The documents view is the same page with the document formats. Pagination, the count and the bulk selection (which spans pages) use the server's total.
  - *Folder counts come with the folder list.* `Folder.file_count` (files directly in a folder, one grouped query) replaces counting the files held; the confirmation for deleting a folder sums it over the subtree. Moving a file refetches the page.
  - *Tests (written, not run):* `the_artifact_query_filters_by_folder_repository_and_kind_and_folders_count_their_files` (storage), `test/evidenceView.test.ts` (every file lands in exactly the category the page gives it; the page request), a `Combobox` test for the server-search mode, and the section tests' mocks. `npm run typecheck`, `styles:check`, `a11y:check`, `types:check` pass; `cargo check` of the storage and server crates with their tests compiles.
- **Tests (written, not run):** `test/artifactWindow.test.ts` (one page of 500 with the total; a search always carries a limit the server accepts) and `test/commandPalette.test.tsx` (the palette mocks the search).
- **Checks run on this release:** `npm run typecheck`, `styles:check`, `a11y:check`, `types:check`, `docs:check` and `version:check` pass; `cargo check` of the server with its tests compiles. **Not run:** the web tests, as asked.

### Upgrading
- Nothing to do. A script that read `POST …/artifacts/query` without a limit now gets at most 2,000 matches and a header that says so.

## V0.2.154 (2026-10-21)

The file list of a workspace is read a page at a time, in SQL, so a workspace of 100,000 files no longer sends them all on every refresh.

### Changed
- **`GET /v1/workspaces/{id}/artifacts` pages in SQL (P15a):** it loaded every file row, two whole-workspace maps (folders and repositories) and cut the page in memory, which on a large repository meant tens of megabytes read and thrown away for each page. It now reads one page (`StorageEngine::list_artifacts_page`, with a count), and looks up the folders and repositories of the files on that page only. A new index (migration 0041, `idx_artifacts_workspace_updated` on workspace, change date and path) holds the order the list is in, so a page is the first rows of the index and not a sort of the workspace. The order is unchanged: newest change first, then path.
- **`X-Total-Count` on every answer, and a cap without `limit`:** a call that asks for a page gets at most 500 files (as before); one that does not gets the first 2,000, with `X-RepoMemo-Truncated: true` when there were more. The web client still asks for the full list until V0.2.155, so a workspace of more than 2,000 files shows its 2,000 newest in the meantime.
- **`PageQuery::apply` is gone** (its only user was this route) with its unit test.
- **A case in `performance_at_scale`:** it adds `REPOMEMO_PERF_LISTED_FILES` bare files (100,000 by default) in one statement and prints the p95 of a page with its folders and repositories (target 100 ms in a release build) and the time of a page at offset 50,000. The figure is the maintainer's to record in `performance.md`.
- **Tests (written, not run):** `a_page_of_files_is_read_in_sql_with_the_total` (storage: 2,100 files, a page capped at its size, the same order as the whole list, the last and an empty page, folders and repositories for given ids) and `the_file_list_is_paged_in_sql_with_its_total` (server: the total, a slice of the same order, a file's folder on its page, an offset past the end).
- **Checks run on this release:** `cargo check` of the storage and server crates with their tests compiles; `rustfmt` shows nothing for the lines changed; `docs:check`, `version:check` and `migrations:check` pass. **Not run:** the tests and the release-build run, as asked.

### Upgrading
- Migration 0041 adds one index and runs on first start. A script that read the whole file list in one call now gets at most 2,000 files and a header that says so: ask in pages (`limit` up to 500, `offset`) or through `POST …/artifacts/query`.

## V0.2.153 (2026-10-21)

Uploads stop being held whole in memory, and the bytes arriving at once get a cap.

### Changed
- **An upload or a new version is streamed to disk (P14):** `POST …/artifacts/upload` and `POST /v1/artifacts/{id}/versions` read the whole body into memory, copied it (`body.to_vec()`), then hashed and wrote it, so a 500 MiB upload held up to three copies at once. The body is now written chunk by chunk to a temporary file in `<data>/tmp/`, hashed and counted as it arrives, and moved into the blob store with a rename, under the same hash and the same `blobs` row as before (an identical file already stored keeps its blob and the received copy is deleted). What the server holds in memory is one chunk. A refused or interrupted upload leaves nothing in the folder, which is also emptied at start.
- **Room is checked twice (P14):** against the announced `Content-Length` before any of the body is read (this check also counts toward the hourly upload limit), then against the bytes that arrived. The limit `REPOMEMO_MAX_UPLOAD_MB` is enforced as the bytes come, whatever the request announces, and answers a JSON `413` (`payload_too_large`); an announced size above it is refused before the body is read.
- **A cap on uploads in flight (P14, new setting `REPOMEMO_UPLOAD_INFLIGHT_MB`, 512):** each upload takes room in a semaphore (one permit a KiB) for its announced size, the largest allowed when it announces none, until it is stored or refused. When the room is taken the answer is `503 uploads_busy` with `Retry-After: 5`, instead of the server taking on more than it can write.
- **Correction to V0.2.152:** that release put the upload limit on the workspace import as a route layer. An import streams its body and is bound by `REPOMEMO_MAX_IMPORT_MB`, which `DefaultBodyLimit` never applied to, so the layer did nothing and is gone; the upload routes' layers went the same way, since the limit is now enforced as the body is received.
- **Tests (written, not run):** `a_received_file_is_hashed_as_it_arrives_and_moved_into_the_blob_store` (storage: a file in pieces, its hash, a rename and not a copy, the same content twice, an abandoned or unused file leaves nothing) and `uploads_are_streamed_to_disk_and_the_bytes_in_flight_are_capped` (server: a file in six pieces stored with the right hash and bytes, a body that grows past the limit and an announced size above it refused with `413`, a second upload refused with `503` and `Retry-After` while a first one still arrives, and accepted once it ends).
- **Checks run on this release:** `cargo check` of the storage, api and server crates with their tests compiles; `rustfmt` shows nothing for the lines changed; `docs:check` and `version:check` pass. **Not run:** the tests, and an upload of 500 MiB with the memory watched, as asked; that is the maintainer's try (below).

### Upgrading
- A new setting, `REPOMEMO_UPLOAD_INFLIGHT_MB` (512), is read at start; nothing needs to change. A script or proxy that gave up on a `503` should retry after `Retry-After`. Try: upload a file of a few hundred MiB (with `REPOMEMO_MAX_UPLOAD_MB` raised) and watch the server's memory stay flat; start two at once with the new setting at `1` and see the second one wait.

## V0.2.152 (2026-10-21)

Request bodies get a limit that fits the route: a comment or a search is never as large as an upload.

### Security
- **Body limits per route (S5-5):** the server took a body of up to `REPOMEMO_MAX_UPLOAD_MB` (up to 1 GiB) on every route of a signed-in caller, so a request for a comment or a search could make it hold that much. The limit is now 1 MiB for every JSON route, and the upload limit applies only to the three routes that take a file: a new file (`POST /v1/workspaces/{id}/artifacts/upload`), a new version (`POST /v1/artifacts/{id}/versions`) and a workspace import (`POST /v1/workspaces/import`). Above its limit a request is answered `413`. The public routes (64 KiB), the webhook receivers (1 MiB) and the avatar (512 KiB) keep their own.

### Changed
- **Notes are JSON, so a note above 1 MiB is refused** (`POST …/artifacts/text`); a larger text goes up as a file.
- **`scripts/list-routes.mjs` reads a route layer whose limit is a setting** (`DefaultBodyLimit::max(config.max_upload_bytes)`), and the new-version route is registered as two `.route` calls (`GET` and `POST`) so the listing, and `docs:check` with it, still see all of them.
- **Tests (written, not run):** `json_bodies_stop_at_a_mebibyte_and_files_keep_the_upload_limit` sends 2 MiB of JSON (a comment, a note, a search) with the upload limit at 4 MiB and gets `413` each time, then an ordinary comment (`201`), a 3 MiB file and a 3 MiB new version (accepted) and a 5 MiB file and a 5 MiB new version (`413`).
- **Checks run on this release:** `cargo check` of the server with its tests compiles; `rustfmt` shows nothing for the lines changed; `docs:check` and `version:check` pass. **Not run:** the tests, as asked.

### Upgrading
- A script that posts a note of more than 1 MiB as JSON now gets `413`: upload it as a file. Nothing else changes, and `REPOMEMO_MAX_UPLOAD_MB` keeps its meaning for files.

## V0.2.151 (2026-10-21)

A job can be stopped only by the person who started it or an administrator, and a re-index of one file no longer starts a task of its own for every request.

### Security
- **Who may cancel a job (S5-9):** any workspace writer could cancel any job of the workspace, an administrator's whole-workspace re-index included. A job now records who started it (migration 0040, `indexing_jobs.started_by`), and `POST /v1/jobs/{id}/cancel` accepts that person or an owner or administrator of the workspace (a system administrator counts as one), and answers `403` to everyone else. A job nobody started (a webhook or scheduled sync, a restart) and the jobs of earlier versions have no starter, so only an administrator stops them. The whole-workspace re-index, a repository sync and a connector sync started by a person record them as the starter.

### Changed
- **A re-index of one file goes through the indexing queue (E8):** `POST /v1/artifacts/{id}/index` started a background task for every request, with no limit and no check for one already running. The file now joins the queue the uploads use, behind the same cap on files indexed at once, once per file. The answer is still `202` with the job, made when the request arrives; asking again while that job waits or runs returns the same job, so ten requests make one. If an automatic pass for the same file is waiting or running, it takes the work, or the file is queued again when that pass ends. A failed manual re-index is not retried (the job carries the reason; ask again), as before.
- **Tests (written, not run):** `ten_requests_for_one_file_make_one_job` (a queue whose worker has not started: ten requests, one job, one entry in the queue, the starter recorded, another file with a job of its own) and `a_job_is_cancelled_by_its_starter_or_an_administrator` (a member cannot cancel an administrator's re-index or another member's; the starter and an administrator can).
- **Checks run on this release:** `cargo check` of the storage, api and server crates with their tests compiles; `rustfmt` shows nothing for the lines changed; `docs:check`, `version:check` and `migrations:check` pass. **Not run:** the tests, as asked; so the new migration has not run against a real database here.

### Upgrading
- Migration 0040 adds a nullable column and runs on first start. Jobs started before it have no starter, so only an owner or administrator cancels one that is still running.

## V0.2.150 (2026-10-21)

The artifact page stops reading a document again every time it opens.

### Changed
- **The page's text comes from what indexing stored (E7b, P13):** opening a Word, Excel, PowerPoint, PDF, OneNote or email file started a reader process on every view, although indexing had already stored the text as passages. For an indexed file the page now joins the stored passages in order, with the same 120,000-character cap and the same "shortened" mark, and starts no reader. A file that is not indexed yet, or has a new version waiting to be indexed (a new version clears the index mark), is read once, on the blocking pool rather than on an async worker thread. An indexed file with no text, such as a scanned PDF, shows no text without being read again.
- **The stored file is no longer read whole for the preview:** the plain preview (`StorageEngine::get_artifact`) read the entire file into memory to show its first 120,000 bytes, and a document was then read a second time for its text. It now reads at most the first 120,001 bytes.
- **Tests (written, not run):** `the_page_of_an_indexed_document_starts_no_reader` (in `repomemo-api`) uploads an email file and shows its page is read once before indexing, not at all after it, and once again after a new version; the server's test of V0.2.149 now expects no reading for an owner's page of an indexed file.
- **Checks run on this release:** `cargo check` of the storage, api and server crates with their tests compiles; `rustfmt` and `clippy` show nothing new for the lines changed; `docs:check` and `version:check` pass. **Not run:** the tests, as asked.

### Upgrading
- Nothing to do.

## V0.2.149 (2026-10-21)

Rights before reading: a request for a file the caller may not see no longer starts a document reader.

### Security
- **Every artifact route checks rights on the summary first (E7a, P12):** ten handlers (`GET`, `PUT` and `DELETE /v1/artifacts/{id}`, the move, the lifecycle read, write and history, the re-index and the new-version upload) loaded the whole artifact to learn its workspace. For a Word, Excel, PowerPoint, PDF, OneNote or email file that meant reading and extracting the document, in a reader process, on a runtime worker thread, and only then asking whether the caller was a member. They now read the artifact's one summary row, check the caller's role, and only then do the rest. `GET /v1/artifacts/{id}` still reads the document for a caller who may see it. A non-member asking for a large PDF gets `403` before any reader starts.
- **The comment and task routes read the summary too:** listing and adding comments, and attaching evidence to a task, no longer load the artifact's passages to find its workspace.

### Changed
- **A counter of readings:** the ingestion crate counts the readings it starts (`started_readings`) and keeps the file names of the last 512 (`recent_readings_of`), re-exported by `repomemo-api`, so a test can show that a request started none. Nothing outside tests reads them.
- **Tests (written, not run):** `a_refused_artifact_request_starts_no_document_reader` uploads an email file, waits for its indexing, then calls fifteen routes as an account outside the workspace; each answers `403` and no reading of the file is started; the owner's own `GET` then reads it once.
- **Checks run on this release:** `cargo check` of the server with its tests compiles; `rustfmt` shows nothing for the files changed and `clippy` nothing new; `docs:check` and `version:check` pass. **Not run:** the tests, as asked.

### Upgrading
- Nothing to do.

## V0.2.148 (2026-10-21)

The container layout stops giving a document that escapes LibreOffice the database, the key file and the network. This closes track L.

### Security
- **A locked-down server container (S5-8):** the compose file runs the server with `read_only: true` (`/tmp` is a `tmpfs`), `cap_drop: [ALL]` and `security_opt: [no-new-privileges:true]`. It writes only to the data volume, the previews volume and the socket volume.
- **LibreOffice in a container of its own (E1c):** a new `converter` service, behind the compose profile `previews`, has **no network** (`network_mode: none`), the same lock-down, and sees only two volumes: `repomemo-previews` (the converted PDFs, the work folders and LibreOffice's profile) and `soffice-socket`. It no longer shares a container with the database, `secret.key` and the network. The server reaches it through `repomemo-soffice-wrapper` (`REPOMEMO_SOFFICE_WRAPPER`), which sends LibreOffice's arguments over the socket with `socat`, NUL-separated; a listener in the converter runs one `repomemo-soffice-run` per request, which accepts only LibreOffice's own flags, the `pdf` format and paths inside the previews volume, and answers with LibreOffice's output and an exit status. Version probing and conversions go through the same path, so the release check of V0.2.137 applies unchanged. With previews off the converter is not started; with the profile missing the server finds nobody answering and converts nothing, and says so.
- **`REPOMEMO_PREVIEW_ROOT` (new setting):** the folder that holds the `previews` folder, the data folder when unset. The compose file points it at the previews volume, which is how the converter can see the work folders without seeing anything else of the data. System › Overview counts the previews there.

### Changed
- **`npm run container:check` asserts each setting:** the server's read-only root, `/tmp` `tmpfs`, dropped capabilities and `no-new-privileges`; for the converter, the same, no network, the `previews` profile, no data volume, no published port, the two shared volumes, the Dockerfile stage and the three scripts it copies; the `docker/*.sh` files stay on LF. Tried on planted examples: with the server's lock-down removed, a network on the converter, or the data volume given to it, it reported each and exited with 1; on the tree it passes.
- **`REPOMEMO_WITH_LIBREOFFICE` is gone from the compose file** and `.env.example` (the build argument still exists for a plain `docker build`); `COMPOSE_PROFILES=previews` takes its place.
- **Checks run on this release:** `cargo check` of the server with its tests compiles and `rustfmt` and `clippy` show nothing new for the files changed; `container:check`, `docs:check`, `version:check` and `migrations:check` pass. **Not run:** `docker compose build` or `up`, so the converter, its scripts and the socket have never run; that is the maintainer's try (below).

### Upgrading
- Rebuild (`docker compose up -d --build`). To keep layout-accurate previews set `COMPOSE_PROFILES=previews` and `REPOMEMO_OFFICE_PREVIEWS=true` in `.env`. Try: with them on, open a Word file; System › Overview should name LibreOffice's release, or say why it is refused (Debian 13's may be older than 26.2.5). A custom compose file must let the server write only to its volumes and `/tmp`.

## V0.2.147 (2026-10-21)

Two small fixes in the web client: links that look local but leave the site, and a button that should not open a file that can carry macros.

### Security
- **Links to another host say so (E10):** Markdown links in notes, answers, memory cards and READMEs show their target host and open in a new tab without an opener or referrer, but only for addresses that parsed on their own. `//evil.example/x` and `\\evil.example\share` do not (no scheme), so they were drawn like local links, with no host and in the same tab, although a browser takes them to another host. The address is now read the way the browser reads it from the page, and anything that leaves the page's own origin is external. Local paths, `#section` anchors and `mailto:` links are as before; a full address of this same site is now treated as local.
- **No *Open in Office* for macro formats (E16):** `.xlsm`, `.xlsb`, `.xltm`, `.xlam`, `.docm`, `.dotm`, `.pptm`, `.potm`, `.ppsm`, `.sldm` and `.ppam` are still read here (text, tables, slides) and downloaded, but the button that hands the file to the desktop application through a link is no longer shown for them. The rest of the Office formats keep it, in view mode.

### Changed
- **The preview note** for a server with the layout preview off now says an administrator turns it on in the server settings (it used to say to install LibreOffice, which is no longer enough since V0.2.137).
- **Tests (written, not run):** `//host` and `\\host` addresses give their host and render as a new-tab link with the host named; local paths, anchors and the page's own address do not; the macro formats (and unknown files) get no *Open in Office*, the others do.
- **Checks run on this release:** `npm run typecheck`, `docs:check` and `version:check` pass. The web tests were not run.

### Upgrading
- Nothing to do.

## V0.2.146 (2026-10-21)

The last low findings on input: odd characters in new addresses and names, a log line a stranger could forge, and the server's secrets visible to local git.

### Security
- **New addresses are strict (E13):** an address for a new account, an invitation, an address change or the first administrator needs one `@` between two non-empty parts and no white space, control or text-direction character. Looking an address up is as lenient as before, so every existing account signs in.
- **New names refuse hidden characters (E13):** display, workspace and organization names (and renames) no longer take control characters or the bidirectional marks and overrides that make `gpj.exe` read as `exe.jpg`. Names stored earlier are left. A file title taken from a file name or pasted text keeps only the length check, so a repository holding an odd file name still syncs.
- **No forged log lines (E12):** the sign-in address was written to the text console log as typed, so a line break could start a line of its own. It is now written escaped (`email = "ada@example.com\nINFO …"`), one line per sign-in whatever the address; the same for the warning about a listed administrator with no account.
- **Local git starts empty (E18):** git processes for local repositories inherited the server's whole environment (its `REPOMEMO_*` settings, keys, tokens). They now start from an empty environment with only what git needs to run (`PATH`, home and temporary folders, `LANG`, `LC_ALL`) plus the `GIT_*` the server sets, as the remote commands already did.

### Changed
- **Clippy:** the tuple read for a refresh token (V0.2.144) became a named type.
- **Tests (written, not run):** a new account refuses spaces, line breaks, NUL, text-direction marks, a second `@` and an empty side, while a lookup with capitals and spaces still finds the account; names and an address change refuse hidden characters; a hostile address in Debug form has no line break; local git keeps `PATH`, `HOME` and `LANG` and drops `REPOMEMO_*`, tokens, keys and `GIT_DIR`.
- **Checks run on this release:** `cargo check` of the git and storage crates and the server with their tests compiles, `rustfmt` is clean for what changed apart from hunks that were already there and `clippy` reports nothing new; `docs:check` and `version:check` pass.

### Upgrading
- Nothing to do for existing accounts and names. Scripts that create accounts or invitations with unusual addresses get `400`.

## V0.2.145 (2026-10-21)

Sign-in protection can no longer be sidestepped by changing IPv6 address, and a burst of sign-ins cannot exhaust the server.

### Security
- **IPv6 per /64 (E11):** limits and lockouts were counted per address, but whoever holds an IPv6 connection holds a whole /64, billions of addresses, each with its own budget. They are now counted by the /64 (an IPv4-mapped address by its IPv4); the sign-in, registration, refresh, setup-code and webhook-failure limits and the per-account-and-address lockout use it. Logs keep the full address.
- **A cap on password hashing:** Argon2 takes tens of milliseconds and a good deal of memory, and every sign-in ran it on the blocking pool without limit. Hashing and checking passwords now take a place at a gate of twice the number of cores; past it the answer is `429` with `Retry-After`, instead of a growing queue. Unknown addresses still cost the same time as wrong passwords.
- **Bounded maps:** the rate limiter and the lockout table held a key for every address seen until the hourly maintenance. Each now holds at most 100,000 keys; past that, windows that ran out are dropped, then the oldest, and a lockout is never evicted to make room.

### Changed
- **Tests (written, not run):** the /64 key (one prefix shares one budget, another has its own, an IPv4-mapped address is IPv4), the gate (two places, a third is refused with 429, a finished hash gives its place back), and both maps under a flood of 500 keys with a bound of 20, with a lockout surviving it.
- **Checks run on this release:** `cargo check` of the server with its tests compiles, `rustfmt` is clean for what changed apart from hunks that were already there, `clippy` reports nothing new; `docs:check` and `version:check` pass. A load of 200 simultaneous sign-ins was not tried.

### Upgrading
- Nothing to do. People on one IPv6 prefix share one sign-in budget.

## V0.2.144 (2026-10-21)

Reopening a browser with several RepoMemo tabs, or two tabs crossing the hour together, no longer signs the person out everywhere.

### Security
- **Two tabs refreshing at once end no session (S5-4):** the refresh cookie is shared by the tabs of a browser, and renewing rotates it. When two tabs renewed at the same moment, the second presented the cookie the first had just rotated, which read as token theft and revoked every session of the account, the first tab's new one included. A refresh token now remembers why it was revoked and when it was rotated (migration `0039_refresh_token_reasons.sql`). A token rotated less than 30 seconds ago and presented again is **refused with `409`** and nothing is revoked; a token rotated longer ago, or revoked for any other reason (signing out, ending sessions), is still reuse and ends them all. A request that loses the race for the same token in the database gets the same `409`. Tokens that existed before this release have no reason recorded, so presenting one is reuse, as before.
- **The cookie is left alone on a `409`:** the server clears the cookie on a `401` from `/v1/auth/refresh`, which here would have deleted the new cookie the first tab had just received; a `409` does not touch it.
- **The web app renews one tab at a time:** a renewal runs inside the browser's `navigator.locks` lock `repomemo-refresh`, so the second tab waits and presents the cookie the first one received; where the lock is missing, or a `409` still comes back, the app asks again after 300 ms, twice at most, and keeps the session if that fails.
- **`__Secure-` prefix over HTTPS (E14):** when the public address is HTTPS the cookie is `__Secure-repomemo_refresh`, which a browser refuses unless it was set with `Secure` from a secure page, so a plain-HTTP response cannot plant or overwrite it. The old name is read once (a browser holding it stays signed in), replaced by the new one, and cleared; signing out and a refused renewal clear both. Over plain HTTP (development) the name is unchanged.

### Changed
- **Tests (written, not run):** a token rotated a moment ago is `JustRotated` while the new token keeps working, then the same token with an old rotation time ends every session, a signed-out token is reuse at once, an unknown token is just refused (storage); the replay in the refresh and cookie tests answers `409` and the cookie is not cleared; over HTTPS the new name is set, the old one read once, replaced and cleared, and signing out clears both (server); the cookie names and their order (cookie unit tests); renewing takes the lock, asks again after a `409` and gives up after two retries (web).
- **Checks run on this release:** `cargo check` of the storage crate and the server with their tests compiles, `rustfmt` is clean for what changed apart from hunks that were already there and `clippy` reports nothing new; `npm run typecheck`, `migrations:check`, `docs:check` and `version:check` pass. No server was started and two tabs were not tried.

### Upgrading
- The migration runs at start and nobody is signed out. Over HTTPS each browser gets the new cookie name on its next renewal. A script that calls `POST /v1/auth/refresh` should treat a `409` as "try again", not as the end of the session.

## V0.2.143 (2026-10-21)

Creating an organization and linking a folder of the server's disk are no longer open to every account and every workspace administrator.

### Security
- **Who creates organizations (E3, decision 22):** every verified account could create an organization and become its owner. The System setting *Who can create organizations* (`organization_creators`) decides: **system administrators only** by default, or every verified account. An account that is turned away gets a message naming the setting, and `GET /v1/session` now says `can_create_organizations`, so the web client does not offer "New organization" or "Create organization" to someone who cannot (the empty dashboard says to ask a system administrator).
- **Who links a local repository (E4, decision 22):** any workspace administrator could link any folder under `REPOMEMO_REPO_ROOTS`, which can hold other teams' repositories. The System setting *Who can link a local repository* (`local_repository_linkers`) decides: **system administrators only** by default, or workspace administrators. It is checked when a link is checked and when it is made, for a folder only; remote repositories (an `https` or `ssh` address) are not affected. "Administrators" here are system and app administrators.

### Changed
- **The setup wizard asks both questions** in its *Access and security* step, next to open registration. Both settings live under System › Settings › Accounts and apply at once.
- **Existing data is untouched:** organizations, workspaces and links made before this release stay. Only creating and linking from now on follows the settings.
- **Tests (written, not run):** with the server's defaults an ordinary account is refused with a message and the session says so, the administrator creates an organization, a workspace administrator who is not a system administrator is refused when checking and when making a local link while a remote address is not touched, opening one setting does not open the other, and a value outside the choices is refused. The tests of everything else start with both settings open.
- **Checks run on this release:** `cargo check` of the server with its tests compiles, `rustfmt` is clean for the files changed (apart from hunks that were already there) and `clippy` reports nothing new; `npm run typecheck`, `types:check`, `docs:check` and `version:check` pass. No server was started.

### Upgrading
- **Ordinary accounts can no longer create organizations, and workspace administrators can no longer link a local folder.** If that is what your server relied on, open the matching setting after upgrading (System › Settings › Accounts). Nothing already created changes.

## V0.2.142 (2026-10-21)

Changing a person's organization role no longer undoes what a workspace decided for itself.

### Security
- **Organization roles respect a workspace's own removals and downgrades (E9):** changing someone's role in an organization wrote that role into every workspace of it. A person a workspace owner had removed came back, and a viewer became a member or an administrator. A workspace role now remembers where it came from (`origin`: `organization` or `direct`), and an organization role change updates only rows that came from the organization. A role a workspace set (adding someone, changing their role) is `direct` and stays; a person a workspace administrator removes from the workspace while they belong to the organization is remembered, and the next organization role change does not add them back. Adding the person to the workspace again on purpose clears the memory; leaving the organization and rejoining starts afresh. The workspace owner's role is still never lowered.
- **Existing roles:** migration `0038_membership_origin.sql` adds the column, with older rows `direct` unless they match the organization role exactly (then they were most likely made by it), and the table of remembered removals.

### Changed
- **New workspaces** take their members from the organization as before; those rows are `organization`.
- **Anonymizing an account** also forgets its remembered removals.
- **Tests (written, not run):** one end-to-end test: three people join an organization; a workspace removes one and makes another a viewer; the organization raises all three to administrator and only the third follows; it follows the role back down; a person added on purpose keeps the workspace's role; leaving and rejoining the organization brings the workspace back.
- **Checks run on this release:** `cargo check` of the storage crate and the server with their tests compiles, `rustfmt` is clean for what changed apart from hunks that were already there, and `clippy` reports nothing new; `migrations:check`, `docs:check` and `version:check` pass. The upgrade test (a 0.2.0 environment opening in place) was not run: CI runs it.

### Upgrading
- The migration runs at start. Roles stay as they are. From now on, to make a workspace follow the organization again for someone, remove them from the workspace and change their organization role once more, or leave and rejoin the organization.

## V0.2.141 (2026-10-21)

The live stream no longer hands every reader the activity feed that only owners and administrators may read.

### Security
- **Activity events by role (E6):** `GET /v1/workspaces/{id}/events` forwarded every activity row (who did what, the summary, the actor) to anyone who could read the workspace, while `GET …/activity` asks for an administrator. The stream now matches: owners and administrators (a system administrator acts as one) get the `activity` event as before; every other reader gets `event: changed` with `{"type":"changed","action","subject_type","subject_id"}`. The notice carries no actor, summary or time, and the id only for something a reader can open (a file, memory card, task, folder, repository, connector or saved search); the ids of members, invitations, providers and the workspace itself stay out. A viewer's stream carries no actor, summary or address.
- **A demotion applies to a stream already open:** the role is read when the stream opens and again every 30 seconds with the other access checks, so a person made a viewer stops receiving the feed without reconnecting.

### Changed
- **The web client** turns a `changed` notice into the activity event its pages already listen to (`fromWire` in `liveEvents.ts`), so the file list, tasks, memory cards and repositories still refresh live for a viewer. A change a viewer makes themselves also refreshes their page, because the notice has no actor to compare with.
- **Tests (written, not run):** who sees the feed, what a notice carries (and that it does not carry the address, the summary or the ids of a member), how an event becomes the message for each kind of reader, a viewer's stream against an owner's when a task is created, and the client's reading of a notice.
- **Checks run on this release:** `cargo check` of the server with its tests compiles, `rustfmt` is clean for the files changed and `clippy` reports nothing new; `npm run typecheck`, `docs:check`, `version:check` and `container:check` pass. No server was started.

### Upgrading
- Nothing to do for people using the web app. A script that read the `activity` events as a member or viewer receives `changed` events now; use an owner or administrator account for the feed.

## V0.2.140 (2026-10-21)

A personal access token with read and write access can no longer grant access, delete, repoint, link or export: those routes need a session, or a token that was given the new `admin` scope on purpose.

### Security
- **A read-and-write token does content work, not access (E5, decision 23):** inviting people (workspace and organization), changing member roles, adding or removing members, deleting or renaming a workspace or an organization, saving AI providers and the AI policy, linking, changing or removing repositories and connectors (their tokens, deploy keys and webhooks included), and exporting or importing a workspace are taken by a new extractor, `ManagingSubject`. It lets through a signed-in session, or a token with the `admin` scope, and refuses every other token with `403` on every method, a `GET` included (an export hands over the whole workspace). The answer names the scope: "Sign in in the browser, or use a personal access token with the admin scope." 29 handlers moved to it. Read-only, read-and-write and `system` tokens are refused there; content work (files, notes, tasks, comments, memory, searches) and the System area stay as they were.
- **Revoking a token ends its streams (E15):** a live event stream opened with a personal access token re-checks that the token still exists and has not expired, as it already did for the session, the membership and the role, so a revoked token stops watching within the recheck interval (30 seconds).
- **The comment was wrong, the code was right to be narrower:** the doc comment of `AuthenticatedSubject` said invitations needed a session while the handlers took any token; it now names the three extractors and what each is for.

### Added
- **The `admin` scope** (migration `0037_admin_token_scope.sql`: the token table is rebuilt with the new value, as in 0024; every existing token keeps its scope). Profile › API tokens offers "Admin: read, write and manage access, links and exports" to everyone, with a warning under the form and a badge in the list. A token is still shown once, expires in 7 to 365 days and cannot manage tokens or the password.
- **The API reference** marks the 29 routes as *managing* (generated from the handler signatures by `scripts/list-routes.mjs`, which learned the new extractor).

### Changed
- **Tests (written, not run):** every one of the 29 routes is refused to a read, a read-and-write and a `system` token with the message naming the scope, and answers an `admin` token as it answers the session (an admin token also deletes a workspace); a read-and-write token still creates a note; the storage test covers the new scope, the token id and a revoked token no longer being active; a stream opened with a token closes when the token is revoked.
- **Checks run on this release:** `cargo check` of the storage crate and the server with their tests compiles; `rustfmt` is clean for the files changed (apart from hunks that were already there) and `clippy` reports nothing new; `npm run typecheck`, `docs:check`, `migrations:check`, `version:check` and `container:check` pass. No server was started.

### Upgrading
- **Scripts with a read-and-write token that invite, change roles, delete, change AI settings, link repositories or connectors, set webhooks, export or import now get `403`.** Create a token with the `admin` scope in Profile › API tokens for those, keep it short-lived, or sign in. Nothing else changes; existing tokens keep their scope, and the migration runs at start.

## V0.2.139 (2026-10-21)

A workspace administrator can no longer aim the server at other services of the machine through an AI provider address.

### Security
- **A provider address is a host and a port (E2, high):** the built-in list of provider hosts now names ports: `127.0.0.1:11434`, `localhost:11434`, `[::1]:11434` (Ollama) and `openrouter.ai:443`. Before, `127.0.0.1` and `localhost` allowed every port, so the server could be pointed at its own port, an admin console or any other local service.
- **No path beyond the provider's own:** with the built-in list, the base URL may carry nothing after the host and port except OpenRouter's `/api/v1` (Ollama: nothing), and no query or fragment. `http://localhost:11434/api/pull`, `http://127.0.0.1:11434/api/tags` and `https://openrouter.ai/other` are refused; so are `http://127.0.0.1:3020`, `http://127.0.0.1:22` and `http://127.0.0.1` (port 80).
- **One answer for an address nothing answers at:** the connection test said different things for a refused connection and other failures. A refused connection, a reset, a timeout and a name that resolves nowhere allowed now give the same message, `The provider could not be reached at this address.` (the cause stays in the server log), so the test cannot be used to find open ports.

### Changed
- **`REPOMEMO_AI_ALLOWED_HOSTS`:** an entry may end in `:port` (`llm.internal:8080`, `[::1]:11434`, `.example.com:443`); one without a port still allows any port, and a list you write yourself leaves paths to you, so gateways at `/v1` keep working. The entries are checked when the server starts, and a typo (`host:99999`) stops it with the name of the setting. `*` still allows every host.
- **Server check and documentation:** the setup wizard says "This machine's Ollama (port 11434) and OpenRouter"; the settings table, the technical mindmap and `UPGRADING.md` describe the ports and paths.
- **Tests (written, not run):** every refused form above under the built-in list, the allowed forms (including `localhost:11434/` and `openrouter.ai` with no path), how entries with ports, suffixes and bare IPv6 addresses are read, and an operator's list with and without ports.
- **Checks run on this release:** `cargo check` of the AI crate and the server with their tests compiles, `rustfmt` is clean for the AI crate and `clippy` reports nothing in the files changed; `docs:check`, `version:check` and `container:check` pass. No server was started and no provider was contacted.

### Upgrading
- A provider saved on this machine with another port (for example a second Ollama on 11435), or with a path such as `http://localhost:11434/api`, is refused when it is next saved or used. Remove the path, or list the host in `REPOMEMO_AI_ALLOWED_HOSTS` (`localhost` for any port, or `localhost:11435`).

## V0.2.138 (2026-10-21)

LibreOffice is started with settings the server controls, and a workspace is no longer told a preview is ready because another workspace holds the same file.

### Security
- **A locked profile (E1b):** before every conversion the server writes LibreOffice's profile again, over whatever the previous document left: macro execution disabled at the highest security level, active content (OLE and DDE) and OLE automation off, links from untrusted documents blocked, links never updated when a Writer or Calc document loads, the Python runtime off, and the update check off. Java is switched off by a settings file the server writes and points LibreOffice to (`-env:UNO_JAVA_JFW_USER_DATA`). The server's secrets stay out of its environment and `--norestore` stays, as before. The settings follow LibreOffice's own configuration schema; they were written from it and have not been tried against a LibreOffice here.
- **Preview state per workspace (E19):** whether a preview is `ready`, `converting` or `failed` is now remembered per workspace (`previews/workspaces/<workspace>/<hash>.ready` or `.failed`), and the conversions in flight are counted per workspace and file. A workspace goes through the same states for a file whoever else holds it, and its PDF is not served before its own request has run. The converted PDF itself is still stored once per content hash and shared. What remains: when another workspace already converted the file, the first conversion is fast, so a careful observer could still guess from the delay; closing that would mean converting the file again in every workspace.

### Changed
- **Clean-up:** forgetting a file whose blob is gone also removes its per-workspace markers; a restart still clears remembered failures (in every workspace, and the ones an earlier release kept next to the PDFs). Markers of a deleted workspace stay until the file is forgotten; they are empty files.
- **Tests (written, not run):** the profile is rewritten over a hostile one and holds each setting (and the Java file); a preview another workspace made is not ready or served here until this workspace has asked; workspace ids are accepted as folder names only when plain; forgetting clears every workspace.
- **Checks run on this release:** `cargo check -p repomemo-server --tests` compiles; `rustfmt` is clean for the file changed, and `clippy` reports nothing in the files changed; `docs:check`, `version:check` and `container:check` pass. No server was started and no file was converted.

### Upgrading
- Nothing to do. Previews already converted show their first state again once per workspace (a quick `converting` before `ready`), because the markers start empty.

## V0.2.137 (2026-10-21)

LibreOffice no longer converts uploaded files just because it is installed: previews are opt-in, and only a release that carries the security fixes is used.

### Security
- **Office previews are off until the operator turns them on (E1a, decision 21):** the new setting `REPOMEMO_OFFICE_PREVIEWS` (default `false`) must be `true` before any Word, Excel or PowerPoint file is converted. While it is off, LibreOffice is not even started, wherever it is installed, and files show extracted text and tables. Before, finding `soffice` on the machine was enough, so a strangers' upload was parsed by a program with a long history of memory-safety bugs on the server's own host.
- **A minimum LibreOffice release:** when previews are on, the server asks LibreOffice for its release once at start (`soffice --version`) and converts only with 26.2.5 or later on the 26.2 line, or 26.8.0 or later (the releases that fixed the advisories of 2026-10-05). An older release, or one whose version cannot be read, is refused: nothing is converted, files show extracted text, and the reason names the release. Nothing is converted before that check has answered.
- **Where it shows:** System › Overview (`LibreOffice`: `installed, off (REPOMEMO_OFFICE_PREVIEWS)`, `refused: LibreOffice 24.8.7.2 … is older than the fixed releases …`, or the version and the sandbox), the setup wizard's server check (a warning when previews are on and the release is refused or LibreOffice is missing), the start-up log, and `GET /v1/system/overview` (`background.office_previews` gains `installed`, `requested` and `refused`).

### Changed
- **`REPOMEMO_SOFFICE` alone no longer turns previews on:** it only says where `soffice` is. The container's compose file passes `REPOMEMO_OFFICE_PREVIEWS` (and `.env.example` explains it) next to `REPOMEMO_WITH_LIBREOFFICE`, which adds the program to the image.
- **Tests (written, not run):** the release table (fixed, older, unreadable), a converter that is off never starts LibreOffice, a turned-on one waits for its check and refuses an old release naming it, and the existing conversion test now sets the setting. The test stand-in for LibreOffice answers `--version`.
- **Checks run on this release:** `cargo check -p repomemo-server --tests` compiles; `npm run typecheck`, `container:check`, `docs:check` and `version:check` pass. No server was started and no file was converted. `rustfmt` and `clippy` already report findings elsewhere in the tree on the current Rust; none are new in the files changed here.

### Upgrading
- **Previews stop until you turn them on.** A server with LibreOffice installed that showed layout-accurate Word, Excel and PowerPoint files falls back to extracted text and tables after this release. To keep them: set `REPOMEMO_OFFICE_PREVIEWS=true`, use LibreOffice 26.2.5 or later (26.8.0 or later on the next line), ideally behind `REPOMEMO_SOFFICE_WRAPPER`, and restart. In the containers, set `REPOMEMO_OFFICE_PREVIEWS=true` next to `REPOMEMO_WITH_LIBREOFFICE=true`; Debian 13's own LibreOffice is older than the fixed releases, so previews stay refused there until V0.2.148 puts LibreOffice in a container of its own.

## V0.2.136 (2026-10-21)

Remote repositories can be cloned inside the official container: its git is now one the server accepts.

### Fixed
- **The container's git was refused (S5-7):** the server image was built on Debian 12, which ships git 2.39.5 with the fix for CVE-2025-48384 backported. The server checks the version number, and 2.39.5 is below every release it accepts, so it refused every remote clone in the official image. The `server` stage is now `debian:trixie-slim` (Debian 13), whose git is 2.47.3, a release `is_patched_git` accepts. The build stage follows (`rust:1-trixie`), so the binary is built and run on the same libraries. Both are pinned by digest. The web stage (`node:22-bookworm-slim`) is unchanged: it ships nothing.

### Changed
- **`npm run container:check` asserts the pair:** it reads the release table of `is_patched_git` in `crates/git/src/remote.rs`, looks up the git that the server stage's Debian release carries (bookworm 2.39.5, trixie 2.47.3) and fails if the server would refuse it. It prints the result (`git 2.47.3 on Debian trixie`). With the base put back to bookworm it reported the problem and exited with 1; on the tree it passes. A new Debian release has to be added to the check's table before it can be used.
- **Documentation:** the production guide and the commands page say which git the image carries and what the check asserts.
- **Checks run on this release:** `container:check`, `version:check`, `docs:check` and `migrations:check`. No image was built and no repository was cloned; that is the maintainer's try (below). LibreOffice (`--build-arg WITH_LIBREOFFICE=true`) now comes from Debian 13's packages; V0.2.137 sets a minimum version for it and V0.2.148 moves it into a container of its own.

### Upgrading
- Rebuild the images (`docker compose up -d --build`); the volume and the database are untouched.
- To try it: add a remote repository to a workspace in the container and sync it. Before this release the sync was refused with a message about the git version.

## V0.2.135 (2026-10-21)

Nothing that builds RepoMemo can move under it any more: CI actions and container base images are pinned, and Dependabot proposes the next versions.

### Security
- **CI actions pinned to a commit (E17):** `actions/checkout`, `actions/setup-node`, `Swatinem/rust-cache`, `EmbarkStudios/cargo-deny-action` and `dtolnay/rust-toolchain` name a full commit, with the release it is in a comment (v4.4.0, v4.4.0, v2.9.3, v2.1.1, `stable`). A tag can be moved to other code after the fact; a commit cannot. `dtolnay/rust-toolchain` took its toolchain from the name it was called by, so the jobs now say `toolchain: stable` themselves.
- **Container base images pinned by digest:** `node:22-bookworm-slim`, `rust:1-bookworm`, `debian:bookworm-slim` and `caddy:2-alpine` name their `sha256` digest in the `Dockerfile`. The `NODE_VERSION`, `RUST_VERSION` and `DEBIAN_RELEASE` build arguments are gone (nothing used them): the tag and the digest are written on the `FROM` line, where Dependabot reads them.

### Added
- **`.github/dependabot.yml`:** a weekly check of `cargo`, `npm`, `github-actions` and `docker`; the minor and patch updates of Cargo and npm arrive as one pull request each. Security updates open their own pull requests whenever an advisory appears.

### Changed
- **`npm run container:check`** also refuses a `FROM` line without a digest, a `uses:` in `.github/workflows` that names a tag or branch, and a Dependabot file that leaves out one of the four ecosystems. It was tried on a planted example: with the digest of the Caddy image removed and `actions/checkout@v4` put back it reported both and exited with 1; on the tree it passes.
- **Checks run on this release:** `container:check`, `version:check`, `docs:check`, `migrations:check`. No image was built and no workflow was run: the digests and commits were read from the registry and from GitHub, and CI is the first run of the pinned workflow. The images are the same as the tags pointed to when they were read.

### Upgrading
- Nothing to do. Merge or close Dependabot's pull requests as they come: each one is a release of its own.

## V0.2.134 (2026-10-21)

The web build and test tools move to current majors, which closes the last advisories of the development packages.

### Security
- **Web toolchain, second part (S5-3b):** `vite` 5.4.21 to 8.3.5, `@vitejs/plugin-react` 4.7.0 to 6.1.2 and `vitest` 3.2.7 to 4.1.12. The Vite 5 line gets no fixes; the advisories were a development-server file-access bypass (`server.fs.deny` with Windows alternate paths), a `.map` path traversal, and prototype-pollution and file-read issues in Vitest and `tinypool`. `npm audit` now finds **nothing**, with or without development packages. The built web app never contained these tools.
- **Fewer packages:** Vite 8 builds with Rolldown, so `esbuild` and `rollup` leave the lock; `package-lock.json` lists 66 packages fewer (478 to 412).

### Changed
- **Node 22.12 or later:** Vite 8 needs it (`engines` in the root `package.json`, the README and the Windows setup page say so). The container image and CI already use the current Node 22.
- **Configuration:** `vite.config.ts` keeps the content security policy plugin, the `VITE_` and `TAURI_ENV_` prefixes, the development server on `127.0.0.1:3021` and the `jsdom` tests unchanged. One setting is added, `test.mockReset: true`: Vitest 4's `restoreMocks` restores only `vi.spyOn` spies, so the `vi.fn()` mocks of the tests' `vi.mock` factories would have kept their calls and answers from one test to the next; the reset keeps each test as isolated as before.
- **Checks run on this release:** `npm run typecheck` passes against the new versions (run in a scratch copy, so the working `node_modules` was not touched); the new lockfile passes `npm ci --dry-run` and `npm audit` (0, with and without `--omit=dev`); Vite loads `vite.config.ts` with both plugins and the test settings. **Not run:** `npm run build` (the `content-security-policy.txt` file and the meta tag are the maintainer's to confirm), `npm run web:dev` and the Vitest suite (CI runs it).

### Upgrading
- Run `npm ci` after pulling. Node 22.12 or later is required.

## V0.2.133 (2026-10-21)

Five transitive packages of the web toolchain are updated to patch releases that close their advisories.

### Security
- **Web toolchain, first part (S5-3a):** `postcss` 8.5.15 to 8.5.31, `nanoid` 3.3.12 to 3.3.20, `browserslist` 4.28.2 to 4.29.4, `source-map-js` 1.2.1 to 1.2.2 and `baseline-browser-mapping` 2.10.33 to 2.11.30 (source-map file disclosure, loops and memory growth). They are development packages: the built web app never contained them, and `npm audit --omit=dev` was already clean.
- **What is left:** `npm audit` now lists five entries, all in the test and build tools that need a new major: `vite`, `esbuild`, `vitest`, `@vitest/mocker` and `tinypool`. V0.2.134 moves Vite and Vitest.

### Changed
- **Only `package-lock.json` moves,** inside the ranges `package.json` already allowed. `caniuse-lite`, `electron-to-chromium` and `node-releases` (data used by `browserslist`) moved with it; no source file changed.
- **Checks run on this release:** `npm audit` (5 entries, none in production), `npm audit --omit=dev` (0), `version:check` and `docs:check`. The lockfile was updated without reinstalling, so the typecheck and the web tests were not run; CI installs from the new lockfile and runs both.

### Upgrading
- Nothing to do. Run `npm ci` to install the new versions.

## V0.2.132 (2026-10-21)

The database layer no longer links a TLS stack it never used.

### Security
- **A smaller surface (S5-10):** `sqlx` is built with the `runtime-tokio` feature instead of `runtime-tokio-rustls`. SQLite is a file and needs no TLS, yet the feature pulled `rustls` and a second `webpki-roots` (0.26) into the server through the storage crate. `webpki-roots` 0.26.11 is gone from the lock; `rustls` and `webpki-roots` 1.0 are now reached only by `reqwest` (AI providers, connectors, the desktop client) and by the mail crate.

### Changed
- **Checks run on this release:** `cargo tree -p sqlx` lists no TLS crate; `cargo check -p repomemo-storage --tests` compiles (the tests were not run; CI runs them); `cargo deny --workspace check` passes; version, container and type checks pass. Nothing in the database code changed.

### Upgrading
- Nothing to do.

## V0.2.131 (2026-10-21)

Spreadsheet reading moves to `calamine` 0.37, which brings `zip` 9 and closes the last advisories that fail CI's dependency check.

### Security
- **`zip` 8.6.0 to 9.0.5 (S5-2):** the three extraction advisories (RUSTSEC-2026-0367, -0368 and -0369: SUID bits, symbolic links and permissions applied by `extract()`) are gone from the lock. RepoMemo never called `extract()`, so they were not reachable, but they failed the blocking `cargo deny` job. `calamine` 0.37 is the release that accepts `zip` 9; it is the only user of `zip`.

### Changed
- **Dependencies:** `calamine` 0.36 to 0.37 and `quick-xml` 0.41 to 0.42 (Tauri already used 0.42, so the lock now holds a single `quick-xml`). `nix` 0.31 (Unix only) is added next to the older ones, through the new `zip`.
- **XML reading:** `quick-xml` 0.42 reads names, attributes and text as UTF-8 strings. `crates/ingestion` (`documents.rs` for workbooks, presentations and shared strings, `lib.rs` for Word) follows: names are compared as text, and text is read with its line endings normalized the way XML 1.0 says (`
` becomes `
`). The readings and their results are otherwise the same; a part that is not UTF-8 (rare, and not what Office writes) is now reported as an invalid file instead of being decoded.
- **Rust version:** the workspace's `rust-version` is 1.88, which `calamine` 0.37 needs. The README and the Windows setup page say "1.88 or later"; `rust-toolchain.toml` already follows `stable`.
- **`deny.toml`:** the comment on duplicate versions no longer names `quick-xml`. No advisory is ignored that was not before.

### Checks run on this release
- `cargo deny --workspace check` passes: advisories, bans, licenses and sources are all ok, with no new ignore.
- `cargo check -p repomemo-ingestion --tests` compiles. The tests were not run (CI runs them), and no spreadsheet, Word or PowerPoint file was opened by hand.

### Upgrading
- Building from source needs Rust 1.88 or later (`rustup update`).

## V0.2.130 (2026-10-21)

The TLS libraries behind every outbound connection are updated, and CI's blocking dependency check no longer fails on them. This is the first release of track L (security and dependencies), which lifts the feature freeze for security fixes, dependency updates and backend hardening until V0.2.162.

### Security
- **TLS libraries updated (S5-1):** `rustls` 0.23.45 to 0.23.46, `rustls-webpki` 0.103.15 to 0.103.16 and `webpki-roots` 1.0.9 to 1.0.10. They carry six advisories (RUSTSEC-2026-0346, -0347, -0348, -0349, -0381 and -0382), among them a certificate-decompression allocation of up to 1 GiB before authentication, a key-exchange downgrade after an unauthenticated ServerHello, and distrusted intermediates accepted. They are used by every outbound call: AI providers, mail, connectors and the desktop client.
- **A lockfile-only change:** all three are patch releases inside the current requirements, so `Cargo.lock` is the only dependency file that moves; no source file changed.

### Changed
- **Checks run on this release:** `cargo deny --workspace check advisories` now reports only the three `zip` advisories (RUSTSEC-2026-0367, -0368, -0369), which V0.2.131 closes; the TLS advisories are gone. `version:check`, `container:check`, `migrations:check` and `types:check` pass. `cargo check` was not run: the Rust sources did not change.
- **Plan and review:** the [fifth security and backend review](mindmap/security/2026-10-21-security-review.md) and the plan for tracks L and M (V0.2.130 to V0.2.162, with decisions 20 to 23) are added to the roadmap and the release index. Decisions 20 to 23 are taken as proposed.

### Upgrading
- Nothing to do. Rebuild the server and the desktop app to pick up the new libraries.

## V0.2.129 (2026-10-21)

The release checks are repeated after the frontend polish track (V0.2.57 to V0.2.129), the notes for V0.3.0 include it, and the feature freeze is back on until the tag.

### Changed
- **Checks run on this release:** `npm run typecheck`, `styles:check`, `a11y:check`, `types:check`, `version:check`, `docs:check`, `container:check` and `migrations:check` all pass. The Vitest tests written since V0.2.57 (building blocks, pages, behaviour) were written and not run, as the project's rules ask; CI runs them. `cargo` was not run: the Rust crates changed only by their version number in this track.
- **Release notes for V0.3.0:** [clean-install.md](mindmap/technical/clean-install.md) now lists V0.2.1 to V0.2.129 as the source of the `V0.3.0` changelog entry and carries a draft section of notes for the polish track (what a person sees, what a developer gets).
- **Before the tag:** the runbook's pre-tag list has a new line for a browser pass over the polished pages in both themes (dialogs, forms, lists, focus) and the web steps of the run (4, 6 to 8 and 11) made once on them. That pass, and the clean-machine install, are the maintainer's.
- **Freeze restored:** from this release until `V0.3.0` is tagged, only the release candidate's checks and the fixes the clean install finds go in; track K's exception (decision 15) ends here.
- **Track K:** 73 releases (V0.2.57 to V0.2.129) in six parts: foundations (K1), building blocks (K2), React foundations (K3), pages on the shared pieces (K4), behaviour (K5) and the checks that keep it so (K6). The plan's status line says it is built.

### Upgrading
- Nothing to do.

## V0.2.128 (2026-10-21)

DESIGN.md describes the interface as it is built: accent, sizes, layers, buttons and building blocks.

### Changed
- **Colour and type:** the front matter and the text now name the shipped values: the blue accent `hsl(233 84% 44%)` (#1228ce; the green accent an older audit read was a dead token block), neutral greys at saturation 0 (canvas #f2f2f2, ink #262626, divider #e6e6e6), Plus Jakarta Sans for text and Fjalla One for page titles.
- **Controls:** 36 px tall (40 px on touch screens), a 2 px border, 8 px corners, from the `--control-*` tokens; the dropdown's width, list height and layer are described.
- **Buttons:** three treatments and one shape, `main`, `secondary`, `danger` and `IconButton`, replacing "exactly two variants"; a `Button` is `type="button"` unless it says `type="submit"`.
- **Layers and icons:** a table of the layer scale (`--layer-raised` to `--layer-toast`) and the three icon sizes (14, 16 and 18 px).
- **Building blocks:** `Dialog` (sizes, first field, Enter, Escape, Cancel first, focus return), `ActionMenu` and `Pagination` with `usePagedList` are added to the list that already had `Field`, `Checkbox`, `Switch`, `IconButton`, `Tooltip`, `ConfirmDialog`, `LoadingState`, `EmptyState`, `Badge`, `SegmentedControl`, `PanelHeading`, `PageHeader`, `Table`, `Combobox`, `SearchField`, `CopyButton`, `Time` and the option list.
- **Rules written down:** the field-message rule (a refused field marks itself, API errors stay toasts), sending with Enter and Ctrl+Enter, unsaved changes, focus after a dialog and a deletion, and that every change confirms itself with a toast or a visible change. The do's and don'ts point to the tokens and to `npm run styles:check`.
- **Technical mindmap §9:** the UI kit row names DESIGN.md as the place that lists each block and rule.

### Upgrading
- Nothing to do.

## V0.2.127 (2026-10-21)

The style check refuses bare element styles, raw colours, raw layers and raw control heights, so the stylesheets stop drifting.

### Changed
- **`npm run styles:check` fails on:** a bare `input`, `select`, `textarea` or `button` selector (alone, or at the end of a path such as `.page-form input`) outside the base reset (the top of `01-tokens-and-shared-web.css`) and the retired local edition; a hex, rgb or hsl colour outside a `:root` or `[data-theme]` token block; a `z-index` that is not a `var(--layer-*)` token (0, -1 and auto stay allowed); and a pixel `height` or `min-height` on a button, input, select or dropdown trigger.
- **What the tree already did:** 111 older cases (62 bare element selectors in page rules, 49 control heights) are listed in `scripts/styles-baseline.json`. The check passes on the tree, fails for anything not on that list, and also fails for a listed case that is gone, so the list can only shrink; `npm run styles:check -- --update-baseline` rewrites it after a cleanup. No raw colour or raw layer was left to list.
- **Raw colours:** the three scrollbar colours of `11-blueprint-primitives.css` became the tokens `--scrollbar`, `--scrollbar-thumb` and `--scrollbar-thumb-hover` (same values).
- **Proof that it catches drift:** every run first checks a planted stylesheet in memory (a page rule on `input`, a hex colour, `z-index: 5`, a 30 px `.rm-button`), which must trip all four rules, and a clean twin, which must trip none. Planted into a folder of its own, the check exited with 5 problems; the clean twin exited 0.
- **Docs:** `docs/DEVELOPMENT_COMMANDS.md` lists `npm run styles:check`.

### Upgrading
- Nothing to do.

## V0.2.126 (2026-10-21)

Tests for every shared building block. They are written, not run: CI runs them.

### Changed
- **`test/buildingBlocks.test.tsx`:** `Field` (label, hint and message tied to the control by id and `aria-describedby`; no message while valid), `Checkbox` and `Switch` (toggle from their label, nothing while disabled), `Dropdown` (its list opens in a portal outside the form, chosen with the keyboard), `ConfirmDialog` and `useConfirm` (true on the decisive action, false on Cancel, an error outside its provider), `LoadingState` (a status line), `EmptyState` (title, sentence, one action), `Badge` (tone as data), `SegmentedControl` (chooses, never turns the chosen option off), `Combobox` (filters by label and description, Enter chooses, empty text), `SearchField` (Escape and the clear button, Escape passes when empty), `CopyButton` (Copied for two seconds, a toast and `onUnavailable` when the clipboard refuses) and `useFormState` (a new saved value starts the form over).
- **Already covered by earlier releases:** `Field` marking a refused control (`fieldValidation`), `Dialog` (`dialogBehaviour`), `Pagination` and `usePagedList` (`pagedLists`), `useUnsavedChanges` (`unsavedChanges`), the send shortcut (`submitShortcut`), focus return (`focusReturn`), `IconButton` (`assistantPieces`) and `useFormState` itself (`useFormState`).
- **Setup:** the file gives jsdom the few browser functions Radix asks for (pointer capture, `scrollIntoView`, `ResizeObserver`).

### Upgrading
- Nothing to do.

## V0.2.125 (2026-10-21)

Focus returns to the opener after a dialog and moves to the next item after a deletion.

### Changed
- **After a dialog:** every `Dialog` notes the control that had focus when it opened (a menu or another dialog is never the opener: the control that opened them is) and gives focus back to it when it closes, by Cancel, Escape, a click outside or the decisive action. The action menus of rows and cards count: closing a dialog opened from *Actions for…* lands on that button.
- **After a deletion:** when the control that had focus is removed from the page (a deleted comment, task, member, token, saved search, file or backup row, a removed chip), focus moves to the item that took its place, else the one before it, else the heading of the list, instead of falling to the top of the page. A dialog whose opener is gone does the same.
- **Unchanged:** a move to another page still puts focus at the start of the page (V0.2.47).

### Added
- **`lib/focusReturn.ts`:** `currentOpener`, `anchorFor`, `restoreFocus` and `watchFocusLoss` (started once in `SharedWebApp`).

### Upgrading
- Nothing to do.

## V0.2.124 (2026-10-21)

Every change confirms itself with a success toast or a visible change on the page, and every failure with an error toast.

### Changed
- **Success toasts added:** evidence renamed, deleted, lifecycle saved, comment added, updated and deleted (artifact page); memory card confirmed, saved and deleted; organization renamed, member saved, member removed; workspace renamed and deleted; workspace member saved and removed; note stored; search saved and saved search deleted; task created and deleted, checklist item added; organization and workspace created (dashboard); AI provider saved; chat renamed; all notifications marked read; notification preference saved; backup and backup copy started.
- **Already confirmed before:** the profile (name, picture, address, password, verification link), API tokens, system settings, user administration, maintenance, restore and delete backup, uploads, folders, moves, bulk actions, connectors, repositories (the sync announces when it finishes), the workspace export and import, the AI policy (a notice) and the sign-in flows.
- **Confirmed by what the page shows, so no toast:** a task moved to another column or reassigned, a checklist item ticked, a message sent to the assistant (the answer appears), a sync or stop (the progress shows), a token created (the secret dialog opens), a console command (its output), transferring ownership and detaching the environment (the page reloads).
- **Failures:** every mutation of the web client already reports a failure as an error toast (through `showToast`, `useAsyncAction` or the page's error toast); none fails silently. The remaining empty handlers are reads and conveniences (remembering a width, refreshing after a live event).
- **Not added on purpose:** a toast for the start of a repository sync, since the sync announces itself when it finishes.

### Upgrading
- Nothing to do.

## V0.2.123 (2026-10-21)

Long lists show 24 rows a page with the shared pagination.

### Changed
- **Lists that page:** notifications, the tasks of each board column (each column has its own pager), memory cards (and the results of a memory search), System › Users, System › Jobs, System › Audit, and a repository's file list. Evidence and search results already did.
- **How:** each list shows 24 rows and the shared `Pagination` under it ("1–24 of 60", *Previous page*, *Next page*), which stays hidden while everything fits on one page. A list starts again at the first page when its filter changes (users, jobs by status, files, memory search); a page that no longer exists after a change moves to the last one.
- **Repository files:** the list no longer stops at the first 300 files with a "Filter to narrow the list" line; every matching file is reachable, 24 at a time, with the count under the pager.

### Added
- **`usePagedList(items, resetKey)`** (`lib/usePagedList.ts`): the rows of the current page and the props of `<Pagination>`.

### Upgrading
- Nothing to do.

## V0.2.122 (2026-10-21)

Required and malformed fields are marked before anything is sent; server errors stay toasts.

### Changed
- **`Field` marks its own control:** when a form is submitted and the browser refuses a control inside a `Field` (empty `required`, a wrong `type="email"` or `type="url"`, `minLength`, a number outside its range), the field sets `aria-invalid`, shows a one-line message under the control, replaces the browser's bubble, and focuses the first refused control. The message clears as soon as the value is valid. Nothing is sent while a field is refused.
- **Messages:** "Title is required." (the field's own label), "Enter an email address such as name@example.com.", "Enter a web address that starts with http:// or https://.", "Enter a value from 1 to 65535.", "Use at least 12 characters.", "Enter a number." and the like, worded in `lib/fieldMessage.ts`. A page can still pass `invalid` and `message` for a rule only it knows; that wins.
- **Covered without a change to each form:** every form built with `Field` already uses the browser's own checks (`required`, `type="email"`, `minLength`, `min`/`max`): sign-in and registration, invitations, password reset, the setup wizard, profile, organization and workspace members, tasks, memory, notes, comments and the rest.
- **AI provider address:** the *Base URL* field is `type="url"`, so a malformed address is marked before saving.
- **Unchanged:** a form whose submit button stays disabled until it is ready (many create forms) never reaches the browser's check, so it shows no message; server errors stay toasts.

### Upgrading
- Nothing to do.

## V0.2.121 (2026-10-21)

Ctrl+Enter (Cmd+Enter on macOS) sends comments, notes, questions and memory statements, and a muted hint says so.

### Changed
- **Every `Textarea`:** Ctrl+Enter, or Cmd+Enter on macOS, submits the form the field is in, unless the field's own key handler already used the key (the mention list, for example). A plain Enter still adds a line.
- **The note editor:** Ctrl+Enter (Cmd+Enter) stores the note instead of adding a line break.
- **The assistant:** Enter sends, Shift+Enter adds a line, and Ctrl+Enter (Cmd+Enter) sends as well; a muted line under the composer says so.
- **Hints:** a muted line under the field names the shortcut and what it does: new and edited evidence comments ("Ctrl+Enter sends."), the question to ask the evidence ("asks"), the statement of a memory card in the memory section, on the card page and in *Save as memory* ("saves"), and the note ("stores the note"). The hint reads "Cmd+Enter" on macOS.

### Added
- **`lib/submitShortcut.ts`:** `isSubmitShortcut`, `submitOnShortcut`, `submitShortcutLabel` and `submitHint`.

### Upgrading
- Nothing to do.

## V0.2.120 (2026-10-21)

Leaving a form with unsaved changes asks first: System › Settings, the evidence note, a memory card being edited, the workspace name and the "Save as memory" dialog.

### Changed
- **A question before leaving:** while one of these forms holds unsaved changes, moving to another page of the app asks "Discard unsaved changes?" in the app's own dialog (`useConfirm`), with *Cancel* and a danger *Discard changes*; closing or reloading the tab shows the browser's own warning.
- **Where:** System › Settings (the count of unsaved settings), the note title and body being written in the evidence section, a memory card being edited (title, source, statement or evidence changed), the new workspace name in workspace settings, and the title and statement of the *Save as memory* dialog (closing it asks, as does *Cancel edit* on a card).
- **Where it does not ask:** the note draft and the new workspace name are kept while someone moves between sections of the same workspace, so those moves do not ask; an automatic redirect (such as to sign-in) never asks; the browser's back button is not intercepted, because the address has already changed when the app hears of it.
- **Navigation:** `navigate()` in `web/router.ts` asks the registered guards before a move the person made; the `replace` form used for redirects does not.

### Added
- **`useUnsavedChanges(isDirty, { keepsOn, subject })`** (`lib/unsavedChanges.ts`): registers the guard and the tab warning, and returns `canLeave()` for a form's own exits.

### Upgrading
- Nothing to do.

## V0.2.119 (2026-10-21)

Every dialog focuses its first field, submits on Enter, closes on Escape and has one of three widths.

### Changed
- **`Dialog`:** takes `size` (`sm` 400 px for a question, `md` 520 px for a form, `lg` 720 px for long content; `md` when it has a body and `sm` when it has none) instead of the one-off `rm-dialog-wide` class, which is removed. It focuses its first writable field when it opens, and Escape closes it, as before.
- **Submit on Enter:** a dialog with fields takes `onSubmit`; it wraps the body and footer in a form, so Enter in a field runs it and the decisive footer button is `type="submit"` (disabled or busy, it blocks the Enter too). The five hand-written Enter key handlers and `autoFocus` props are gone.
- **Dialogs checked:** all 28 dialogs of the web client follow the rule; Cancel is before the decisive action in each, and the ones with a field are Rename chat, Rename folder, Replace the token, Restore a backup, the System user confirmation (anonymize) and Save as memory.
- **Widths:** the command palette, the commit text, the evidence list of the assistant, the version text and comparison, the secret link, the repository edit form, the move-to-server dialog and Save as memory are `lg`; the forms are `md`; the questions are `sm`.

### Upgrading
- Nothing to do.

## V0.2.118 (2026-10-21)

The assistant and the command palette use the shared field and option list. The pages-on-the-shared-pieces track (K4) is complete: V0.2.102 to V0.2.118.

### Changed
- **Assistant dock and panel:** the chat history, new chat and shrink or close buttons are `IconButton`s (32 px, named in a tooltip on hover and focus) instead of icon buttons with a `title`; renaming a chat uses a `Field`; *Delete chat* is a `danger` button; the send button, rename and delete show their work with `busy`; "Loading chat…" and "Working on it…" are the shared `LoadingState`, so the assistant's own pending style is gone.
- **Command palette:** the field is the shared `SearchField` and the results are the shared `rm-option-list` (both since V0.2.88 and V0.2.90); a query with no match shows the shared `EmptyState`, sized to the palette.
- **CSS removed:** `.shared-assistant-pending`, the 32 px override for the assistant's header buttons, and the checkbox rule of the retired provider toggle (the last user of `.shared-ai-provider-toggle` went in V0.2.117).
- **K4 as a whole:** every form of the pages in the plan has labelled fields of one height, its dropdowns are the shared ones and fill their field, and its loading, empty and busy states are the shared ones. Still on their own markup, outside this track's list: the dashboard's create forms, the knowledge map's graph toggles, the health findings list, the workspace import and the retired local edition's move-to-server dialog.

### Upgrading
- Nothing to do.

## V0.2.117 (2026-10-21)

System toolbars and filters use the shared search field, dropdowns and date fields.

### Changed
- **Filters as fields:** the log source, category, workspace and level, the job status, the usage period, the audit export format and the two export days are `Field`s with a visible label above the control, in place of the inline `label` that wrapped each control. The two days are date inputs that bound each other (`From` stops at `To`, `To` starts at `From`). Toolbars line their controls up on the bottom edge. The user filter (`SearchField`) was already shared.
- **Follow:** the logs' *Follow* toggle is the shared `Switch`.
- **Dialogs:** the email to type before anonymizing an account and the name of the environment a backup is restored into are `Field`s inside their dialog (not inside the description text); destructive confirmations (*Remove role*, *Sign out everywhere*, *Deactivate account*, *Anonymize account*, *Delete backup*) are `danger` buttons with `busy`.
- **States and buttons:** empty users, jobs, log events, backups and audit trail are the shared `EmptyState`; *Back up now*, *Restore* and *Download* use `busy`.
- **CSS removed:** `.rm-system-inline`.

### Upgrading
- Nothing to do.

## V0.2.116 (2026-10-21)

System settings rows use the shared fields and switches.

### Changed
- **One control for every row:** the control of a setting (a `Switch` for on and off, a `Dropdown` for a choice, a number `Input` with its unit for a whole number) is the new `SettingControl` (`web/system/SettingControl.tsx`), used by System › Settings and by the setup wizard's settings steps, which had two copies of it.
- **Number rows:** a whole number outside the range the server accepts is marked `aria-invalid` with a one-line message under the field ("Between 1 and 365 days."), as decision 19 describes, and *Save changes* (or the wizard's *Save and continue*) stays off until every changed number is valid. The row's description is tied to the control with `aria-describedby`.
- **Read-only state:** for an application administrator who may read but not change, every control of the row is disabled as before; the shared control carries that in one place.
- **Buttons:** *Save changes* uses `busy`; *Detach environment* is a `danger` button with `busy`; the redundant `type="button"` is dropped.

### Upgrading
- Nothing to do.

## V0.2.115 (2026-10-21)

Mail settings use the shared fields and states.

### Changed
- **Fields:** the mail server, connection security, port, user name, password, sender address and sender name are `Field`s with visible labels (they carried only an `aria-label` copy before); the connection and port, the user name and password, and the two sender fields sit side by side in a `FieldRow` that stacks on narrow screens; the test message's address is a `Field`.
- **Switch and checkbox:** *Send mail* is the shared `Switch` (saved with the form's button, as in System › Settings) and *Remove the saved password* is the shared `Checkbox`, instead of two bare checkboxes in labels.
- **States and buttons:** "Loading mail settings…" is the shared `LoadingState` and an empty outbox is the shared `EmptyState`; *Save mail settings* and *Send test mail* each show their own work with `busy`.
- **CSS:** the form has its own layout (`.rm-mail-form`, `.rm-mail-test`) instead of borrowing the settings list's, and the dropdown width rule for labels in that list is removed.

### Upgrading
- Nothing to do.

## V0.2.114 (2026-10-21)

Connector forms use the shared pieces.

### Changed
- **Form:** the repository address, the tracker (a dropdown tied to its label by the field's id) and the access token are `Field`s with their explanation as the hint; the new token in the *Replace the token* dialog is a required `Field`. This also gives the connector form back the labels that the repository form's CSS used to style (V0.2.113 removed those rules).
- **States:** the connector list shows the shared `LoadingState` while it is read for the first time.
- **Buttons:** *Check access*, *Connect and sync* and *Replace token* use `busy`; *Remove connector* is a `danger` button; the redundant `type="button"` is dropped.

### Upgrading
- Nothing to do.

## V0.2.113 (2026-10-21)

Repository forms and pages use the shared pieces.

### Changed
- **Link form** (Workspace Settings › Repositories, and the edit dialog): the repository link, deploy key, host key, access token and its account name, name, branch and the include and exclude patterns are `Field`s whose explanation is the field's hint (a muted line under the control, tied to it with `aria-describedby`); the token and its account name, and the name and branch, sit in a `FieldRow` that stacks on narrow screens.
- **Status:** the sync status of a repository (not synced, syncing, up to date, failed) is the shared `Badge` with a tone (`STATUS_TONE`), on the repository card and on the repository page, instead of its own pill.
- **States and buttons:** "Loading repositories…", "Loading the repository…" and "Loading files…" are the shared `LoadingState`; *Check access*, *Link and sync*, *Save* and *Generate summary* show their work with `busy`; *Remove repository* is a `danger` button.
- **CSS removed:** the label rules of `.rm-repo-form`, `.rm-repo-form-row` and the four `.rm-repo-status` rules, and the dropdown width rule of the form.

### Upgrading
- Nothing to do.

## V0.2.112 (2026-10-21)

The artifact page's forms use the shared pieces.

### Changed
- **Fields:** the evidence title, the lifecycle form (status, review owner, replacement evidence in the searchable picker, review note), editing a comment and writing a new one (the mention-aware text area included) are `Field`s with visible labels; the dropdowns and the picker are tied to their labels by the field's id.
- **Lifecycle status:** the status shown in the lifecycle panel is the shared `Badge` (a warning dot for evidence that needs review or is outdated), instead of its own pill style.
- **States:** "Loading chunks…" is the shared `LoadingState`; a file with no text preview, no discussion and an empty chunk list show the shared `EmptyState`.
- **Buttons:** *Save evidence*, *Save lifecycle*, *Save comment*, *Add comment*, *New version* and *Re-index* use `busy`; the redundant `type="button"` is dropped.
- **CSS:** the label rules of the lifecycle form and the pill styles of the status are removed; the lifecycle form and the comment form now place their buttons by class (`.shared-lifecycle-actions`) rather than by "any div", which would have caught the fields.

### Upgrading
- Nothing to do.

## V0.2.111 (2026-10-21)

The memory section and the memory card page use the shared fields, picker and states.

### Changed
- **Memory section:** the search is the shared `SearchField`; a new card's title and statement are `Field`s (required); its evidence links are chosen with the searchable `Combobox` in a `Field` (type part of a file name) instead of a dropdown that held every file behind an "Add" placeholder item.
- **Memory card page:** editing uses `Field`s for the title, source and statement, a `Field` around the evidence picker, the shared `EmptyState` for "No evidence is cited" and for a card without linked evidence, and the shared `LoadingState` while the card loads. *Export Markdown* and *Save memory* use `busy`; *Confirm as it is* shows its work.
- **Save as memory dialog** (an answer kept from Retrieval or the assistant): title and statement are `Field`s and each evidence link is a shared `Checkbox`.
- **CSS removed:** the label rules of `.shared-record-edit-form` and `.shared-memory-form`, the evidence label rule and the dropdown width rule of `.shared-memory-evidence`, and the checkbox label rules of the draft dialog.

### Upgrading
- Nothing to do.

## V0.2.110 (2026-10-21)

Tasks use the shared fields, checkboxes and states.

### Changed
- **Create form:** title, description, priority, assignee, related evidence (the searchable picker) and due date are `Field`s with visible labels instead of unlabelled controls carrying an `aria-label`; the title is marked required.
- **Checklist:** the steps of a task are shared `Checkbox`es (a clickable label, a 16 px box, a struck-through label once done) instead of bare checkboxes; a task without steps shows the shared `EmptyState`; the new step is a `Field`.
- **Card:** the delete button of a task card is an `IconButton` that names the task. The per-card status, assignee and priority dropdowns and the board's search and assignee filter keep their accessible names without visible labels, so the cards and the toolbar stay compact.
- **Buttons:** *Create task* and *Add step* use `busy`.
- **CSS:** the checkbox rules of `.shared-task-checklist-list` and the dropdown width rule of the create fields are removed.

### Upgrading
- Nothing to do.

## V0.2.109 (2026-10-21)

Retrieval filters, saved searches, Ask and symbol search use the shared pieces.

### Changed
- **Search fields:** the evidence query and the symbol name are the shared `SearchField` (icon, clear button, Escape to clear), as the other list filters are.
- **Filters:** file type, language, source, evidence state, folder, the two dates and the number of results are `Field`s with a visible label ("File type", "Language", "Source", "Evidence state", "Folder", "Updated from", "Updated to", "Results") instead of unlabelled dropdowns that carried an `aria-label`. The filter row is a grid that fills the width and lines its buttons up with the fields; a dropdown in a field fills it.
- **Saved searches and Ask:** the search name and the question are `Field`s (the question's text area included); the delete button of a saved search is an `IconButton` that names it.
- **States and buttons:** a search that was not run yet, no saved search and no symbol found are the shared `EmptyState`; *Search*, *Find definition*, *Save current search* and *Ask* use `busy`.
- **CSS removed:** `.shared-retrieval-date` and the `justify-self` rule of the retrieval dropdowns.

### Upgrading
- Nothing to do.

## V0.2.108 (2026-10-21)

The evidence section's forms, filters and bulk actions use the shared pieces.

### Changed
- **Fields:** the new folder form, the rename-folder dialog and the note title use `Field`, with visible labels instead of hidden ones. The type filter (`Checkbox`) and the search (`SearchField`) were already shared and are unchanged.
- **Bulk selection:** the box that picks a file while *Select files* is on is the shared `Checkbox`, labelled "Select <file>" for screen readers.
- **Destructive dialogs:** *Delete folder*, *Delete file* and *Delete N files* are `danger` buttons (they were plain secondary buttons); *Rename*, *Move here*, *Create*, *Store evidence* and *Upload* show their work with `busy`.
- **States:** the empty ledger, an empty folder, a filter with no match and a document list with no selection are the shared `EmptyState`.
- **CSS:** the folder form lines its buttons up with the field; `.shared-select-box` loses its flex-only rules.

### Upgrading
- Nothing to do.

## V0.2.107 (2026-10-21)

The people section uses the shared fields, badges and segmented control.

### Changed
- **Fields:** the form that adds or updates a member (email and role) uses `Field`, with visible labels instead of hidden ones. The invitation form below it already uses them (V0.2.105).
- **Badge:** a member's role in the team list is the shared `Badge`, in place of the `.shared-member-role` pill.
- **States and buttons:** an empty activity history is the shared `EmptyState`; *Add / update* uses `busy`; the redundant `type="button"` of *Make owner* and *Remove* is dropped.
- **Segmented control:** *User activity* and *AI activity* already use `SegmentedControl` (V0.2.82); nothing changes there.
- **CSS removed:** `.shared-member-role`, the width rules of `.shared-member-form` inputs and its dropdown width rule. The form's columns (email, 92 px role, button) are unchanged and now align on the bottom of the fields.

### Upgrading
- Nothing to do.

## V0.2.106 (2026-10-21)

Workspace settings use the shared fields and switches.

### Changed
- **Fields:** the three AI provider forms (text, images, search: provider, name, base URL, model, API key), *Who can use AI* and the workspace rename use `Field`; the rename field has a visible label instead of a hidden one.
- **Checkbox:** the cloud acknowledgement of a provider is the shared `Checkbox`, with a clickable label, instead of a bare checkbox in a label.
- **States and buttons:** a settings page the role may not open shows the shared `EmptyState`; *Save provider* and *Test connection* each show their own work with `busy` (before, both swapped to a spinner at the same time); *Rename* and *Download archive* use `busy`.
- **CSS removed:** the label and toggle rules of `.shared-ai-provider-form` and its dropdown width rule (a dropdown in a field fills it). The workspace's repository links keep their form until V0.2.113, which moves the repository forms together.
- **Narrow screens:** the rename form stacks and stretches its field.

### Upgrading
- Nothing to do.

## V0.2.105 (2026-10-21)

The organization page uses the shared fields, badges and states.

### Changed
- **Fields:** renaming the organization, adding a member (email and role) and the invitation form (`InviteMembers`: address and role, now with visible labels instead of hidden ones) use `Field`; the role dropdowns are tied to their labels by the field's id.
- **Badge and states:** the *Owner* mark of the member list is the shared `Badge`, and the member list shows the shared `LoadingState` while it loads.
- **Buttons:** *Save name*, *Add member* and *Create invite link* use the `busy` prop.
- **CSS:** the label rules of `.shared-organization-rename` and `.shared-organization-member-form` now size the fields (`.rm-field`), the invite form lines its button up with the bottom of the fields, and the per-form dropdown width rules for both are removed. The invitation form is shared with the people section, which gets the same fields.

### Upgrading
- Nothing to do.

## V0.2.104 (2026-10-21)

The profile page and API tokens use the shared fields, switches and states.

### Changed
- **Fields:** *Email address* (new address, current password), *Password* (current, new, confirmation), each notification preference and the API token form (name, access, lifetime) use `Field`. The notification, access and lifetime dropdowns are tied to their labels by the field's id instead of an `aria-label` copy of the label; *Personal details* already did so.
- **Dropdowns in fields fill the field:** one rule, `.rm-field .rm-dropdown-trigger`, replaces the per-form rules that stretched them (`.shared-profile-panel label`, `.rm-tokens-form`). The notification preferences stay dropdowns, as each has more than two channels.
- **States:** an empty task list on the profile is the shared `EmptyState`; the API token list shows `LoadingState` while it loads and `EmptyState` when there is no token.
- **Buttons:** *Send the link again*, *Change address*, *Create token* and *Revoke token* use the `busy` prop.
- **CSS removed:** `.shared-profile-panel label`, `.rm-tokens-form label` and their dropdown-width selectors.

### Upgrading
- Nothing to do.

## V0.2.103 (2026-10-21)

The setup wizard and the environment menu use the shared fields, switches and states.

### Changed
- **Fields:** the setup code (wizard and environment menu), the administrator's name, email and password twice, the first organization and workspace names and the new environment's name use `Field` with an `Input`. Each control is tied to its label, the required ones are marked on the `Field`, and the environment name's rule ("It is created as <folder>/<name>") is the field's hint instead of a separate paragraph.
- **Switch:** a yes or no setting of the wizard's settings steps is the shared `Switch` (as in System › Settings) instead of the hand-made toggle with a bare checkbox.
- **States:** "Loading the settings…" and "Checking the server…" are the shared `LoadingState`; an environments folder with nothing in it is the shared `EmptyState`.
- **Buttons:** *Unlock*, *Continue*, *Create administrator*, *Create and continue*, *Save and continue*, *Check again*, *Finish setup*, *Refresh* and *Create and use* use the `busy` prop in place of a swapped-in spinner; redundant `type="button"` removed.
- **CSS removed:** the `.shared-setup-form` label and input rules, which only these forms used.

### Upgrading
- Nothing to do.

## V0.2.102 (2026-10-21)

The sign-in, registration, invitation, password reset and email verification pages use the shared fields, buttons and states.

### Changed
- **Fields:** the sign-in and registration form (`web/AuthCanvas.tsx`), the invitation form (`InvitePage`), the new-password form (`PasswordResetPage`) and the server address of the desktop app's first screen (`DesktopConnect`) use `Field` with an `Input`: every control is tied to its label with an id, the required ones are marked in one place (`required` on the `Field`), and all have the one control height. The invited address stays a read-only field.
- **Buttons:** the submit buttons, *Accept the invitation* and *Send the link* show that they are working with the shared `busy` prop (a spinner, the label kept, `aria-busy`) instead of swapping their icon for a hand-placed spinner; the `type="button"` that every `Button` already defaults to is dropped.
- **States:** the "Checking the invitation…" and "Checking the link…" lines are the shared `LoadingState`, with the same words as before.
- **CSS removed:** the `.shared-auth-form label` and `.shared-auth-form input` rules, which only these forms used. The setup wizard's `.shared-setup-form` rules stay for V0.2.103.

### Upgrading
- Nothing to do.

## V0.2.101 (2026-10-19)

The knowledge map, the document viewer and the assistant's chat list load through the query cache.

### Changed
- **Knowledge map:** read through `useServerState` (`workspaceKeys.knowledgeMap`). While files wait to be indexed or passages to be embedded it is read again every 5 s, up to 120 times, by the query's `refetchInterval`. Live evidence and memory events and the *Refresh* button read it again; only those show it as busy, not the reads while indexing runs.
- **Document viewer:** the preview, the render status of a Word or PowerPoint file (asked again every 1.5 s while the server converts it, up to 80 times), the rendered PDF and the original PDF are four queries (`workspaceKeys.artifactPreview`, `.artifactRenderStatus`, `.artifactRendered`, `.artifactFile`). The two PDFs get their browser addresses from the new `useObjectUrl`, which releases them when the file goes or the page is left. The viewer has no data-loading effect left.
- **Assistant:** the capabilities and the chat list are queries (`workspaceKeys.assistantCapabilities`, `.assistantChats`); renaming, deleting and adding a chat write to the cache. The chat that was open last time is still opened once the list is known. The palette's commands wait for the assistant in the new `useAssistantCommands` hook (two effects moved out of the component).
- **New small hooks:** `useErrorToast(error, fallback)` shows a failed query as a toast; `useObjectUrl(file)`; `useAssistantCommands({ ready, handle })`.
- A result that arrives after the page was left lands in the cache and nowhere else, in all three.

### Upgrading
- Nothing to do.

## V0.2.100 (2026-10-19)

The repositories and connectors pages load through the query cache.

### Changed
- **The list of a workspace's repositories, a repository's page, a repository's file list and the connectors list** are read through `useServerState`, with new keys (`workspaceKeys.repositories`, `workspaceKeys.connectors` and `repositoryKeys.detail` and `.files` in `lib/queryClient.ts`). A page shows its cached answer at once on a later visit and reads again when it opens. An answer that arrives after the page was left no longer reaches it.
- **The hand-made polling timers are the query's `refetchInterval`:** a repository list or page is read again every 1.5 s while a sync runs and the event stream is down (every 15 s when it is up), and the connectors list every 1.5 s while one syncs. They stop by themselves when the sync ends.
- **The end of a sync is announced from the data.** The toast ("synced", "stopped" or the failure) and the refresh of the workspace now follow a change of the cached list instead of the code that fetched it. The connectors page tells its parent only when a sync *ends*, not when a cached list is shown.
- A connectors list that cannot be read shows as empty with an error toast, as before.

### Upgrading
- Nothing to do.

## V0.2.99 (2026-10-19)

The System pages load through the query cache.

### Changed
- **Every System page reads its data through the cache** (`useSystemData`, now built on `useServerState`), with keys in the new `systemKeys` (`lib/queryClient.ts`): overview, usage (per period), users, settings, the environment status, log files, logs (per filter), backups, jobs (per status) and the audit trail. A page shows its cached answer at once on a later visit and reads again when it opens, as before.
- **An answer that arrives after a page was left is ignored by that page.** It lands in the cache, where the next visit finds it; no page sets state from it. This replaces the `useEffect` blocks that loaded with `.then(setState)`, and the timers that polled by hand (the overview every 30 s, the logs every 5 s while following, the backups every 2 s while one runs, the jobs every 10 s or 60 s).
- **`useServerState` takes `refetchInterval`** (a number, or a function of the data so a page can stop polling) **and `keepPrevious`** (keep showing the last answer while a changed filter is fetched).
- **Live job events invalidate the jobs query:** a resync, or a job of a workspace not yet listed, marks the list stale so it is read again; other job events still update the row in place. The Overview and Jobs pages share the maintenance status through the cached overview.
- The logs search is sent 300 ms after the last key, as before.

### Upgrading
- Nothing to do.

## V0.2.98 (2026-10-19)

The knowledge map page is split into its four parts.

### Changed
- **`components/KnowledgeMapPanel.tsx` (530 lines) becomes `web/map/`:** `KnowledgeMapPanel.tsx` (loading, live refresh and the polling while indexing runs), `Pipeline.tsx` (from files to searchable knowledge), `Coverage.tsx` (coverage by kind of file), `RelationGraph.tsx` (the graph and its viewport), `SelectionPanel.tsx` (the side panel of the selected dot), `EdgeTable.tsx` (the table of links) and `shared.ts` (the file groups, the canvas size and the small helpers). The longest file is 263 lines.
- **Nothing else changes:** the code was moved as it was. The hover label's type is called `HoverTip` so that it no longer shares a name with the `Tooltip` component.
- **`test/splitModules.test.ts`** now guards `web/map` too.

### Upgrading
- Nothing to do.

## V0.2.97 (2026-10-19)

The setup wizard is one component per step.

### Changed
- **`components/ServerSetup.tsx` (375 lines) becomes `web/setup/`:** `ServerSetup.tsx` (which step is shown, the setup code, the settings read and the choices carried to the last step, plus the waiting page), `StepFrame.tsx` (the shared frame: header, list of steps and panel, and the step heading), `steps.tsx` (the step list and the settings offered), and one file per step: `WelcomeStep`, `AdminStep`, `ChecksStep`, `SettingsStep` (used for *Access and security* and for *Logging*), `OrganizationStep` and `FinishStep`. Each step keeps its own form state. The longest file is 99 lines.
- **Two copies of the "settings were saved" handler are one** (`settingsSaved`), and the page shell that wrapped every step is the new `StepFrame`. Nothing else changes: the wizard looks and behaves as before.
- **`test/splitModules.test.ts`** now guards `web/setup` too.

### Upgrading
- Nothing to do.

## V0.2.96 (2026-10-19)

The repositories pages are split by what they show.

### Changed
- **`components/RepositoriesPanel.tsx` (872 lines) becomes `web/repositories/`:** `RepositoriesPanel.tsx` (the list of a workspace's repositories), `RepositoryCard.tsx` (one repository in that list), `RepositorySettings.tsx` (the link form, the repositories' settings and their removal), `LinkForm.tsx` (the fields and the access check of a link), `RepositoryPage.tsx` (the page of a whole repository, its summary and overview), `FileBrowser.tsx` (the file list), `useRepositories.ts` (loading and following syncs) and `shared.ts` (constants and helpers). The longest file is 260 lines.
- **Nothing else changes:** the code was moved as it was. `web/lazy.tsx` loads the three pages from their new files.
- **`test/splitModules.test.ts`** now guards `web/repositories` too.

### Upgrading
- Nothing to do.

## V0.2.95 (2026-10-18)

The System area is one file per page.

### Changed
- **`components/SystemAdminPanel.tsx` (887 lines) becomes `web/system/`:** `SystemAdminPanel.tsx` (the page header, the tabs and the choice of page), `Overview.tsx`, `Usage.tsx`, `Users.tsx`, `Settings.tsx`, `Logs.tsx`, `Jobs.tsx`, `Backups.tsx`, `Audit.tsx`, `Maintenance.tsx` (shared by Overview and Jobs) and `shared.tsx` (the data hook, `RefreshButton`, `Facts`, `BarStrip`, `CountTable`). The longest file is 178 lines.
- **Nothing else changes:** the code was moved as it was, so the pages look and behave as before. `web/SystemArea.tsx` imports the area from its new place.
- **`test/splitModules.test.ts`** checks that no file of a split folder grows past 300 lines. It lists `web/system` and later releases add their folders.

### Upgrading
- Nothing to do.

## V0.2.94 (2026-10-18)

A small hook holds a form's values and tells whether they changed.

### Changed
- **`useFormState(initial)`** (`lib/useFormState.ts`) returns `{ values, initial, isDirty, set, patch, reset }`: the current values, the values the form started from, whether they differ, a setter for one field or several, and a way back. When `initial` changes (the saved record was loaded, or a save updated it) the form starts over from the new values, so a page needs no effect that copies a record into its fields.
- **Profile › Personal details uses it** for the display name, in place of a `useState` and the effect that copied the loaded name into it. The form looks and behaves as before.

### Upgrading
- Nothing to do.

## V0.2.93 (2026-10-18)

One hook runs a mutation, with its busy flag and its toasts.

### Changed
- **`useAsyncAction()`** (`lib/useAsyncAction.ts`) returns `{ isBusy, run }`. `run(action, { success, failure })` sets the busy flag, shows a success toast (a text, or a function of what the action returned) and shows an error toast when the action throws (the error's own message, or `failure` when it has none). It resolves to `true` or `false`. Overlapping actions keep the flag on until the last is done, and nothing is set after the component left.
- **The profile page and the task board use it.** Their thirteen `setIsSubmitting(true) … try … catch … finally` blocks are gone. Profile no longer keeps `error` and `notice` states for two `<Toast>` elements: its messages go straight to toasts, with the same texts. The task board's busy flag is its own instead of the workspace page's, so it disables only the task board's controls.

### Upgrading
- Nothing to do.

## V0.2.92 (2026-10-18)

One helper turns an error into the text of a toast.

### Changed
- **`apiMessage(error, fallback)`** in `web/format.ts` gives an error's message, or `fallback` when it has none (a thrown string, an empty message). Without a fallback it says the server could not be reached.
- **Twelve local helpers are deleted** (`errorMessage` in ten components, `message` in the mail settings, `errorText` in the document viewer) and eighteen inline `error instanceof Error ? error.message : "…"` checks use `apiMessage` instead. A message that is empty now shows the fallback in the few places that showed nothing.

### Upgrading
- Nothing to do.

## V0.2.91 (2026-10-18)

Dates, sizes and durations are formatted in one place.

### Changed
- **`web/format.ts` holds the formatters:** `formatDate` (a date), `formatTime` (a date with its time), `formatRelativeTime` ("3h ago"), `formatRecentTime` (the time today, the day before), `formatBytes`, `formatDuration` and `formatShare`. Each takes an empty value and one that is not a date without failing (a fallback or the text as it is).
- **Fifteen local copies are deleted** from the API tokens, assistant, connectors, document viewer, invitation, language chart, move-to-server, import, repositories, System and workspace health components (`formatDate`, `formatBytes`, `formatSize`, `formatDuration`, `formatTime`, `formatChatTime`, `formatShare`). The retired local edition (`App.tsx`) keeps its own.
- **Small wording differences are gone:** a size reads `512 B`, `48 KB`, `1.5 MB`, `GB` or `TB` everywhere (it was `bytes`, `KiB` or `MiB` in four places), and a date with a time reads the same on every page.
- **`Time`** (`components/ui/time.tsx`) shows a relative time in a `<time>` element, with the exact date and time in a tooltip. No page uses it yet; the pages of K4 do.

### Upgrading
- Nothing to do.

## V0.2.90 (2026-10-18)

One list style for palettes, suggestions, pickers and chats.

### Changed
- **`rm-option-list` and `rm-option`** are the shared style of a list you pick a row from: a row as tall as a control, one highlight for hover and for the active row (read from `aria-selected`, `aria-pressed` or `aria-current`), one focus ring, a muted small line, group labels (`rm-option-group`), a boxed variant (`rm-option-list-box`) and a row that holds a menu button (`rm-option-row`).
- **Four lists use it, and their own rule sets are gone:** the command palette, the mention suggestions of notes and comments, the folder picker of *Move to folder* and the assistant's chat list. Each keeps only what is its own: the palette's width and height, the suggestion list's popover position, the chat list's hidden-until-hover menu.
- The *Mentioned in* and *Mentions* lists of an evidence page used the suggestion list's class by accident (and so its popover position); they are plain lists now.

### Upgrading
- Nothing to do.

## V0.2.89 (2026-10-18)

One copy button with the same feedback everywhere.

### Changed
- **`CopyButton`** is in `components/ui/copy-button.tsx`. It puts a value on the clipboard, turns its icon into a check (and its word into "Copied") for two seconds, announces it politely to screen readers and, when the browser refuses (plain http, a denied permission), shows a warning toast and runs an optional `onUnavailable` (the secret dialog selects its field for a manual copy). Icon-only through `IconButton` by default, or a button with a word through `text`.
- **It replaces the copy code of the secret dialog** (invitation links and tokens shown once), whose own clipboard call, success toast and state are gone, and **adds the copy action next to** a repository's last synced commit (the repository page and its settings), a repository file's commit and path on the evidence page, and the payload URL and secret of a webhook (repositories and connectors). Before, those values could only be selected by hand.

### Upgrading
- Nothing to do.

## V0.2.88 (2026-10-18)

One search field for list filters.

### Changed
- **`SearchField`** is in `components/ui/search-field.tsx`: `<SearchField aria-label value onValueChange />`. A text field with a search icon, a clear button once there is text (kept out of the tab order; Escape does the same from the keyboard, and focus stays in the field) and the shared field look. It forwards its ref and any `input` attribute, and `clearOnEscape={false}` leaves Escape alone where it already closes something.
- **It replaces** the evidence file filter, the System users filter, the repository file filter and the command palette's field, plus the System logs search and the task search, which were the same kind of filter. The palette keeps Escape for closing its dialog, and its hand-made field box is gone. The searches that run from a button (Retrieve, Ask, the team memory search, searching every workspace) keep their forms.

### Upgrading
- Nothing to do.

## V0.2.87 (2026-10-18)

The searchable picker where lists are long.

### Changed
- **`Combobox` replaces three long dropdowns:** *Replacement evidence* in the evidence lifecycle form, a task's *Related evidence* in the new-task form, and *Open a task checklist* in the task toolbar. Each listed every file or task of the workspace; they now open a search field and narrow as you type.
- `Combobox` gained `noneLabel`, which adds a first option that clears the choice ("No evidence link" for a task).

### Upgrading
- Nothing to do.

## V0.2.86 (2026-10-18)

A searchable picker for long lists.

### Changed
- **`Combobox`** is in `components/ui/combobox.tsx`, built on Radix Popover (`@radix-ui/react-popover` is a new dependency): a trigger that looks like a dropdown opens a popover with a search field and the matching options. Every typed word must appear in the option's label or description (so a path can be searched next to a title), the arrow keys, Home and End move through the matches, Enter chooses and Escape closes. At most 100 rows are drawn; the rest are summarised in one line until the text narrows the list.
- **First use:** the memory card editor, where *Add a file…* was a native `<select>` over every file of the workspace. Evidence is now picked by typing part of a file name. The select was the last raw `<select>` of the web client.

### Upgrading
- Nothing to do.

## V0.2.85 (2026-10-18)

One table style for the System, map and repository tables.

### Changed
- **`Table`** is in `components/ui/table.tsx`, with the `rm-table` styles: `<Table>` wraps the `<thead>` and `<tbody>` in a box that scrolls sideways on narrow screens and, past 640 px of height, keeps the header in view. Rows highlight on hover, cells use tabular figures, and a numeric column takes `className="rm-table-number"` to align right. The box forwards its ref and attributes, and an `overlay` slot holds the map's hover tooltip.
- **Twelve tables use it:** the System overview clients, usage (activity by kind, most active people, workspaces), users, backups, jobs and audit trail; the System console results; the knowledge map's coverage and link tables; the workspace health checks; and the repository structure. They shared one `rm-map-coverage` class, which is gone along with its rules. The count columns of the coverage, health, repository and System count tables are right-aligned; the other numeric columns follow with their pages (V0.2.102 to V0.2.118).
- The workbook sheet preview keeps its own spreadsheet-like table style.

### Upgrading
- Nothing to do.

## V0.2.84 (2026-10-18)

One header for every page.

### Changed
- **`PageHeader`** is in `components/ui/page-header.tsx`: `<PageHeader title description actions back />`. The title is the page's `h1`; the description sits under it; `actions` are buttons at the end (they stack under the title on narrow screens); `back` is an optional link to the page this one belongs to (no page uses it yet).
- **Seven page shapes use it:** the dashboard (and its organization view), the workspace pages, the artifact and memory card pages (through the shared record layout), the profile, the notifications and every System page. `shared-page-heading` and `shared-detail-heading` are gone.
- **Title sizes now follow DESIGN.md.** The two old styles disagreed (34 to 52 px on detail pages, 42 to 72 px on the dashboard and System); every page title is 28 to 32 px now, and panel titles are 16 px (they were 15). This is the one visible change of the release.

### Upgrading
- Nothing to do.

## V0.2.83 (2026-10-18)

One heading for every panel.

### Changed
- **`PanelHeading`** is in `components/ui/panel-heading.tsx`: `<PanelHeading icon title description actions level id />`. The icon is a Tabler component drawn at the heading's size (18 px in an `h2`, 16 px in an `h3`), `description` is the muted note at the end (a count, a state), `actions` hold buttons or a status element, and `id` lets a section name itself by it with `aria-labelledby`.
- **It replaces all 59 hand-built `shared-panel-heading` blocks** in 24 files (System, Profile, repositories, connectors, the knowledge map, workspace pages and others). The rules moved to `14-building-blocks.css` as `rm-panel-heading`, and the page-specific margin and note rules were renamed with it; `grep shared-panel-heading` finds nothing. The two status pills in a heading (a repository's status, an evidence lifecycle status) now take their own size and colour instead of the heading's note style.

### Upgrading
- Nothing to do.

## V0.2.82 (2026-10-18)

One segmented control for switches and tab strips.

### Changed
- **`SegmentedControl`** is in `components/ui/segmented-control.tsx`, built on Radix Toggle Group (`@radix-ui/react-toggle-group` is a new dependency). Options have a label, an optional icon and count, or only an icon with a `name`; one is always chosen; the arrow keys, Home and End move between them. `fill` shares the width, `followFocus` makes the arrow keys choose as they move.
- **It replaces five hand-made switches:** sign in or create account (`shared-auth-switch`), the activity type filter of People (`shared-activity-filter`), the grid and list switch of the evidence view and the graph and table switch of the knowledge map (`shared-artifact-view-switch`, which were `IconButton`s) and the sheet tabs of a workbook preview (`shared-sheet-tabs`, with `followFocus`). Their styles are gone; the sheets are a labelled group of radio buttons now, with the sheet shown in a region of its name.
- `rm-desktop-switch` was a single button (*Connect to a server*) with no style of its own, not a switch; it lost the class.
- The workbook sheets test in `accessibility.test.tsx` was rewritten for the new roles (written, not run; CI runs it).

### Upgrading
- Nothing to do.

## V0.2.81 (2026-10-18)

One badge for tags, roles and chips.

### Changed
- **`Badge`** is in `components/ui/badge.tsx`: `<Badge tone icon>`. A neutral surface with a border and grey text; a tone (`success`, `warning`, `danger`, `info`) adds a 6 px status dot and nothing else, so there is no tinted fill with text of the same hue, as DESIGN.md asks. It forwards its ref, so a `Tooltip` can wrap it.
- **It replaces six classes:** `rm-system-tag` and `rm-system-role` (token scopes, the client of an access log line, backup trigger and encryption, user roles, deactivated users), `shared-review-badge` (*Needs review* on a memory card, now with a warning dot and a tooltip), `rm-console-tag` (*dry run*, which had a tinted warning fill), `workspace-context-chip` (the retired local edition's workspace chip) and `rm-stack-chip` (a repository's technologies). Their rules are gone; an expired API token now has a danger dot.
- The role badges keep their hints as tooltips instead of `title` attributes.

### Upgrading
- Nothing to do.

## V0.2.80 (2026-10-18)

One empty state for lists with nothing in them.

### Changed
- **`EmptyState`** is in `components/ui/empty-state.tsx`: `<EmptyState icon title description action />` is a muted block centred in its panel, with an illustration-sized icon, a title, one sentence and at most one action.
- **First use:** the task board when no task matches the filters, the memory cards (one wording when a search finds nothing, another when the workspace has no card yet) and the notifications page. The memory cards had a bare line of text and now say what to do next. The older `shared-empty-state` block stays on the pages that still use it until their move in V0.2.102 to V0.2.118.

### Upgrading
- Nothing to do.

## V0.2.79 (2026-10-18)

One loading state with one wording rule.

### Changed
- **`LoadingState`** is in `components/ui/loading-state.tsx`: `<LoadingState label="Loading jobs" />` shows the `Spinner` and one muted line, announced as a status. The rule for the label is "Loading <thing>"; the component adds the ellipsis.
- **First use:** the System pages. The overview, usage, users, settings, logs, backups, jobs and audit pages each had their own sentence ("Loading the system overview…", "Loading usage…" and so on, some with a spinner, some without) and now share the component and its look.

### Upgrading
- Nothing to do.

## V0.2.78 (2026-10-18)

No more browser confirmations.

### Changed
- **`useConfirm` replaces the last four `window.confirm` calls** of the web client: handing an organization to a member (Organization), handing a workspace to a member (People), deleting a workspace (Settings, with the danger button) and deleting a task (Tasks, with the danger button). Handing over names the member in the title and says in the description that the previous owner stays on as an administrator and cannot take it back.
- `grep window.confirm` finds nothing in the web client.

### Upgrading
- Nothing to do.

## V0.2.77 (2026-10-18)

Confirmations in the app's own dialog.

### Changed
- **`ConfirmDialog` and `useConfirm`** are in `components/ui/confirm-dialog.tsx`. `const confirm = useConfirm(); if (!(await confirm({ title, description, action, danger }))) return;` opens the app's dialog with Cancel on the left and the decisive action on the right (the `danger` button when `danger` is set) and resolves `true` or `false`. Escape and a click outside answer `false`. `ConfirmProvider` hosts the one dialog and is mounted in `SharedWebApp`; a second question while one is open answers the first with `false`.
- **First use:** deleting evidence (it names the file and says that linked memory citations are removed), deleting an evidence comment and deleting a memory card. They were the browser's `window.confirm`.

### Upgrading
- Nothing to do.

## V0.2.76 (2026-10-18)

A danger button for destructive confirmations.

### Changed
- **`variant="danger"` on `Button`.** A solid `--color-danger` fill with white text and a darker hover, defined once in `14-building-blocks.css`. It replaces the one-off `.rm-dialog-footer .rm-button-danger` rule, which only worked by className inside a dialog footer.
- **Used by** the confirmations that delete or remove for good: deleting a folder, a file or several files, removing a repository, removing a connector and deleting an assistant chat. The `rm-button-danger` class that five row buttons (*Remove*, *Turn off*, bulk delete) carried without effect is gone; they stay secondary buttons.

### Upgrading
- Nothing to do.

## V0.2.75 (2026-10-18)

Icon-only buttons name themselves in a tooltip.

### Changed
- **`Tooltip`** is in `components/ui/tooltip.tsx`, built on Radix Tooltip (`@radix-ui/react-tooltip` is a new dependency): a small label in inverted greys above the control, after 400 ms of hover and at once on keyboard focus. It sits in the popover layer, so it shows above dialogs.
- **`IconButton` shows its `label` in a tooltip** instead of a `title` attribute, which keyboard users never saw. The theme toggle of the account controls is an `IconButton` now.
- **The other `title` hints on controls** became tooltips: *New folder* in the evidence browser (including why it is disabled at the folder depth limit) and *Similar content* in the knowledge map when AI for search is not set up.

### Upgrading
- Nothing to do.

## V0.2.74 (2026-10-18)

A shared button for a single icon.

### Changed
- **`IconButton`** is in `components/ui/icon-button.tsx`: a square button holding one icon with a required `label` (its `aria-label`, and for now its `title`). It is 32 px (`size="sm"`) or the control height of 36 px (`md`, the default), has an `outline` or a `plain` look (bare until hovered) and takes `pressed` for a toggle that is on and `busy` for work in progress.
- **Where it is used:** the *more actions* trigger of every `ActionMenu` (it was `rm-action-trigger`, 28 px, now 32 px), the System pages' *Refresh* button (an icon now, not an icon with a word), the remove button of an organization member, and the grid and list switch of the evidence view and the graph and table switch of the knowledge map (they were 34 px squares).
- Removed: `rm-action-trigger` and the 34 px rules of the view switch and of the organization member actions. The latter also applied to the *Make owner* text button, which now has the normal button width.

### Upgrading
- Nothing to do.

## V0.2.73 (2026-10-18)

Buttons can show that they are working.

### Changed
- **`busy` on `Button`.** `<Button busy={isSubmitting}>` puts a spinner in front of the label, hides the button's own icon, disables the button and sets `aria-busy`. The label does not change, so the button keeps its width. A new `Spinner` (`components/ui/spinner.tsx`) is the turning ring, also for inline use.
- **First use:** the sign-in and account creation button, and Profile's *Save details*, *Change password* and *Sign out everywhere* buttons, which each had their own spinner swap.

### Upgrading
- Nothing to do.

## V0.2.72 (2026-10-18)

A button only submits a form when it says so.

### Changed
- **`Button` is a plain button by default.** It renders `type="button"` unless the caller gives a type, so a button inside a form no longer submits it by accident (an HTML button submits by default). `asChild` buttons keep whatever the child element is.
- **Every form was checked** with a syntax-tree pass over the web client: every `Button` and native button in the web client already carried an explicit `type`, and every form has exactly one `type="submit"` button, either inside it or tied to it with `form="…"` (the repository edit dialog). The one form without a submit button, *Who can use AI* in the workspace settings (a single dropdown that saves when it changes), is a `div` now: it never submitted anything.
- No submit button lives outside a form without a `form` attribute.

### Upgrading
- Nothing to do.

## V0.2.71 (2026-10-18)

A shared switch for settings that turn on and off.

### Changed
- **`Switch`** is in `components/ui/switch.tsx`: `<Switch checked onCheckedChange label>`, built on Radix Switch (`@radix-ui/react-switch` is a new dependency). A 36 by 20 px track with a round thumb, the accent colour when on, with a keyboard focus ring. It takes a `label` beside it (clicking the label toggles it), an `aria-label`, or a `<label htmlFor>` tied to its `id`.
- **First use:** the boolean settings of System › Settings, which were a bare checkbox next to "On" or "Off". They keep their saving: a change is still saved with the page's save button. The other hand-made toggles (`shared-ai-provider-toggle`, in mail settings, the setup wizard and the log follow switch) move with their pages in V0.2.103 to V0.2.116.

### Upgrading
- Nothing to do.

## V0.2.70 (2026-10-18)

A shared checkbox with a label and a description.

### Changed
- **`Checkbox`** is in `components/ui/checkbox.tsx`: `<Checkbox checked onCheckedChange label description>`, built on Radix Checkbox (`@radix-ui/react-checkbox` is a new dependency). A 16 px box that is the accent colour when checked, its label (clicking it toggles the box) and an optional muted description under it. It is a button with the checkbox role, so the keyboard and screen readers behave as for a native one.
- **First use:** the evidence type filter (*All types* and one row per file type). Its hand-drawn check mark and the seven rules that styled the bare inputs of that menu are removed; the rows look the same.

### Upgrading
- Nothing to do.

## V0.2.69 (2026-10-18)

A shared field: a label, a control and a short message.

### Changed
- **`Field` and `FieldRow`** are in `components/ui/field.tsx`. `<Field label hint required invalid message htmlFor>` renders a label tied to its control (the id is generated when none is given), an optional muted hint and, while `invalid`, a one-line message under the control. The control gets `aria-invalid` and `aria-describedby`; only its border turns red, with no banner and no fill. `FieldRow` puts fields side by side and stacks them below about 220 px. The styles are in a new `14-building-blocks.css`, which holds every building block of this track.
- **First use:** Profile › Personal details (display name and email), whose labels are now tied to their fields. The look is unchanged. Required fields are still the browser's own `required`; marking and blocking them with a message comes in V0.2.122.
- `DESIGN.md` has a *Building blocks* section that lists each shared piece.
- The accessibility check counts a control inside a `Field` as labelled.

### Upgrading
- Nothing to do.

## V0.2.68 (2026-10-17)

Icons come in three sizes.

### Changed
- **One constant for icon sizes.** `ICON_SIZE = { sm: 14, md: 16, lg: 18 }` is in `components/ui/icon.ts`. The 503 icons from 12 to 18 px, in 53 files, read it instead of a number: icons of 12, 13 and 15 px became 14 px (the small ones in rows, badges and compact actions) and 17 px ones became 16 px. They used to come in seven sizes, 12 to 18 px. Nothing else in these files changed.
- **Illustrations keep their size**: avatars, the empty states (25 and 26 px), the setup steps and their status icons (20 and 22 px), file and folder tiles, the page loader, the assistant's introduction and the retired local edition's larger icons.

### Upgrading
- Nothing to do.

## V0.2.67 (2026-10-17)

Checkboxes and radio buttons are the same size everywhere.

### Changed
- **One rule for checkboxes, radio buttons and file pickers.** A base rule in `01…` sets a checkbox or radio button to 16 px with the accent colour and leaves a file picker its natural width. Six places had patched the size of their own (the evidence type filter, the task checklist, the AI provider and System toggles, the local edition's provider toggle and filter options, and the duplicate-file radio buttons); the size and colour parts are removed and only their vertical alignment stays.
- The evidence type filter keeps its drawn check mark until the shared `Checkbox` (V0.2.70) replaces it.

### Upgrading
- Nothing to do.

## V0.2.66 (2026-10-17)

Every form uses the same field.

### Changed
- **Page forms no longer resize the shared field.** The sign-in, setup and create-workspace fields were 42 px high, the member and memory fields 38 px with a 1 px border or 10 px of padding, each with its own background and corner. Those rules are gone: the fields of these forms and of the note and search forms are the shared field (36 px, a 2 px border, an 8 px corner), and the stylesheet keeps only their layout (full width, the create-workspace field's width, the minimum height of text areas). The comment form never overrode it.
- `.shared-memory-form select` is deleted; no form has a raw select any more.

### Upgrading
- Nothing to do.

## V0.2.65 (2026-10-17)

Fields are styled by their shared classes only.

### Changed
- **The competing global field rules are out of the web client.** Three more stylesheets (`09…`, `10…`, `11…`) restyled every bare `input`, `textarea` and `select` — a 6, 8 or 10 px corner, a 1 or 2 px border, a white, grey or surface background — and the dark theme repeated each. They now apply only inside the retired local edition (the same `:where(.rm-local-edition)` scope as V0.2.64, so that edition's look and cascade are kept); in the web client a field is styled by `.rm-input`, `.rm-textarea` or `.rm-dropdown-trigger`, plus one base reset in `01…` (font and colour inherited, the focus ring).
- The evidence picker of the memory card page, a raw `select` until V0.2.86 replaces it, takes the shared field class.
- Checkboxes, radio buttons and file pickers are no longer stretched like text fields; V0.2.67 gives them their size and accent colour explicitly.

### Upgrading
- Nothing to do.

## V0.2.64 (2026-10-17)

The retired local edition's styles stay in the local edition.

### Fixed
- **The web client's fields inherited the local edition's rules.** The local app styled bare `input`, `textarea` and `select` elements (a borderless field with a 6 px corner, then a 3 px one with a 1 px border) and some classes (`.paste-language`, `.workspace-create-controls`, `.search-input-wrap`, `.filter-options`, `.artifact-search`) for the whole page, so every field in the web client had to override them. These 36 selectors are now under `:where(.rm-local-edition)`, a class the local app's root carries. `:where` adds no specificity, so the local edition keeps the cascade it had; the web client matches none of them.

### Upgrading
- Nothing to do.

## V0.2.63 (2026-10-17)

Colours come from tokens.

### Changed
- **No raw colours in the page rules.** Every hex, `rgb()` and `rgba()` colour outside a `:root` block in the stylesheets now reads a token: whites are `--color-surface` (or `--color-on-accent` for text on a solid fill), the light greys are the neutral tokens, status colours are the status tokens, focus rings and hover shadows are `color-mix` of the accent or of black. Fields, panels and chips in the dark theme are no longer white where an older rule hard-coded it.
- **One danger red.** The destructive button in dialog footers used its own `#c9372c`; it uses `--color-danger`, with `--color-on-danger` for its text (white on the light theme, dark on the dark theme's light red). Destructive menu items use `--color-danger` too.
- **New tokens** for what had no token: `--color-toast-error`, `-success`, `-warning`, `-info` (solid toast fills), `--file-markdown` and the other file-type icon colours, and the knowledge map's `--viz-*` colours, which moved to `:root`.

### Removed
- **164 token declarations that a later stylesheet overrides.** The cascade's last `:root` blocks decide every token (the blocks in `01…`, `09…` and `10…`, with the green accent no page ever showed, were dead). They are deleted; the effective value of every token in both themes was compared before and after and is identical.

### Upgrading
- Nothing to do.

## V0.2.62 (2026-10-17)

One accent: the workspace palette is retired.

### Changed
- **The `--workspace-*` colours are gone.** Seven colours, a dot-grid colour and a font, with a dark-theme copy, were defined for the "workspaces v3" pages and read about sixty times. They now read the shared tokens (`--color-accent`, `-accent-hover`, `-accent-soft`, `-accent-text`, `--color-border`, `-border-strong`, `--color-surface-2`, `-surface-3`, `--font-ui`). `grep -- --workspace- styles` finds nothing.
- Those pages are the retired local edition's (`App.tsx`): the web client does not use them, so it looks as before, and the local edition's teal turns into the shared accent.

### Upgrading
- Nothing to do.

## V0.2.61 (2026-10-17)

Backgrounds, borders and text are true greys.

### Changed
- **The light theme's neutrals have no hue.** The tokens that win the cascade (`--color-bg`, `-canvas`, `-surface*`, `-code-surface`, `-text*`, `-border*`, in `11-blueprint-primitives.css`) were blue-grey (hue 220); they are now greys of the same lightness (the muted-subtle text is one point darker, 42 % instead of 43 %, to keep its contrast on hovered rows). The dark theme already was grey. The accent and the status colours are untouched, and the contrast check still passes.
- The older, overridden copies of these tokens in `01…`, `09…` and `10…` (green-grey) are greys too, so no neutral token has a hue anywhere.

### Fixed
- **The plan of track K was corrected.** Its audit read the first stylesheet's tokens and took the accent for green and the neutrals for green-tinted; the tokens that win the cascade are the last ones, where the accent is blue, as DESIGN.md says. Decision 16 now keeps that blue, and V0.2.63 removes the overridden token blocks.

### Upgrading
- Nothing to do.

## V0.2.60 (2026-10-17)

Text fields, dropdowns and buttons are the same height.

### Changed
- **One size for controls.** The tokens `--control-height` (36 px), `--control-height-touch` (40 px), `--control-radius` (8 px), `--control-border` (2 px) and `--control-padding-x` (12 px) are in `01-tokens-and-shared-web.css`. Text fields, text areas, dropdowns, dropdown rows and buttons use them, so they are 36 px high instead of 34 px, which is what DESIGN.md says (36–40 px); on narrow screens they use the touch height of 40 px. Colours, borders and corners look as before.
- Fields of the sign-in, setup, member and memory forms keep their own sizes until V0.2.66.

### Upgrading
- Nothing to do.

## V0.2.59 (2026-10-17)

Dropdowns no longer stretch across toolbars and filter rows.

### Changed
- **A dropdown is as wide as its content, 160 px at least.** It used to fill whatever it was placed in, so a dropdown in the System logs toolbar, the Jobs filter or the Tasks and Retrieval filter rows stretched across the row. The Retrieval and Tasks filters stay at the start of their cell.
- **Forms still stretch it.** The forms whose dropdown is a field (AI providers and policy, API tokens, invitations, the move-to-server dialog, evidence lifecycle, profile notifications, members, task creation and cards, mail and System settings, repository and connector forms) say so in one rule, until the shared `Field` (V0.2.69) does.

### Upgrading
- Nothing to do.

## V0.2.58 (2026-10-17)

Long dropdown lists stay inside the window.

### Fixed
- **A long dropdown list ran off the screen.** The list had no maximum height and clipped what did not fit, so a list opened near the bottom of the window (the level in System › Settings, the language filter in Retrieval) could not be reached. A list is now at most 320 px tall or the room the window leaves, whichever is less, and scrolls; arrows at the top and the bottom scroll it while the pointer rests on them.

### Upgrading
- Nothing to do.

## V0.2.57 (2026-10-17)

Dropdown lists and menus open above dialogs. The first release of the frontend polish track (K1, [plan](mindmap/plans/2026-10-17-frontend-polish.md)): it changes no look and no behaviour besides the stacking.

### Fixed
- **A dropdown inside a dialog opened behind it.** A dropdown list was at layer 50 and a dialog at 901, so the list of a dropdown in the API token, connector, move-to-server, System and profile dialogs was hidden. Dropdown lists, action menus, mention lists and the map tooltip are now above dialogs.

### Changed
- **One layer scale.** `--layer-raised` (1), `--layer-sticky` (10), `--layer-float` (20), `--layer-sheet` (800) and `--layer-sheet-panel` (801) for the assistant on narrow screens, `--layer-overlay` (900), `--layer-dialog` (901), `--layer-popover` (950) and `--layer-toast` (1000) are tokens in `01-tokens-and-shared-web.css`. Every `z-index` of the stylesheets uses one, except a single `0` behind the pipeline connectors.

### Upgrading
- Nothing to do.

## V0.2.56 (2026-10-16)

The clean-machine install is prepared. `V0.3.0` is not tagged.

### Added
- **A runbook for the run before the tag** ([mindmap/technical/clean-install.md](mindmap/technical/clean-install.md)): fourteen steps from getting the server to the desktop app, each with the guide section it follows and what you should see, a list of what has to be done before the tag, and a table for the result. A workaround is anything the guide does not say; each becomes a fix and the run is made again.
- **A smoke test that covers the new surfaces.** `scripts/smoke-test.mjs` also checks that sign-in refuses a 200 KiB body and that an unverifiable webhook answers `401`, and with an administrator reads the mail settings, sends a test mail to the address in `REPOMEMO_SMOKE_MAIL` and waits for it to leave the outbox, and checks that a remote repository can be linked (`REPOMEMO_SMOKE_REMOTE_REPO`, nothing is stored).

### Not done
- **The install itself.** It needs a second machine, a mail account and a running server. Until it passes with no workaround, and until the tests written since V0.2.46 have been run, the performance runs made and the browser passes done, `V0.3.0` should not be tagged. The page lists each of them.

### Upgrading
- Nothing to do.

## V0.2.55 (2026-10-16)

The documentation matches the code, and a check keeps it so.

### Added
- **A documentation check in CI** (`npm run docs:check`). It fails when a route of the server has no row in the API reference, when a setting the code reads is not described in the development commands, when a mindmap or the release index describes another version than the product's, when a release is done in the index and missing from this changelog (or the other way round), or when a link in the documents points at a file or a heading that is not there.
- **Upgrade notes from 0.2.x** in `UPGRADING.md`: what stays off until you turn it on (mail, remote repositories and trackers, sealed backups, containers), what changed in behaviour, and a build note about line endings.

### Changed
- **The API reference** gained the nine routes it was missing (mail settings and outbox, verification, notification preferences), the route count is right (203 in the main router) and it lists the body limits.
- **Pages that were out of date** now say what shipped: the repository sources page (remote links, credentials, webhooks), the connectors, production, console and authentication pages, which has a new section for what the 0.2.x line added. The technical mindmap's counts of migrations and tests are current.
- `scripts/list-routes.mjs` finds every route again (it had lost the ones with a body limit of their own) and reads Windows checkouts correctly.

### Upgrading
- Nothing to do.

## V0.2.54 (2026-10-16)

Performance runs for repositories and trackers.

### Added
- **A performance run for a repository of 100,000 files.** `repo_sync_at_scale` builds a repository that size, syncs it, syncs it again with nothing changed and after one changed file, and then asks the workspace what it is asked (search, the map, health). A remote repository is read by the same engine as a local one, so this measures everything after the clone.
- **A performance run for a tracker with 10,000 issues.** `ten_thousand_issues_sync_and_stay_searchable` serves that many issues the way GitHub pages them, syncs them through a connector pass after pass, times their indexing and asks the same questions.
- **[A page for performance](mindmap/technical/performance.md):** the targets, the four commands (these two and the earlier two at 50,000 passages), what each measures, and a table for the figures.

### Not done
- **The runs were not made.** They need release builds, which the automated work does not make, so no result is claimed and "the targets still hold" is open until the maintainer runs the four commands and records the figures in the page's table.

### Upgrading
- Nothing to do. The runs are tests marked as ignored: they never run in CI or with `cargo test`.

## V0.2.53 (2026-10-16)

Upgrading from 0.2.0 is tested.

### Added
- **An upgrade test in CI.** A server environment made by 0.2.0 (its database schema, the rows it wrote and the files it stored) is opened by the current code, which migrates it in place, and everything in it is read back: the people, the workspace, its folder, files and their text, the memory card with the passage and the file it cites, the verified file with its history, the task with its checklist, the comment and the system setting. Text search still finds the passage, a new workspace can be written, and opening the folder again changes nothing.
- **Released migrations are locked.** `crates/storage/migrations.lock` holds the checksum of every database migration that has shipped, and `npm run migrations:check` (in CI) fails if one is edited, removed, renumbered or left out of the lock. The server refuses an environment whose recorded checksum differs from its file, so a change belongs in a new migration.

### Upgrading
- Nothing to do. If you build the server yourself, build from a checkout with LF line endings, as CI does: a build with other line ends computes other checksums and refuses an environment another build made. The test was compiled, not run here; its data was run through all 36 migrations in SQLite.

## V0.2.52 (2026-10-16)

Feature freeze, and the fourth security pass.

### Security
- **The fourth security pass** is in [mindmap/security/2026-10-16-security-review.md](mindmap/security/2026-10-16-security-review.md). It checked mail, webhooks, remote clones, connectors, export and import, and the desktop client against the requirements set before they were built. It found no high-severity problem; three medium and four low ones are fixed in this release, and five informational notes are recorded. No high or medium finding is open.
- **Mail cannot be used to flood a mailbox.** One address is sent at most 12 messages an hour (invitations, mentions, assignments, verification links), one person causes at most 60 an hour, and nothing is queued while 1,000 messages are waiting. An invitation over the limit is still created, with its link to pass on by hand.
- **Anonymous requests are small.** Sign-in, registration, refresh, password reset, invitations, first-run setup and the two webhook receivers now take 64 KiB (webhooks 1 MiB) instead of the upload limit, which an operator can raise to 1 GiB.
- **Imports cannot be used as a bomb.** An archive that claims to unpack to more than 100 times its own size (and over 64 MiB) is refused before anything is unpacked, each file is read only up to what the archive says it holds, and the total of the sizes it declares can no longer wrap around to a small number.
- **A webhook delivery is acted on once.** A delivery id (`X-GitHub-Delivery`, `X-Gitlab-Event-UUID`, or `X-RepoMemo-Delivery` for your own sender) received in the last hour is answered `202` with `already received`, so a captured call cannot be replayed to keep a repository syncing.
- Smaller: a tracker's answer is read only up to its 20 MiB cap, the desktop app follows no redirects away from its server, and `ssh` clones set `ProxyCommand=none`.

### Changed
- The feature freeze starts: until `V0.3.0`, releases carry only the release candidate's checks and their fixes.

### Upgrading
- Nothing to do. If a script posts large bodies to sign-in or the webhooks, it now gets `413`. Generic webhook senders should send a unique `X-RepoMemo-Delivery` header per call. The fixes were compiled and their tests written, not executed or attacked; run `cargo test` and try a flood of invitations before relying on them.

## V0.2.51 (2026-10-16)

The local desktop workspace is retired.

### Changed
- **The local workspace is labelled as retired.** In the desktop app it now sits under a standing notice: it stays available for now, receives security fixes only and will be removed in a 0.3.x release. The notice offers *Move a workspace to a server* and *Connect to a server*, and the first screen says the same next to *Use the local workspace*. Nothing is removed in this release.
- **The docs describe the desktop app as a window onto a server.** `PRODUCT.md`, the README and the mindmaps no longer present the local edition as a current mode of the product.
- **Safer Markdown in the retired edition.** AI answers, summaries and memory cards in the local workspace are shown with the same Markdown rules as the web client: no images are loaded, and links name their host and open in your system browser.

### Upgrading
- Nothing to do. Move any local workspace you still need to a server with *Move a workspace to a server* (V0.2.50) before the removal release.

## V0.2.50 (2026-10-16)

A workspace from the local desktop edition can be moved to a server.

### Added
- **Move a workspace to a server.** In the local workspace of the desktop app (bottom-left), choose a workspace and *Prepare the archive*. It is written as a workspace archive, the same file a server exports, so the server's import reads it as it reads any other. Then *Go to the server to import it* (connect and sign in), *Save a copy…* or *Discard the archive*.
- **The import is checked first.** On the server side of the app the dashboard's *Import a workspace* opens with the prepared archive already checked: it shows the files, folders, memory cards with their citations, tasks and comments it holds, and writes nothing until you confirm, under the name you give. The prepared file is deleted afterwards. Your local workspace is not changed or removed.
- The archive is complete or not there: it is written under another name and renamed when whole, and a workspace with a file whose stored content is missing is refused with the first file named.

### Not moved
- AI provider settings and keys, saved searches, assistant chats, the activity feed, earlier versions of a file and repository links, as in every archive. The server indexes the files again.

### Upgrading
- Nothing to do on servers: the import is the one from V0.2.41. Importing needs an organization owner or administrator. Prepared archives older than seven days are removed from the app's data folder when a new one is made.

## V0.2.49 (2026-10-16)

The desktop app opens a RepoMemo server.

### Added
- **The desktop app is a client of a server.** On first start it asks for the server's address and shows the same pages as the browser once you sign in. `https://` is assumed; plain `http://` is accepted only for a server on your own network (this computer, a private address, `.local` or a single-word name). The app keeps the address, and only a server that answered like RepoMemo is kept.
- **Your session is kept in the system credential store** (Windows Credential Manager, macOS Keychain, the Secret Service on Linux) and nowhere else, so restarting the app does not ask for the password again. Signing out ends the session on the server and deletes the entry.
- **The app can only go where it should.** Its window can show only the bundled app, so a link or a script cannot take it to another site, and it may call only the server you chose. Links to the outside open in your system browser.
- **Change server** (in the left rail, and on the sign-in page) forgets the server and the session and restarts the app.
- **The local workspace is still there**, behind *Use the local workspace* on the first screen, until it is retired. Moving it to a server comes next.

### Changed
- In the desktop app, *Open in Word* and *Open in PDF viewer* save the file instead of opening a browser tab. The desktop app is listed as a `desktop` client under System › Overview.

### Upgrading
- Servers need nothing: they already return the refresh token in the response to clients that are not the web app.
- Developers: `cargo check -p repomemo-desktop` pulls `tauri-plugin-http` and `keyring`; `npm install` adds `@tauri-apps/plugin-http`. The app was not started in this release: try it against `cargo run -p repomemo-server`.

## V0.2.48 (2026-10-16)

The web client works on a tablet and a phone.

### Fixed
- **The page could not scroll on a narrow screen.** The page was clipped to the window at every width, which suits the wide layout (each pane scrolls on its own) but left the stacked tablet layout unreachable below the first screen. Below 900 px the page now scrolls, and only sideways overflow is cut.

### Added
- **A layout for narrow screens** (below 900 px, and a phone layout below 640 px, checked down to 375 px by reading the styles): grids fall to one column, long names and paths wrap, tables, the knowledge map and the contribution calendar scroll inside their own box, and controls are at least 40 px high on a phone.
- **The side rail folds** behind a *Workspaces and organizations* button on narrow screens, so each page starts at its top.
- **The assistant becomes a sheet** on narrow screens. It starts closed, a floating *Assistant* button opens it over the bottom of the page, and *Esc*, its close button or a tap outside closes it. The conversation is kept while it is closed. The wide-screen dock, its width and its open or shrunk state are unchanged.

### Changed
- On a phone the header keeps its buttons as icons (Profile and Sign out keep their names for screen readers), notifications sit above the floating assistant button, and dialogs and the command palette use the window.

### Upgrading
- Nothing to do. The layout was written from the styles, not seen on a screen: look at a few pages at 375 px and 768 px (browser device mode) and report what overflows.

## V0.2.47 (2026-10-16)

The web client can be used by keyboard and screen reader.

### Added
- **A frame that can be navigated.** Every signed-in page starts with a *Skip to the page* link, then the header, a labelled side rail and one main area. The browser tab names the page (*Atlas · Evidence*, *System · Audit trail*).
- **Moving to another page is announced.** A screen reader hears the page's name and keyboard focus goes to the start of the page, so it is not left on a control that is gone. Changing a tab inside a workspace keeps focus on the tab, and the first page after signing in or reloading keeps the browser's starting point.
- **A check for the rules, in CI.** `npm run a11y:check` reads every component for images without `alt`, fields and buttons without a name, click handlers on elements that are not controls, `aria-label` where it is not read, table headers without `scope` and positive `tabIndex`, and measures the contrast of the colour pairs used for text in both themes (4.5:1) and of the focus ring (3:1). It reads files only.

### Changed
- **Every field has a label and every button a name.** The note title and text, memory title and text, memory and evidence search, retrieval question, saved-search name and checklist step fields had only a placeholder.
- **Charts say what they show.** The activity timeline lists its days and figures, bars and the indexing coverage are images with their values in the name, and legends are groups.
- **The sign-in / create account switch is a pair of toggle buttons**, not tabs that did not behave like tabs. The workbook sheet tabs now follow the tabs pattern: arrow keys, Home and End, one tab stop, a labelled panel, and `scope`d table headers.
- **Notifications wait for the keyboard too.** A notification stays while a control inside it has focus, and *Esc* dismisses it.
- **Less motion on request.** With the system setting for reduced motion, transitions and animations stop (the working spinner turns slowly), and in high-contrast modes the focus ring is the system's.
- **Readable greys and status colours.** In the light theme the muted and subtle text greys and the success and warning text colours were darkened, and in the dark theme the subtle grey was lightened and the primary button's hover darkened, so each reaches 4.5:1 on every surface it is used on.

### Upgrading
- Nothing to do. This release was checked by reading files; it was not run in a browser or with a screen reader, so an axe or Lighthouse run and a pass with NVDA or VoiceOver are still worth doing.

## V0.2.46 (2026-10-16)

The audit trail can be downloaded.

### Added
- **Export the audit trail** (System › Audit trail, full system administrators). Choose JSON lines or CSV and a range of days, or none for the whole trail, and download every event of it, oldest first: its id, time, who did it (id, address and name), the action and the detail. It is for an auditor, or for keeping the trail longer than the server does.
- **The download is itself audited.** An *audit exported* event, with the range and format, is recorded before the first byte is sent, so even a download that is cut short is on record. The file stops at the moment the export began, so it does not contain its own record; the next export does.
- **`GET /v1/system/audit/export?format=jsonl|csv&from=&to=`.** `from` and `to` are days (UTC, both included) or times such as `2026-10-15T09:30:00Z` (`to` excluded). The file is streamed in pages, so a long trail does not sit in memory, and `X-Total-Count` gives the number of events. A bad format or date, or a range that ends before it starts, is refused.
- **Spreadsheet-safe CSV.** A cell that starts with `=`, `+`, `-`, `@`, a tab or a return (event details can hold text people typed) is prefixed with an apostrophe so a spreadsheet shows it instead of running it. Cells with commas, quotes or line breaks are quoted.

### Upgrading
- Nothing to do. The trail is still trimmed by `REPOMEMO_SYSTEM_AUDIT_RETENTION_DAYS`; export before it ages out if you need it longer. The production guide shows a `curl` line for a scheduled export with a `system`-scope token.

## V0.2.45 (2026-10-16)

RepoMemo can be run with `docker compose up`.

### Added
- **A `Dockerfile` with two images.** `server` is `repomemo-server` on Debian slim with `git` and CA certificates, running unprivileged with its data folder in a volume and a health check on `/health/ready`. `proxy` is Caddy with the web app built into it. LibreOffice for layout-accurate Office previews is left out to keep the image small; `--build-arg WITH_LIBREOFFICE=true` adds it.
- **`docker-compose.yml`, `docker/Caddyfile` and `.env.example`.** Copy `.env.example` to `.env`, set `REPOMEMO_HOST` and the two required secrets, and run `docker compose up -d --build`. The server is reachable only through the proxy, which gets the certificate (Let's Encrypt for a real name, its own authority for `localhost`), serves the web app with its content security policy, sends `/v1` and `/health` to the server without buffering the live event streams, and is the only address the server trusts for client addresses. Backups run every 24 hours inside the volume.
- **The web app can be built for the page's own host.** An empty `VITE_REPOMEMO_API_URL` now means "the same host" instead of falling back to `127.0.0.1:3020`, so one image works under any name. Unset still means `http://127.0.0.1:3020`.
- **`npm run container:check`** (also run by CI) compares the container files with each other and with the variables the server reads. It starts nothing and needs no Docker.
- The production guide has a new section, *In containers*: the volumes, backups with a second copy on a mount, repositories on disk, upgrading and using your own proxy.

### Upgrading
- Nothing to do for a server that is not run in containers. To move to containers, back up, restore the backup into the new environment (production guide §6) or mount the old data folder as the volume, and keep the same `REPOMEMO_JWT_SECRET` and `REPOMEMO_SECRET_KEY`.
- The images were not built or started when this release was cut; the first `docker compose up` is yours to try.

## V0.2.44 (2026-10-16)

A document that cannot be read no longer costs the server anything.

### Fixed
- **Documents are read in a process of their own.** Word, Excel, PowerPoint, PDF, OneNote and email files are now read by a child process (the server started with `--document-reader`). A file that takes too long is **killed** at `REPOMEMO_DOCUMENT_TIMEOUT_SECONDS` instead of being left running, and one that needs too much memory stops at the new `REPOMEMO_DOCUMENT_MEMORY_MB` (default 1024; `0` for no limit). A parser that crashes or overflows its stack ends the child only: the file is reported as possibly damaged and the server carries on.
- The child starts with an empty environment, so even a parser bug has no key or token to read, and it is given the file name only, never a folder of the server.
- Previews report the same reasons ("took longer than 60 seconds", "needed more than 1024 MB of memory") as indexing does.

### Added
- `REPOMEMO_DOCUMENT_READER` (`process`, the default, or `thread` for the earlier behaviour as a stopgap), `REPOMEMO_DOCUMENT_MEMORY_MB`, a *Document readings* line in System › Overview and the counter `repomemo_document_readings_killed_total`.

### Security
- Closes the accepted review finding **R2-4** (an overrunning reading was abandoned, not killed, and had no memory limit). A stack overflow or out-of-memory abort in a document parser can no longer stop the whole server.

### Upgrading
- Nothing to do. Reading a file now starts a short-lived process, which takes a few milliseconds. If readings fail with "could not be started", the server could not start its own program file: check where it runs from, or set `REPOMEMO_DOCUMENT_READER=thread` until it is fixed.

## V0.2.43 (2026-10-16)

Each backup can be copied somewhere else, and the copy is checked.

### Added
- **`REPOMEMO_BACKUP_COPY_DIR`.** Set it to a folder on another disk or a mounted network share and the server copies every backup there once it is made. The copy is written as `<name>.partial` and renamed only when it is whole.
- **The copy is verified.** Every file is hashed as it is read, written and read back from the destination; the hashes are saved as `SHA256SUMS` (`sha256sum -c SHA256SUMS` repeats the check), and the number of files and their total size must equal the backup's manifest. A copy that does not add up is removed and reported.
- **Unchanged files are not sent again.** A file whose hash matches the previous copy is linked to it, so the second location grows like the first. It keeps the same number of backups (`REPOMEMO_BACKUP_KEEP`). A sealed backup is copied sealed.
- **Failures are seen and retried.** If the share is down, the backup is kept, the job shows as failed with the reason, and the copy is tried again on every maintenance pass. System › Backups shows the second location, when each backup was checked there (or *waiting*) and a *Copy to the second location now* button (`POST /v1/system/backups/copy`). The new metric `repomemo_backup_copy_last_timestamp_seconds` can be alerted on, and the setup checks have a *Second copy of backups* line.
- Backup manifests now record `files` and `bytes`.

### Upgrading
- Nothing to do. Without `REPOMEMO_BACKUP_COPY_DIR` nothing changes. Mount the share before the server starts and give the account that runs the server write access; the folder must not be inside the backups folder. Backups made before this version have no totals in their manifest and are checked file by file only.

## V0.2.42 (2026-10-16)

Backups can be encrypted.

### Added
- **`REPOMEMO_BACKUP_KEY`** (16 or more characters). When it is set, every file of every new backup is sealed with AES-256-GCM before it is written: the database, the key file and each stored file (`*.enc`). Only `manifest.json` stays readable, and it holds nothing that can open the backup. Off by default.
- **A wrong key is refused before anything is written.** Restoring a sealed backup needs the same `REPOMEMO_BACKUP_KEY`. The server checks it against the manifest first, and says so plainly when it does not match or is not set; a file that was changed, moved or cut fails to open instead of restoring wrong data.
- **Backups stay small.** Unchanged files are still shared between backups made with the same key (a new backup keeps the salt of the one before it). A sealed file is 16 bytes larger per 64 KiB.
- **Visible in System.** Backups shows whether encryption is on and tags each sealed backup; `GET /v1/system/backups` returns `encryption_enabled` and each backup's `encryption`; the setup checks have a *Backup encryption* line that suggests the key until it is set.

### Security
- Closes the accepted review finding **R2-5** (backups were not encrypted and held `secret.key`).

### Upgrading
- Nothing to do: without the key, backups are made as before. To turn it on, set `REPOMEMO_BACKUP_KEY` and restart; the next backup is sealed, older ones stay plain until they are deleted.
- **Keep the passphrase apart from the backups, in a password manager.** It is stored nowhere in them, and without it a sealed backup cannot be restored. Changing it seals new backups only; keep the old one for as long as you keep the old backups.

## V0.2.41 (2026-10-16)

A workspace archive can be imported as a new workspace.

### Added
- **Import a workspace** on the dashboard, for owners and administrators of an organization. Choose an archive made by *Export this workspace*, **check it first** (it is read completely and nothing is written), see what it holds and any warning, name the new workspace and import it. You become its owner.
- **The check** verifies the manifest and format version, the data against its checksum, every stored file against its hash and size, and that the records agree with each other (references, folder tree, states). It also tells you if the archive is too large for the server or for a workspace's storage allowance. A damaged archive, one from a newer RepoMemo or one whose records disagree is refused with the reason, and a failed import leaves no workspace behind.
- **Everything is linked as before:** folders, each file's lifecycle and history, memory cards with their citations, tasks with checklists, comments and the workspace's AI policy, with their dates. A cited passage is found again once its file is indexed; if it cannot be found, the citation points at the whole file and the report says how many. Searching works as soon as a file is indexed, in the background.
- **Nobody is credited by account.** An archive can be written by anyone, so its authors are not believed: comments, tasks and history belong to the person who imports, and the original name is kept in the text. Links to a folder, repository or tracker are not recreated; their files arrive as plain evidence.
- **`POST /v1/workspaces/import?organization_id=&dry_run=&name=`** takes the archive as the request body. It is received into a temporary file in the data folder (not into memory) and is not subject to the request timeout.
- **`REPOMEMO_MAX_IMPORT_MB`** (default 2048) is the largest archive accepted.

### Upgrading
- Nothing to do. If a reverse proxy sits in front of the server, allow `POST /v1/workspaces/import` a body as large as your biggest archive and do not buffer it (the production guide shows the nginx location).
- Importing makes a copy; importing the same archive twice makes two workspaces.

## V0.2.40 (2026-10-16)

A workspace can be exported as one file.

### Added
- **Export this workspace** (Settings, for owners and administrators). It downloads a compressed archive (`repomemo-<name>-<date>.tar.gz`, readable with any archive tool) holding a `manifest.json`, a `workspace.json` and the original files.
- **What is in it:** the evidence with its original files (a file stored once is in the archive once), folders, each file's lifecycle state, owner, review note and history, memory cards with their citations, tasks with their checklists, comments, and the workspace's own settings. The manifest lists the counts, every file's checksum and a checksum of the data.
- **Citations travel with the card.** A citation names its file and, for a passage, its place and a fingerprint of its text, so the passage can be found again after the file is indexed on another server. Citations whose passage or file no longer exists are left out and counted in the manifest.
- **People are a name and an address,** never an account of this server. Anonymized accounts stay "Former member" with no address.
- **Nothing secret leaves.** AI provider keys, repository and tracker tokens and webhook secrets are not in the archive, and links to a repository or tracker are not carried (their files arrive as plain evidence).
- **`GET /v1/workspaces/{id}/export`** streams the archive while it is built, so a large workspace does not fill the server's memory. A workspace with a stored file missing from the server's disk is refused with a plain message (`409`) instead of being exported incomplete. The export is recorded in the workspace's activity (`workspace_exported`).

### Upgrading
- Nothing to do. Importing an archive into a new workspace comes in V0.2.41; earlier versions of a file, saved searches, assistant chats and the activity feed are not part of the archive.

## V0.2.39 (2026-10-15)

Mentions are suggested as you type them.

### Added
- **`@` suggestions in comments.** Typing `@` in the discussion of a file offers the workspace's members; typing more narrows them (by the start of a name, of one of its words or of the address, then anywhere in either). Arrow keys move, Enter or Tab chooses, Esc closes. Choosing one writes their address after the `@`, which is what a mention has always been, so they are notified when the comment is saved.
- **Only members of the workspace are suggested,** and only to people who can comment (members, administrators and owners; viewers see no suggestions). Yourself, people whose account was deactivated and anonymized accounts are left out, and at most eight are offered at a time.
- **`GET /v1/workspaces/{id}/mention-suggestions?q=`** answers with `{id, display_name, email}` for each.

### Upgrading
- Nothing to do. Comments written with a typed `@address` work as before.

## V0.2.38 (2026-10-15)

Earlier versions of a file stay readable.

### Added
- **Version history.** Uploading a new version of a file (the *New version* action on its page) keeps the content it replaces. A file with more than one version shows a *Version history* panel: each version with who uploaded it, when it became current and its size. The first upload's author is not recorded.
- **Read any version.** *View text* shows what a version said, as search reads it: the file's own text, or the text extracted from a Word, Excel, PowerPoint, PDF, OneNote or Outlook file. An image or a binary file says it has no text to show.
- **Compare versions.** *What changed* compares a version with the one before it, and *Compare with current* compares an earlier one with the latest. The comparison is line by line, with removed and added lines marked and a few unchanged lines around each change; long unchanged stretches are folded. Only the first 4,000 lines of each version are compared.
- **API:** `GET /v1/artifacts/{id}/versions` (the list, newest first), `GET /v1/artifacts/{id}/versions/{n}` (the text of version n), `GET /v1/artifacts/{id}/compare?from=&to=` (members of the workspace; the comparison counts as a heavy request).
- Migration 0036 (`artifact_versions`).

### Changed
- Earlier versions' stored content is no longer collected as unreferenced: it is kept for as long as the file exists, and counts in the workspace's storage use. Deleting the file lets it go.

### Upgrading
- Nothing to do. Files uploaded as new versions before this release have no history: their next new version starts it (the version before it is kept from then on).

## V0.2.37 (2026-10-15)

One list of what waits for a person.

### Added
- **Review queue:** a new *Review* section in every workspace lists, in one place, the files marked *needs review*, the memory cards whose evidence changed, disappeared or was retired, and the open health findings. Each item says why it is waiting and since when, and opens where it is resolved: the file's page, the card's page or Health. The oldest wait first; findings, which have no date, follow.
- **It empties as things are resolved:** the list is computed from those places each time, so marking a file verified or active again, confirming or editing the card, or acting on or dismissing the finding takes it off. It refreshes by itself when workspace activity changes it.
- **`GET /v1/workspaces/{id}/review`** (members of the workspace): `{items, evidence_count, memory_card_count, finding_count}`. Each item has a `kind` (`evidence`, `memory_card`, `health_finding`), a title, the reason, where it lives, the file to open, when it started waiting and who can resolve it (`writer` or `admin`).
- A stale memory card is listed once, as the card, and not again as a *Stale memory* finding. Findings a person has dismissed or acted on stay out, as on the Health page.
- The command palette lists the new section.

### Upgrading
- Nothing to do. No migration; nothing is stored for the queue.

## V0.2.36 (2026-10-15)

C# and PHP are read like the other languages.

### Added
- **C# and PHP files are split along their declarations.** A class, struct, record, interface, enum, function or method is not cut in half; a large class is split by method, and a namespace that holds one large class is split inside it. Each passage records its scope in its heading (`App > Store`).
- **Their symbols are searchable.** C# classes, structs, records, interfaces, enums, methods and constructors; PHP classes, interfaces, traits, enums, functions and methods. They appear in *Code symbols* in Retrieval, on the file's page and in the assistant's *Find a symbol*.
- **`.cs` and `.php` files can be uploaded** and are indexed as C# and PHP. PHP files are read with the grammar that also understands the HTML around `<?php … ?>`.

### Changed
- Namespaces, classes and other containers that hold a single, large member are now split along that member too (before, such a container was cut by size). Every language benefits.
- The indexer version is 4, so every file already indexed is indexed once more in the background after the upgrade. Passages whose text is unchanged keep their identity, their embeddings and the links that point at them.

### Upgrading
- Nothing to do. C# and PHP files already in a workspace through a repository are re-indexed in the background and gain their symbols.

## V0.2.35 (2026-10-15)

Go and Java are read like Rust and Python.

### Added
- **Go and Java files are split along their declarations.** A function, a method, a struct, an interface, a class or a record is not cut in half: small neighbours are grouped, a large class is split by method, and each passage records its scope in its heading (`Store.Get` for a Go method, `Store > lookupEntry` for a Java class). Comments stay attached to the declaration below them.
- **Their symbols are searchable.** Go functions, methods (named after their receiver type), structs and interfaces; Java classes, records, interfaces, annotation types, enums, methods and constructors. They appear in *Code symbols* in Retrieval, on the file's page and in the assistant's *Find a symbol*.
- **`.go` and `.java` files can be uploaded** (the file picker and folder uploads accept them) and are indexed as Go and Java. Repositories already read them as such.

### Changed
- The indexer version is 3, so every file already indexed is indexed once more in the background after the upgrade. Passages whose text is unchanged keep their identity, their embeddings and the links that point at them.

### Upgrading
- Nothing to do. Go and Java files already in a workspace through a repository are re-indexed in the background and gain their symbols.

## V0.2.34 (2026-10-15)

A keyboard command palette.

### Added
- **Command palette:** press **Ctrl K** (or **⌘ K**) on any signed-in page, or use the search button in the header. Type to jump to a workspace, one of the open workspace's sections (People and Workspace settings only for those who administer it), a file by its title or path, a memory card, the dashboard, notifications, your profile or, for system administrators, a System page. Arrow keys move, Enter runs, Esc closes.
- **Assistant actions from the palette:** on a workspace page the typed text can be sent as *Ask the assistant*, *Find files*, *Search content* or *Find the symbol*, or the assistant can be asked for a workspace overview. The assistant dock opens, runs the request in the current chat and shows the reply; with nothing typed the action is only selected, ready for the question.
- Files come from the open workspace's list (read when the palette opens) and memory cards from its search, both with your own access: nothing is listed that the page itself would not show you.

### Upgrading
- Nothing to do. No server change.

## V0.2.33 (2026-10-15)

Search reaches every workspace you can read.

### Added
- **Search all workspaces** on the dashboard: one box searches the evidence of every workspace the account can read. Each result names its workspace, shows the snippet with the matched words highlighted, and opens the file in its own workspace.
- **`POST /v1/search`** (`{query, workspace_ids?, lifecycle_statuses?, limit?}`; 20 results by default, at most 50). Each workspace is searched on its own: its role check is made for the caller, outdated and superseded evidence is left out unless asked for, and a workspace the caller cannot read is left out without being named, even when asked for.
- **Each workspace's AI policy decides whether the query is matched by meaning.** Where *AI features* are kept to other roles, that workspace is searched by keywords only and the query is never sent to its embedding provider; the answer says so for that workspace (`by_meaning`, `note`).
- **Merged by score.** A workspace's scores are relative to its own best match, so every workspace's best result ties at the top and the others follow by score, then by their rank in their workspace. At most 30 workspaces are read per search (`not_searched` counts the rest); one that cannot be searched is skipped with a note instead of failing the search.

### Upgrading
- Nothing to do. The search counts as one heavy request against the account's quota, not one request per workspace.

## V0.2.32 (2026-10-15)

The assistant remembers the conversation.

### Added
- **Earlier turns go with each question.** Inside a chat, the assistant is reminded of what was said before: the newest earlier turns that fit a budget of about 1,500 tokens (at most 8, each answer cut to 700 characters; a turn is kept whole or left out) are sent along with an *Ask a question* request. A follow-up such as *and what happens when it fails?* is searched together with the turn before it and the titles of the files it was about, so it finds what it refers to, and the model is shown the turns only to tell what "it" and "that file" mean, never as evidence: citations still point at the numbered passages.
- **"Summarize it" and "summarize that file"** summarize the single file the latest turn was about (a file that was found, summarized or, failing that, cited). When that turn listed several files nothing is guessed: the assistant asks which one.
- **Replies say what they read:** *read 2 earlier messages* appears in the reply's header when earlier turns went with the question (`context_turns` on the reply).
- The history is always taken from the stored conversation of the person asking. A `history` sent by a client is ignored, and a new chat starts with none.

### Upgrading
- Nothing to do. Chats that already exist are remembered from their stored turns. A follow-up that goes to the AI provider now carries up to about 1,500 more tokens of context, which counts as one request against the same hourly quota.

## V0.2.31 (2026-10-15)

Connectors follow their tracker within seconds, and behave when it pushes back.

### Added
- **Webhooks for connectors.** A workspace administrator sets one up from a connector's *Webhook* button in Settings › Issues and pull requests: the payload address and a secret are shown once. `POST /v1/webhooks/connectors/{id}` takes GitHub (`X-Hub-Signature-256`, events `issues`, `issue_comment`, `pull_request`, `pull_request_review`, `pull_request_review_comment`), GitLab (`X-Gitlab-Token`, *Issue*, *Note* and *Merge Request* hooks) and any sender that signs with HMAC-SHA256 (`X-RepoMemo-Signature`). The rules are the repository webhooks': constant-time checks, the same `401` for anything unverifiable, limits per address and per connector, one sync queued per call.
- **`Retry-After` is honoured.** A tracker that asks to wait up to 30 seconds is waited out inside the sync (twice at most). A longer wait (`Retry-After`, GitHub's `X-RateLimit-Reset`, GitLab's `RateLimit-Reset`) **pauses the connector** until then: it shows *Paused until …* with the tracker's reason, is not asked again before that (manual syncs answer `409`), and resumes by itself.
- **A refused token pauses the connector** and says so: *Paused until the token is replaced*. Background syncs stop; *Sync now* may still try once; replacing the token (tried against the tracker first) clears the pause.
- The repository webhooks now share this code and its limits; their behaviour is unchanged.

### Upgrading
- Nothing to do. The server must be reachable from the tracker for webhooks to arrive; polling stays on as a safety net.

## V0.2.30 (2026-10-15)

A file shows the issues and pull requests that name it.

### Added
- **Mentioned in / Mentions** on the page of a repository file, an issue and a pull request. A file lists the issues and pull requests whose text names it, one of its symbols, or the commit that last changed it; an issue or pull request lists the files it names.
- **What counts as a mention:** a **path** (`src/ledger.rs`, or `ledger.rs` when only one file in the workspace has that name), a **symbol in backticks** (`` `post_refund()` ``, `` `Ledger::post_refund` ``; one with more than three definitions is too common to link), and a **commit** (a hash of 7 characters or more, or a pull request's own commits) that is the last change of a file. A name that could mean several files is left alone: nothing is guessed.
- **Kept current:** mentions are recomputed when a connector syncs something and when a repository syncs, so a file added after the issue was written still gets linked. `GET /v1/artifacts/{id}/mentions` (members of the workspace) returns both directions.
- Migration 0035 (`artifact_mentions`).

### Upgrading
- Nothing to do. Links appear after the next sync of a connector that changed something or of a repository.

## V0.2.29 (2026-10-15)

GitLab joins GitHub.

### Added
- **GitLab projects** (gitlab.com or self-hosted) connect from the same place as GitHub repositories: Settings › Issues and pull requests, with the project's address (groups and subgroups included, such as `https://gitlab.example.com/platform/ledger/api`). The tracker is told by the address (a host with `gitlab` in its name is GitLab) or chosen in the new *Tracker* field.
- **The same model as GitHub:** each issue, with its notes, and each **merge request**, with its commits, inline review comments and discussion, is one searchable, citable artifact. System notes ("changed the description") are left out. Closed issues and closed merge requests are *outdated*, reopened ones active, merged ones stay active.
- **Incremental:** issues and merge requests each have their own `updated_after` cursor, oldest update first, with the same request budget per sync as GitHub.
- **The token goes in `PRIVATE-TOKEN`,** to the project's own host only; the host must be on `REPOMEMO_GIT_ALLOWED_HOSTS` like GitHub's. Routes accept an optional `provider` (`github` or `gitlab`).

### Upgrading
- Nothing to do. For GitLab, name its host in `REPOMEMO_GIT_ALLOWED_HOSTS`. A token with the `read_api` scope is enough.

## V0.2.28 (2026-10-15)

GitHub pull requests become evidence, next to the issues.

### Added
- **Pull requests are kept by the same connector** as the repository's issues (Settings › Issues and pull requests). Each one, with its description, **commits** (id, first line, author), **reviews** (reviewer, verdict, text), **inline review comments** (file and line) and conversation, is one artifact (type *pr*): searchable, citable and linkable to memory cards.
- **State:** an open pull request is active, a **merged** one stays active (it describes what is in the code), and one **closed without merging** is *outdated*, like a closed issue; a reopened one is active again. The connector counts a merged pull request with the closed items.
- **The commit ids are kept** in the item's metadata (`commits`), which is what V0.2.30 links to files and commits.
- **A request budget per sync (500).** A pull request takes five requests, so a large tracker is read over several syncs: the sync stops, says *more to fetch*, and resumes from where it stopped.
- Existing connectors read pull requests from the next sync on; the first one may need several syncs on a large tracker.

### Upgrading
- Nothing to do. A token that could read issues reads pull requests too (a fine-grained token also needs *Pull requests: read*).

## V0.2.27 (2026-10-15)

GitHub issues become evidence.

### Added
- **Settings › Issues**: connect a GitHub repository's issues (github.com or GitHub Enterprise Server) to a workspace, with an optional read-only token (needed for a private repository or a higher rate limit). *Check access* shows the repository and its open issues before anything is stored.
- **Each issue, with its comments, is one artifact** (type *issue*): title, state, author, labels, assignees, link, body and discussion as markdown. It is indexed like any file, so it is searchable, citable in Ask answers and linkable to memory cards.
- **Incremental syncs:** each sync asks the tracker only for what changed since the last one, oldest first, so an interrupted sync resumes. It runs when connected, on *Sync now*, and on the repository polling interval. Pull requests listed among the issues are left alone for now.
- **Closed issues are *outdated*;** a reopened one is active again; a *verified* issue that changed is flagged *needs review*.
- Routes `GET/POST /v1/workspaces/{id}/connectors`, `POST …/connectors/check`, `GET/DELETE /v1/connectors/{id}`, `POST …/sync`, `PUT …/token`. The token is stored sealed with the repository tokens and never returned.
- **Outside rules:** the API host must be on `REPOMEMO_GIT_ALLOWED_HOSTS` (add `api.github.com`, or the Enterprise host; unset, connectors are off), `https` only (plain `http` for this machine only), redirects and next-page links to other hosts are not followed, answers are limited in size and time.
- Migration 0034 (`connector_items`).

### Upgrading
- Nothing to do. To use connectors, add `api.github.com` (or the Enterprise host) to `REPOMEMO_GIT_ALLOWED_HOSTS`.

## V0.2.26 (2026-10-15)

A citation can open the text it was made from.

### Added
- **View at commit.** A citation of a repository file names the commit its text is from. Next to it, in Ask answers, AI overviews and the assistant's evidence, *View at 1a2b3c4* opens the file as it was at that commit, labelled with the commit, its summary, author and date, and says whether it is **not the current version** (and which commit the workspace holds now). The cited lines are marked.
- **`GET /v1/artifacts/{id}/at/{commit}`** (any member of the workspace) returns that text with the commit's details. It reads the server's copy of the repository (the folder, or the partial clone of a remote one) with `git cat-file`: the commit must be a hex id of 7 to 64 characters, files over 1 MiB and files that are not text are refused, and a commit that is no longer in the copy (a force-push) or a file over the clone's size limit says so.

### Upgrading
- Nothing to do.

## V0.2.25 (2026-10-15)

A repository's `CODEOWNERS` shows on its files.

### Added
- **Owner hints from `CODEOWNERS`.** Each sync reads the repository's `CODEOWNERS` (`.github/CODEOWNERS`, `CODEOWNERS`, `docs/CODEOWNERS` or `.gitlab/CODEOWNERS`, the first found) and shows the owners it names for a file on the file's page: *CODEOWNERS suggests @payments, lead@example.com*, next to where the review owner is set. It is a hint only: the handles are names on the host, not people of the workspace, and nothing is assigned or notified from it.
- **Matching follows what GitHub and GitLab document:** the last matching pattern wins; `*` and `?` stay inside one path segment and `**` crosses them; a pattern with a `/` in front or in the middle is anchored to the root and one without matches at any depth; a pattern ending in `/` (or in a plain name) covers the directory it names; a pattern with no owners clears ownership; GitLab `[Section]` lines and their default owners are understood. Negation and escapes are not.
- Files whose owners change are updated on the next sync, including when `CODEOWNERS` is edited or removed. Files over 3 MiB of `CODEOWNERS` are ignored.

### Upgrading
- Nothing to do. Owners appear after the next sync of each repository.

## V0.2.24 (2026-10-15)

Who last changed a file, and when.

### Added
- **Last author and date for every repository file**, taken from the repository's history. They show on the file's page (*last changed by Ana · 3 Mar 2026 in 1a2b3c4*), in the repository's file list and under each search result that comes from a repository file.
- **One `git log` pass per sync** over the latest 20,000 commits finds the newest commit that touched each path; it also corrects the author after a revert, which changes the author without changing the file. Files untouched for more than 20,000 commits show no author. Renames count as a change of the new path.
- The author is the commit's *author* (not the committer) and is kept with the file (migration 0033: `last_author`, `last_authored_at`, `last_change_commit` on `repo_files`, and in the artifact's metadata). It is filled in by the first sync after upgrading.

### Upgrading
- Nothing to do. Run a sync (or wait for the next poll) to fill in authors for repositories linked before.

## V0.2.23 (2026-10-15)

A push can ask for a sync at once.

### Added
- **Push webhooks** for any linked repository (local or remote). A workspace administrator sets one up in Settings › Repositories › Edit › *Push webhooks*; the payload address and a secret are shown once (*Make a new secret* retires the old one, *Turn off* removes it). Hosts call `POST /v1/webhooks/repositories/{id}` when someone pushes, and the repository is synced within seconds instead of at the next poll.
- **Three senders are understood:** GitHub (`X-Hub-Signature-256`, an HMAC-SHA256 of the body; `ping` is answered, `push` syncs), GitLab (`X-Gitlab-Token`, the secret itself, with *Push Hook* events) and any other sender that signs the body with HMAC-SHA256 in `X-RepoMemo-Signature: sha256=<hex>`.
- **What makes a call count:** it must be signed right. Every comparison is constant time; a call that is unsigned, signed wrongly or for another body, or for a repository that does not exist or has no webhook, is answered `401` the same way. Failed calls are limited per client address (20 a minute) and verified ones per repository (60 a minute). Pushes to tags, to a branch the link does not follow, and events that are not pushes are accepted and ignored. One push queues one sync; a sync already running absorbs it.
- The repository shows whether a webhook is on and when a signed call last arrived. Polling stays on as a safety net.
- The secret is stored sealed like provider keys (migration 0032, `repo_webhooks`) and is never returned again.

### Upgrading
- Nothing to do. The server must be reachable from the host that sends the webhooks (`REPOMEMO_PUBLIC_URL` gives the full payload address).

## V0.2.22 (2026-10-15)

A remote clone can no longer take unbounded room or time.

### Added
- **A size cap and a time cap on every clone and fetch of a remote repository.** The size of the server's copy is measured twice a second while git runs; a transfer that goes over the cap, or past the time limit, is stopped at once and the person is told which limit it hit. A stopped clone is removed; a stopped fetch leaves the previous copy in place and removes its half-written packs.
- **Two settings** in System › Settings › Limits, changeable while the server runs: *Repository clone size* (`REPOMEMO_REPO_CLONE_MAX_MB`, 2048 MiB by default, `0` for no cap) and *Repository clone time* (`REPOMEMO_REPO_CLONE_TIMEOUT_MINUTES`, 30 by default). Before, the time cap was a fixed 30 minutes and there was no size cap.
- **The copies count in the workspace's storage allowance** like its files do: uploads and new notes are refused when files and copies together would go over, and a sync is not started once they are at the allowance.
- **System › Overview › Storage** shows how many remote copies the server keeps, their total size, the cap each one stops at, and the five largest.

### Upgrading
- Nothing to do. A workspace with large remote repositories may now be at its allowance: raise *Storage per workspace* or the repositories' branch rules, or remove a link (its copy is deleted with it).

## V0.2.21 (2026-10-15)

Link a repository by its `ssh` address, with a deploy key.

### Added
- **Repository links accept `ssh://git@host/owner/repo.git` and `git@host:owner/repo.git`**, on a host listed in `REPOMEMO_GIT_ALLOWED_HOSTS` (`host:port` for a port other than 22). The link needs a **deploy key** (a private key without a passphrase, read-only on the host) and the **host's public key**, as printed by `ssh-keyscan -t ed25519 host`.
- **The host key is pinned:** the connection only goes through to a host that presents exactly the pasted key. An unknown or changed key is refused, with a message that says so; nothing is trusted on first use, and the server's own `known_hosts` and `ssh` configuration are not read.
- **The deploy key never reaches a command line or the log:** it is stored sealed (like tokens), written to a private file that exists only for the length of one git command and is then removed, and passed to `ssh` as `-i`. `ssh` runs with the agent, passwords, forwarding and the user's configuration off. The host key is public and kept with the link.
- `PUT /v1/repositories/{id}/deploy-key` replaces the key (and, when given, the pinned host key); the new key is tried against the host before it replaces the old one. A key cannot be removed from an `ssh` link, only replaced. The change is recorded in the workspace activity.
- Clone, fetch, polling for new commits and everything after them work as for `https`.

### Upgrading
- Nothing to do. `ssh` links need an `ssh` client on the server's `PATH` (it comes with git on Windows and with OpenSSH elsewhere).

## V0.2.20 (2026-10-15)

Link a repository by its `https` address.

### Added
- **Repository links accept an `https` address** (such as `https://github.com/acme/app.git`) next to a folder on the server, in Settings › Repositories and in `POST /v1/workspaces/{id}/repositories` and `/check`. The host must be on `REPOMEMO_GIT_ALLOWED_HOSTS` (V0.2.19). *Check access* asks the host once (`git ls-remote`) and shows the branch and commit; nothing is cloned until the first sync.
- **An optional read-only token** for a private repository (`token`, and `token_username` when the host wants an account name). It is stored sealed, bound to its link, and never returned: the repository only shows *with a token* or *no token*. `PUT` and `DELETE /v1/repositories/{id}/token` replace or remove it; a new token is tried against the host before it replaces the old one. Both are recorded in the workspace activity.
- **How it is fetched:** the server keeps a bare, partial clone of the branch under `remote-repos/` in its data folder (blobs over 1 MiB, which are never indexed, stay on the host). git runs with an empty environment, `https` only, no redirects, object checks on, no submodules and no hooks; the token reaches it through the server's own program as `GIT_ASKPASS`, never in an address or an argument, and git's messages are shown with it removed. Fetching, sync, renames, overview, indexing and polling for new commits (one `ls-remote`, no fetch) work as for a local repository.
- **git must be patched** (2.50.1 or later, or a fixed maintenance release) for the CVE-2025-48384 submodule flaw; an older git is refused with that message.
- Removing a link removes the server's copy.

### Not yet
- `ssh` addresses are refused with a message: deploy keys come in V0.2.21. Clone size and time limits are V0.2.22; until then a transfer is stopped after 30 minutes.

### Upgrading
- Nothing to do. Remote links need `REPOMEMO_GIT_ALLOWED_HOSTS`. The server's own program doubles as git's credential helper, so no extra file is installed.

## V0.2.19 (2026-10-15)

Groundwork for linking remote repositories. Nothing is linked yet: that is V0.2.20.

### Added
- **`REPOMEMO_GIT_ALLOWED_HOSTS`**: the hosts remote repository links may use, comma separated (`github.com`, `git.example.com:2222`, `*.corp.example`, or `*`). **Unset turns remote links off**, like `REPOMEMO_REPO_ROOTS` does for local ones. It shows in the server checks (*Remote repository hosts*) and in System › Overview.
- **One address check, before any network call:** only `https` and `ssh` (including the `git@host:owner/repo` form); no credentials in the address (a token or key is entered separately); no `git://`, `http://`, `file://`, local paths, git transports such as `ext::`, options, whitespace or control characters; a valid host that is on the list. Link-local and cloud-metadata addresses are never allowed, and a private address must be named in the list. A refusal says which hosts the server allows, whether or not the address exists.
- **A credential store for each repository link**, for a read-only token (`https`) or a deploy key (`ssh`): sealed with the same key as AI provider keys, bound to its link (a sealed value copied onto another link does not open), removed with the link. It is never returned by the API or written to the log, and the opened value cannot be printed by accident.
- Migration 0031 (`repo_credentials`).

### Upgrading
- Nothing to do. Remote links stay off until `REPOMEMO_GIT_ALLOWED_HOSTS` is set. git resolves host names itself, so for names that point at private addresses the list is advisory: enforce it with an egress rule or a proxy on the server.

## V0.2.18 (2026-10-15)

Open registration no longer reveals who has an account.

### Fixed
- **Creating an account from the sign-up form answers `202` with the same words for a new address and for one that already has an account**, when registration is open, mail is set up and `REPOMEMO_PUBLIC_URL` is known. Before, a known address was refused with `409`, which told anyone which addresses had accounts (review M4, residual).
- **A new address** gets its account, created with the password just chosen, and a confirmation link by mail; the person then signs in (the sign-up form switches to sign in and shows the message). **A known address** is not touched: its owner gets a message that someone tried to register with it, at most twice an hour, with a link to sign in or to *Forgot your password?*. Deactivated accounts get no message.
- The password is hashed in both cases, so the time taken is close; it is not made identical.
- Registrations with an invitation, the first account of a server, and servers where mail is not set up (or has no public address) are unchanged: they answer `201` with a session, and `409` for a known address, since nothing else can carry the difference.

### Upgrading
- Nothing to do. A script that registers accounts on an open server with mail on now gets `202` and no session: it signs in afterwards. Registration stays closed by default.

## V0.2.17 (2026-10-15)

Anonymizing a deactivated account.

### Added
- **System › Users › Anonymize account** for a deactivated account (system administrators), and `users anonymize <email> --yes` in the console; `POST /v1/system/users/{id}/anonymize` with `{confirm}`, the account's address typed to confirm. It cannot be undone, and is audited as `user_anonymized` by the account's id.
- **What goes, for good:** the address, the name, the password, the profile picture, every membership, the notices, notification preferences, the digest record, assistant conversations, the mail waiting for the address (sent ones lose the address and subject) and the invitations made out to it. Tasks assigned to it become unassigned.
- **What stays:** everything it wrote (comments, tasks, notes, lifecycle changes), credited to **Former member**. The account row stays, so nothing that points at it blocks anything (risk 12 closed). Its address becomes an unreachable placeholder.
- **Summaries and audit details** that name the address, or the name on the account's own actions, are rewritten (*a former member*, *Former member*).
- An anonymized account is shown as *Anonymized* in System › Users and cannot be reactivated.
- Migration 0030 (`users.anonymized_at`).

### Upgrading
- Nothing to do.
- **What anonymizing does not reach:** log files and backups made earlier keep the address, as do other people's words and documents that mention the person. Log files age out with their retention setting.

## V0.2.16 (2026-10-15)

A deactivated account cannot sign in or use a token, and comes back when reactivated.

### Added
- **System › Users › Deactivate account / Reactivate account** (system administrators), and the console commands `users deactivate <email> --yes` and `users reactivate <email> --yes`; `POST /v1/system/users/{id}/deactivate` and `/reactivate` (a session or a `system` token). Both are audited as `user_deactivated` and `user_reactivated`.
- **What deactivating ends, in one step:** every session and refresh token, every personal access token, the open event streams of the account, its pending file links, the password reset and verification links waiting for it, and the invitations it sent. Access tokens already issued stop working at once.
- **What stays:** everything the account wrote and every membership. It shows in System › Users as *Deactivated*, with no active sessions.
- **Refused:** deactivating your own account, the last active system administrator, and an account that still owns an organization or a workspace (the answer names them: transfer the ownership first).
- **Sign-in says why**, but only after the right password was given: *This account has been deactivated*. A wrong password answers as for any account, so nothing is revealed about who has one. No password reset link is created or mailed for a deactivated account, and it gets no notification mail or digest.
- A deactivated account cannot be added to an organization or a workspace until it is reactivated.
- Migration 0029 (`users.deactivated_at`).

### Upgrading
- Nothing to do. Every account stays active. Reactivating does not bring back the sessions, tokens or invitations that ended.

## V0.2.15 (2026-10-14)

A daily digest by email.

### Added
- **Profile › Notifications › a summary by mail, once a day** (off until asked for; `daily_digest` with `mail` or `none`). A person who asked for it gets one message that counts the in-app notices they have not read since their last summary and lists the newest ten, with a link to the notifications page.
- **At most one a day, and only when something happened:** a person is not sent another within 24 hours of the last, and no message is made when nothing unread came after it (the next one then covers everything since the last summary). People whose address is unconfirmed get none.
- It goes through the outbox like every mail, so it needs mail set up and `REPOMEMO_PUBLIC_URL`. A person who wants the digest instead of single mails sets *a task is assigned* and *a mention* to *in the app only*.
- Migration 0028 (`mail_digests`: when each person was last sent one). Asking for the digest wakes the sender at once.

### Upgrading
- Nothing to do. Nobody gets a digest until they turn it on.

## V0.2.14 (2026-10-14)

Notification preferences.

### Added
- **Profile › Notifications:** for each kind of notice a person can choose *in the app and by mail* (the default), *in the app only*, *by mail only* or *nowhere*. The kinds that can be chosen are a task assigned to you and a mention in a comment; both the in-app notice and its mail honour the choice, decided when the event happens.
- **Security notices cannot be turned off:** password reset notices always reach the app and the mail (`PUT` for them is refused).
- `GET` and `PUT /v1/profile/notification-preferences`, and migration 0027 (`notification_preferences`; no row means both).
- Notification mails now point to the profile for the choice.

### Upgrading
- Nothing to do: everyone starts at *in the app and by mail*, which is what V0.2.12 did.

## V0.2.13 (2026-10-14)

Email verification.

### Added
- **Unverified accounts are held back.** When registration is open and mail is set up, an account made without an invitation can sign in but cannot create an organization or a workspace until it has used the link mailed to its address (`403` with the reason). The link works once and for 24 hours; the profile page can send another (three an hour). Registration stays closed by default, so this only applies once an operator opens it.
- **What proves an address:** a verification link, or accepting an invitation sent to it (an invitation counts as verification, also for people who register from one). The first account of a server is verified.
- **Changing the address** (Profile › Email address, `PUT /v1/profile/email`) asks for the current password, refuses an address another account uses, and starts the new address unverified with a link sent to it. A link made for the old address stops working.
- **Nothing is withheld without mail:** when mail is not set up nobody could verify, so accounts keep working as they did; the profile still shows whether the address is confirmed.
- The page `/verify-email#<token>`, `POST /v1/auth/verify-email`, `POST /v1/profile/email/verify`, and migration 0026 (`users.email_unverified_at`, `email_verifications`). Every account that exists stays verified.

### Upgrading
- Nothing to do. If you open registration and set up mail, people who register get a confirmation mail and are held back from creating things until they confirm.

## V0.2.12 (2026-10-14)

Mentions and task assignments are mailed.

### Added
- **A mail with each notice that asks for attention:** being assigned a task and being mentioned (`@email`) in an evidence comment. The message says who did it, what it is about and carries a link to the task board or the file. It goes through the outbox, only when mail is set up and `REPOMEMO_PUBLIC_URL` is known.
- **Once per event, never to the person who caused it:** a notice and its mail are made in the same step, so changing a task without changing its assignee sends nothing, and assigning a task to yourself or mentioning yourself sends nothing. Titles are cleaned for the subject, so a crafted title adds no header.
- The in-app notices are unchanged and still the record; the mail says so. Choosing, per kind, what arrives where comes with V0.2.14.

### Upgrading
- Nothing to do. People in workspaces start receiving these mails as soon as mail is switched on.

## V0.2.11 (2026-10-14)

"Forgot your password?" by email.

### Added
- **Get a link by email** on the sign-in page, and `POST /v1/auth/forgot-password {email}`. The answer is the same for every address: the same status (`202`) and the same words whether the address has an account or not, whether mail is set up or not, and whether the address's limit was reached; the work happens after the answer, so the time it takes says nothing either. Only the sign-in limits of the client address can refuse (`429`).
- **When the address belongs to an account and mail is set up**, a link goes through the outbox: one-time, valid for **1 hour** (administrators' links stay at 24 hours), and using it ends every session of the account, lifts lockouts and revokes personal access tokens, as every reset link does. An earlier unused link of the account stops working. The account also gets an in-app notification (`password_reset_issued`).
- **Limits:** at most three mails an hour per address (past it, nothing is sent and the answer does not change), on top of the per-client-address sign-in limits.

### Upgrading
- Nothing to do. Without mail set up (System › Settings › Mail) and `REPOMEMO_PUBLIC_URL`, nothing is sent and the page still says an administrator can issue a link in System › Users.

## V0.2.10 (2026-10-14)

Invitations are sent by email.

### Changed
- **Creating an invitation mails it.** When mail is set up (V0.2.9) and the server knows its public address (`REPOMEMO_PUBLIC_URL`), the invited address receives a message that names who invited them, where to, in which role, the link and the date it expires. It goes through the outbox, so a slow mail server never holds the request. The link is still returned and shown to copy: the answer's new `mail` field says what became of the message (`queued`, `not_set_up`, `no_public_address`, `failed`), and the page says so.
- **A crafted name adds nothing to the mail.** The inviter's and the workspace's or organization's names are cleaned for the subject (control characters, such as a line break, become spaces; long names are cut), and the message builder refuses any header value with a control character anyway.
- The outbox listing never shows a body, and a sent message's body is emptied, so a link in a mail is not kept.

### Upgrading
- Nothing to do. Without `REPOMEMO_PUBLIC_URL` no link can be put in a mail: set it (the production guide already asks for it).

## V0.2.9 (2026-10-14)

Mail delivery: the groundwork every mail feature after it stands on.

### Added
- **System › Settings › Mail** (system administrators; app administrators read): the mail server, port, connection security (TLS from the first byte, or STARTTLS), user name, password, sender address and name, and a switch. The password is stored sealed like AI provider keys and is never returned: the page only knows that one is saved, and leaving the field empty keeps it. Saving is audited (`mail_settings_changed`, without the password) and needs a signed-in session or a `system` token.
- **An outbox with retries.** Features put messages in `mail_outbox` and return; one background task hands due messages to the server. A temporary refusal or a network problem is tried again after 1, 5 and 30 minutes, 2 and 6 hours, then the message is marked failed with the reason; a permanent refusal fails at once. A message that was sent or given up on keeps no body (it can hold a one-time link), and finished messages are removed after 30 days. Messages a stopped server was sending are due again at the next start. A slow or dead mail server therefore never holds a request.
- **Send test mail** (page, `POST /v1/system/mail/test`, console `mail test <address> --yes`): queues a message to yourself or an address and answers at once; the result is on its outbox entry.
- **Console:** `mail status` (server, sender, outbox counts) and `mail test`.
- **The new `repomemo-mail` crate**: a plain-text message builder in which no header can carry a line break or other control character (a crafted name or subject is refused, not turned into a header), non-ASCII text is encoded, and an SMTP client that speaks TLS only. It checks the certificate against the public roots plus `REPOMEMO_SMTP_CA_FILE`, resolves the name itself and refuses link-local, cloud-metadata and private addresses unless the host is in `REPOMEMO_SMTP_ALLOWED_HOSTS`, and has a timeout at every step. A server that does not offer STARTTLS is refused rather than used in plain text, and the password is never sent before TLS.
- A server check *Outgoing mail* in the setup wizard, and migration 0025.

### Upgrading
- Mail is off until an administrator sets it up. Allow the server's outbound connection to the mail port; a mail server on a private address needs `REPOMEMO_SMTP_ALLOWED_HOSTS`.
- Nothing sends mail yet: invitations and reset links are still passed on by hand until V0.2.10 and V0.2.11.

## V0.2.8 (2026-10-14)

The knowledge map and Workspace Health recompute only what a change touched.

### Changed
- **A change to one file no longer recomputes the workspace.** The map and the health check used to read every vector of the workspace again whenever anything in it changed. They now keep, in memory, each file's passage counts and its direction in meaning space, and each comparison of files, and redo only the files written since: one edited file costs its own passages and one row of comparisons. The two share what they read, so opening the map warms the health check.
- At 50,000 passages (a debug build, so every figure is an upper bound): a full computation of the map and health took 5.0 s, the same after one file changed took 233 ms (4.6%), and asking again with nothing changed took 0.2 s. `cargo test -p repomemo-api --release map_and_health_at_scale -- --ignored --nocapture` now prints the third figure and fails when a one-file change costs a tenth or more of the full run.
- The answers are unchanged: a test compares the map and the health after a change with those of a cold computation. The similarity threshold is found by selection instead of sorting every pair.

### Added
- `StorageEngine::vector_stamps`, a journal of which file's passages or vectors were written, and targeted reads of passage counts and centroids for named files. Nothing is stored: after a restart, with another embedding model, or for a workspace whose centroids would take over 64 MiB, the first request simply does the whole work once.

### Upgrading
- Nothing to do. No migration.

## V0.2.7 (2026-10-14)

Workspace commands in the admin console.

### Added
- **`workspace reindex <workspace> --yes`**: re-indexes the files of a workspace that the current indexer has not indexed, as a background job (the same pass as `POST /v1/workspaces/{id}/index`, which answers `202` since V0.2.6). The dry run says how many files it would take.
- **`repo sync <repository> --yes`**: syncs a linked repository now, as a background job. The dry run shows its state and when it last synced; a repository that is syncing is a `409`.
- **`embeddings rebuild <workspace> --yes`**: removes a workspace's stored vectors, whatever model made them, and embeds every passage again with its embedding provider. The dry run reports the vectors it would remove, the passages, the model, the provider calls and whether that exceeds the workspace's daily allowance of background AI calls; a workspace without an embedding provider, or with a pass already running, is refused. Search by meaning is partial until the pass has run; keyword search is not affected.
- **`workspace list` and `repo list [--workspace w]`** to find what the commands above take. A workspace or repository is given by its id, or by its name when only one has it.
- Each of these is for **system administrators** (app administrators are refused), is a dry run without `--yes`, and leaves a `console_command` line with the exact command in the audit trail. They work from `repomemo-console` and from System › Console.

### Upgrading
- Nothing to do. `StorageEngine::clear_workspace_embeddings` and the core's `embedding_rebuild_plan`, `clear_embeddings` and `stale_artifact_count` back the new commands.

## V0.2.6 (2026-10-14)

Manual re-indexing no longer holds a request open.

### Changed
- **`POST /v1/artifacts/{id}/index` and `POST /v1/workspaces/{id}/index` answer `202 Accepted`** with the running job instead of waiting for the indexing to finish. The work continues in the background; the job's status, progress (files done out of the stale ones, for a workspace) and failure reason are on `GET /v1/jobs/{id}` and on the event streams, and the embedding and the activity entry follow when it completes. A workspace pass can be stopped with `POST /v1/jobs/{id}/cancel`.
- **One whole-workspace pass at a time:** starting a second while one runs answers `409` with the reason.
- The chunks dialog (workspace administrators, on a file's page) follows the job and shows its failure reason, so *Re-index* behaves as before from the person's side.

### Upgrading
- Scripts that call these two endpoints and read the job in the answer must now expect `status: "running"` and poll `GET /v1/jobs/{id}` until it is `completed`, `failed` or `cancelled`. The desktop app is unaffected.

## V0.2.5 (2026-10-14)

The stylesheets, split.

### Changed
- **`styles.css` (6,242 lines) and `blueprint-refinement.css` (1,220) are twelve files in `src/styles/`**, none over 900 lines, imported in their original order by `styles/index.ts` (`main.tsx` imports it). The order is the cascade: the files are successive design passes that override one another, so they are split where the passes and their sections already were (tokens and shared web mode, sidebar, workbench, forms, import and search, artifact detail, the three evidence-instrument blocks, the neutral research canvas, the quiet evidence desk, the blueprint refinement).
- **47 dead rules removed** (about 150 lines): rules whose classes appear nowhere in the web client, the Rust sources or the HTML (old list and member rows, the pipeline marks and recent lists of the retired local screens, a brand mark and a route mark). The content of every other rule and its position are unchanged, so nothing looks different.

### Added
- `npm run styles:check` (also in CI) fails when a stylesheet passes 1,000 lines or `styles/index.ts` does not import every file once, in order.

### Upgrading
- Nothing to do.

## V0.2.4 (2026-10-14)

Rendering tests for the two pages a person opens from a link.

### Added
- **Tests of the invitation page** for each state: being checked; valid for someone with an account (sign in, join, continue to the workspace); valid for someone without one (create the account with the invited address and the invitation as proof); signed in as the invited address (one button to join); signed in as someone else (sign out, no way to join); used, expired and withdrawn links with the server's reason; no reason given; joining failing after signing in (the page stays so it can be retried); the destination of an organization invitation.
- **Tests of the password reset page**: being checked; valid (the account is named, two fields); saving signs this browser in; two different passwords are never sent; the server refusing a password keeps the form; used and expired links; no reason given.
- 51 web tests in all. No product code changed.

### Upgrading
- Nothing to do.

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
