# RepoMemo releases: 0.8 → 0.9 (proposed)

> **Theme:** Connected teams and community knowledge · **Baseline:** V0.4.104 · **Related:** [Roadmap](ROADMAP.md#proposed-path-to-10-team-readiness) · [Functional readiness report](analysis/2026-10-29-team-readiness.md)

**Status:** proposed on 2026-10-29; no row below is shipped or a release commitment. One intended commit per row, named `Update V0.N.P`, with a short body describing the change. Split oversized rows before implementation and renumber only unshipped rows. Update the changelog, user/API guides, mindmaps, archive compatibility and focused checks with the implementation, not in this planning change. No product version is bumped by this plan.

**Entry:** The 0.5 audience controls and 0.6 revision/approval model have passed their gates. Read-only tracker mirrors reference 0.7 projects; public publishing uses a separate, explicit audience.

**Exit:** A pilot can migrate existing work, keep one tracker mirror current, receive safe chat links and publish/revoke a reviewed knowledge snapshot. Deletion, lost credentials, restricted source content and retries have demonstrated outcomes.

**Scope control:** local OCR (0.6.13), a second tracker (0.8.7) and a second chat adapter (0.8.10) require a pilot owner before implementation. Jira/Slack are the default first choices; pilot evidence may select Linear/Teams instead before work starts. Record a deferral and narrow the supported feature list when a conditional item is not justified. Access boundaries, trustworthy evidence, recovery and publication isolation are mandatory. Full details and pilot thresholds are in the [readiness report](analysis/2026-10-29-team-readiness.md).

**Type:** ✨ Feature · 🔧 Enhancement · 🐞 Fix · 🔒 Security · 📘 Update (docs, process, evidence, release).
**Status:** ✅ shipped (with date) · 🔄 in progress · 🔜 planned. Every row starts planned; a written plan is not an acceptance-test pass.

## Every release

| Release | Status | Track | Type | Area | What changes | Done when |
|---|---|---|---|---|---|---|
| **V0.8.0** | 🔜 | AL. Connections and sharing | 📘 | Opening | Define a connector conformance contract: ownership, audience, cursor, freshness, deletion, rate limits and permission loss | Fixtures and a support matrix distinguish a deliberate shared-source import from unsupported per-user source ACL mirroring |
| **V0.8.1** | 🔜 | AL. Connections and sharing | 🔒 | Connection setup | Preview the selected source partition, copied data and destination audience before importing | An admin explicitly approves uniform sharing; sources requiring unsupported item-level restrictions are refused and secrets never appear in the preview |
| **V0.8.2** | 🔜 | AL. Connections and sharing | 🔒 | Source access changes | Quarantine affected mirrored evidence when access is revoked or sharing can no longer be established | Denied sources disappear from retrieval, previews, exports and publication; transient failures and deletion differ, with a documented maximum stale-access window |
| **V0.8.3** | 🔜 | AL. Connections and sharing | 🔧 | Connector health | Expose last successful sync, coverage, exclusions, backlog and recoverable errors | An operator distinguishes complete, partial, stale and paused sources, resumes a cursor and diagnoses a retry without duplicate evidence |
| **V0.8.4** | 🔜 | AL. Connections and sharing | ✨ | Outbound events | Add opt-in signed outbound webhooks with event IDs, bounded retries and delivery inspection | Only approved destinations receive selected events, duplicate delivery is identifiable and disabled endpoints stop receiving queued content |
| **V0.8.5** | 🔜 | AL. Connections and sharing | ✨ | Jira intake | Read selected Jira Cloud project issues into existing connector evidence with stable remote IDs | A dry run reports scope and permissions; paged import resumes, preserves source links and changes nothing in Jira |
| **V0.8.6** | 🔜 | AL. Connections and sharing | 🔧 | Jira lifecycle | Reconcile Jira edits, removals and access changes through the connector contract | Rate limits, repeated pages, removed access and deleted issues pass conformance fixtures; remote/local status ownership is explicit |
| **V0.8.7** | 🔜 | AL. Connections and sharing | ✨ | Linear (conditional) | Add a read-only Linear team/project adapter using the same conformance contract | A startup pilot imports and resyncs a representative fixture without duplicates or permission expansion; updates and deletion are covered |
| **V0.8.8** | 🔜 | AL. Connections and sharing | 🔧 | Tracker context | Link mirrored issues and pull requests to local projects and tasks without duplicating ownership | A user can distinguish remote state from local follow-up, navigate both directions through links and never mistakes a local edit for a remote update |
| **V0.8.9** | 🔜 | AL. Connections and sharing | ✨ | Slack delivery | Send opted-in review/project digests to approved Slack destinations as authenticated links | A pilot receives deduplicated links without sensitive titles/excerpts, unauthorized readers cannot open them and disconnect stops delivery |
| **V0.8.10** | 🔜 | AL. Connections and sharing | ✨ | Teams delivery (conditional) | Add equivalent link-only delivery for Microsoft Teams using the same destination and retry policy | A company pilot validates the destination, receives one digest per window and verifies revocation; unsupported delivery modes are named |
| **V0.8.11** | 🔜 | AL. Connections and sharing | ✨ | Knowledge migration | Import a Markdown/asset directory with link mapping, preview and resumable progress | A representative wiki migrates with internal links and attachments intact; unsupported and unsafe paths are reported and retries do not duplicate records |
| **V0.8.12** | 🔜 | AL. Connections and sharing | ✨ | Confluence migration | Add a bounded importer for a documented Confluence export format | A representative space export preserves supported hierarchy, links and source attribution; restrictions require explicit mapping and unsupported macros are reported |
| **V0.8.13** | 🔜 | AL. Connections and sharing | 🔧 | AI administration | Extend existing AI limits with visible workspace budgets, usage and provider-failure behavior | Admins see consumption and limit reasons, spending limits stop calls predictably and keyword search remains available when AI is disabled or unavailable |
| **V0.8.14** | 🔜 | AL. Connections and sharing | 🔒 | Answer quality | Add a versioned retrieval/answer evaluation set and in-product correction feedback | Known/absent/conflicting evidence, stale sources and hostile source instructions are covered; answers cite accessible evidence and never treat source text as action authority |
| **V0.8.15** | 🔜 | AL. Connections and sharing | ✨ | Public snapshots | Create an opt-in publication record containing only explicitly reviewed snapshot content | Publishing requires an authorized preview; private citations, comments, attachments, people and metadata are excluded unless independently approved as public |
| **V0.8.16** | 🔜 | AL. Connections and sharing | ✨ | Publication controls | Add public preview, publish, unpublish and visible stale/source-revoked state | A publisher sees exactly what anonymous readers see; withdrawal and source-access revocation invalidate serving/caches without changing the private original |
| **V0.8.17** | 🔜 | AL. Connections and sharing | ✨ | Community reading | Add a responsive public knowledge index and search limited to published snapshots | Anonymous readers can navigate approved content; query results, counts, URLs and errors reveal no private workspace content |
| **V0.8.18** | 🔜 | AL. Connections and sharing | 🔧 | Integration portability | Carry connector provenance and publication metadata safely in archives | Import retains attribution but reconnects no credentials, sends no webhooks and publishes nothing until explicitly reauthorized |
| **V0.8.19** | 🔜 | AL. Connections and sharing | 📘 | Closing | Validate one real tracker/chat pilot, knowledge migration and community publication journey | The exit journey and connector denial fixtures pass; conditional adapters lacking pilot evidence are explicitly deferred, with no support claim |

## Verification and handoff

Before opening the next line, record the exit journey, changed-data upgrade and export/import checks, role-denial cases and supported-client behavior. Tests must demonstrate the user outcome, including failure and recovery. Follow the repository's execution rules; unrun checks remain explicitly unrun and the maintainer supplies live evidence.

Any new storage field, enum or entity must include its archive compatibility and migration handling in its own implementation; the line's portability row verifies the complete journey, rather than postponing data exit until the end.

When a row ships, update this index and the matching roadmap row together; put the actual behavior and limitations in the changelog. The version range is an ordered plan, not a calendar forecast.
