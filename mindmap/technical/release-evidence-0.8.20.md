# The 0.8 line: closing record (V0.8.20)

> **Date:** 2026-11-18 · **Plan:** [0.8 plan](../plans/2026-11-18-0.8-connections.md) · **Index:** [0.8 releases](../RELEASES_0.8.md) · **Review:** [0.8 security review](../security/2026-11-18-0.8-security-review.md)

This record says what the line delivers, what was and was not checked, what was deliberately not built, and the exit journey written as a sheet for the maintainer. **Every result below is pending.** No pilot was run, no real tracker, Slack workspace or Confluence export was contacted, and no test was executed. The row is therefore 🧪, not ✅.

## What the line delivers

| Release | Delivered | Migration |
|---|---|---|
| 0.8.0 | The connector contract, support matrix, rules and exit journey (docs) | none |
| 0.8.1 | An import preview and the administrator's explicit approval to share; GitLab projects with confidential issues refused | none |
| 0.8.2 | Quarantine of a connector's evidence when access is lost (confirmed twice), a release route, a stated stale window | 0080 |
| 0.8.3 | Connector health (eight states), checkpoints per stored page, recoverable errors | none |
| 0.8.4 | Signed outbound webhooks, approved hosts only, bounded retries, delivery inspection | 0081 |
| 0.8.5, 0.8.6 | Jira Cloud intake with a dry run; edits, removals, restrictions and the failure cases as fixtures | none |
| 0.8.7 | **Linear deferred** (no pilot owner) | none |
| 0.8.8 | Follow-up tasks linked both ways to mirrored items, tracker state kept apart from local status | none |
| 0.8.9 | Slack digests as a count and a link, once a day, with a ledger | 0082 |
| 0.8.10 | **Teams deferred** (no pilot owner) | none |
| 0.8.11, 0.8.12 | A Markdown directory import and a Confluence HTML export import: dry run, link mapping, ledger | 0083 |
| 0.8.13 | AI usage stored in the database, workspace limits, a usage view | 0084 |
| 0.8.14 | A versioned evaluation set, a source-text rule for Ask, wrong-answer corrections | 0085 |
| 0.8.15 | A security review: four medium findings fixed, none open; the rules for public publishing | none |
| 0.8.16 to 0.8.18 | Public snapshots: preview and record, controls and stale state, anonymous reading | 0086 |
| 0.8.19 | Archive format 5: provenance and publication history as records only | none |

Migrations 0080 to 0086. The line is **API-first**: no web screen exists for endpoints, destinations, imports, usage, corrections or publications, except the connection preview, approval, health and quarantine in the connectors panel. Rows that describe a screen behaviour are met by what the API returns and are not claimed as screens.

## What was checked, and what was not

**Run:** `cargo fmt`, `cargo clippy -D warnings` on the domain, storage, API and server crates (all targets, so every new test compiles), the version, migration-lock, generated-types, styles, accessibility and documentation-link checks, and TypeScript.

**Not run:** every test of the line (they compile; none was executed), a browser, a screen reader, a second machine, a server start, a build and CI. `cargo clippy --workspace` fails on a lint in `crates/templates/src/tests.rs` that was already there and was left alone. Nothing was pushed, tagged or published.

## Not built, on purpose

Linear and Microsoft Teams (conditional rows with no pilot owner); per-user mirroring of a source's permissions; writing anything back to a tracker; Jira boards, sprints, attachments and webhooks; direct messages, threads or replies in chat; ZIP upload and the Confluence XML or Cloud export; page restrictions and history from Confluence; money or token pricing for AI; Markdown rendering, custom domains and reader counts for public pages; a web screen for any of the API-only features.

## Support claims

Supported: GitHub, GitLab and Jira Cloud as read-only shared-source imports; Slack channel digests by incoming webhook; signed outbound webhooks to approved hosts; Markdown directory and Confluence HTML imports; public snapshots of text files. **Not supported and not claimed:** Linear, Teams, and any source whose items are restricted to some readers.

## The exit journey (a sheet for the maintainer)

Use a server with `REPOMEMO_GIT_ALLOWED_HOSTS` naming the tracker, `REPOMEMO_OUTBOUND_HOSTS` naming `hooks.slack.com` and the webhook receiver, `REPOMEMO_PUBLIC_URL` set, and `REPOMEMO_PUBLIC_PUBLISHING` on. Fill the result column; "pass" means the stated thing was seen.

| # | Step | Expect | Result |
|---|---|---|---|
| 1 | Check access to a Jira project (or a GitLab project), read the preview, tick the approval, connect | The preview names the partition, what is copied and who reads it; a project with security levels is refused; connect works only with the approval | pending |
| 2 | Stop the first sync half way (stop the tracker), then resume | Health says failed with a step to take, then complete; no item is stored twice | pending |
| 3 | Edit and delete an issue in the tracker, then sync | The edit replaces the copy, the deleted issue leaves search; the report counts one removed | pending |
| 4 | Revoke the token's access, then sync twice | The evidence is held back (search, files, exports), the file routes answer 404; Release works only once access is back | pending |
| 5 | Make a follow-up task from a mirrored issue, finish the task | The citation shows the tracker's state as the tracker's; finishing the task does not change it | pending |
| 6 | Add a Slack destination and run its digest; turn it off | One message, a count and a link with no title; a second run sends nothing; off sends nothing | pending |
| 7 | Import a Markdown directory and a Confluence HTML export; send each again | Links and attachments work; the second send makes nothing; macros and the restrictions note are reported | pending |
| 8 | Preview and publish a snapshot; read it with no account; withdraw it | The reader sees only the previewed text; after withdrawal the same address is the plain 404 | pending |
| 9 | Export the workspace and import it as a new one | Attribution kept; no credential, webhook or destination came; the publication is withdrawn until published again | pending |

**Connector denial fixtures** (the contract's `perm-lost`, `del-vs-fail`, `rate-pause`, `id-repeat`, `cur-resume`, `own-readonly`) are covered by the tests in `connectors_quarantine.rs`, `connectors_health.rs`, `connectors_jira.rs` and `connectors_jira_lifecycle.rs`, which have not been run.

## Before opening the 0.9 line

Run the test suite and record it in a release-evidence record like [the 0.6 one](release-evidence-0.6.13.md); fill the sheet above; decide whether Linear or Teams have a pilot owner; and keep the version range an ordered plan, not a forecast.
