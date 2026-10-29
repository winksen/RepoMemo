# RepoMemo releases: 0.6 → 0.7 (proposed)

> **Theme:** Trusted knowledge in daily work · **Baseline:** V0.4.104 · **Related:** [Roadmap](ROADMAP.md#proposed-path-to-10-team-readiness) · [Functional readiness report](analysis/2026-10-29-team-readiness.md)

**Status:** proposed on 2026-10-29; no row below is shipped or a release commitment. One intended commit per row, named `Update V0.N.P`, with a short body describing the change. Split oversized rows before implementation and renumber only unshipped rows. Update the changelog, user/API guides, mindmaps, archive compatibility and focused checks with the implementation, not in this planning change. No product version is bumped by this plan.

**Entry:** The 0.5 access model is stable. Reuse existing notifications, lifecycle, citation editing, templates, saved searches and workspace archives.

**Exit:** A teammate reaches value without AI, recovers an interrupted edit, reviews a changed conclusion, restores an accidental deletion and completes the workflow by keyboard. New knowledge data survives export/import.

**Scope control:** local OCR (0.6.13), a second tracker (0.8.7) and a second chat adapter (0.8.10) require a pilot owner before implementation. Jira/Slack are the default first choices; pilot evidence may select Linear/Teams instead before work starts. Record a deferral and narrow the supported feature list when a conditional item is not justified. Access boundaries, trustworthy evidence, recovery and publication isolation are mandatory. Full details and pilot thresholds are in the [readiness report](analysis/2026-10-29-team-readiness.md).

**Type:** ✨ Feature · 🔧 Enhancement · 🐞 Fix · 🔒 Security · 📘 Update (docs, process, evidence, release).
**Status:** ✅ shipped (with date) · 🔄 in progress · 🔜 planned. Every row starts planned; a written plan is not an acceptance-test pass.

## Every release

| Release | Status | Track | Type | Area | What changes | Done when |
|---|---|---|---|---|---|---|
| **V0.6.0** | 🔜 | AJ. Daily knowledge | 📘 | Opening | Define the first-value and knowledge-review journeys with representative fixtures and clear state terminology | Fixtures cover no AI, no mail, stale evidence, missing sources and a guest; ownership and approval meanings are agreed in the plan |
| **V0.6.1** | 🔜 | AJ. Daily knowledge | ✨ | Onboarding | Add a dismissible workspace checklist using the chosen template and real source readiness | A new member adds evidence, finds it and saves a cited memory without configuring AI; progress follows completed actions rather than clicks |
| **V0.6.2** | 🔜 | AJ. Daily knowledge | ✨ | My work | Add a permission-filtered personal view of assigned tasks, due reviews and mentions across workspaces | A returning user reaches the next action from one view; inaccessible work and counts disappear after revocation |
| **V0.6.3** | 🔜 | AJ. Daily knowledge | ✨ | Memory history | Keep attributed revisions of memory content and its citation set, with compare and restore | An earlier conclusion and its evidence can be inspected; restore creates a new revision and never rewrites history |
| **V0.6.4** | 🔜 | AJ. Daily knowledge | 🔒 | Concurrent editing | Reject stale note and memory writes using a version precondition | Two editors cannot silently overwrite each other; the losing editor retains their text and can compare or reload explicitly |
| **V0.6.5** | 🔜 | AJ. Daily knowledge | 🔧 | Draft recovery | Recover interrupted note/memory drafts without publishing them automatically | Reload or a failed save preserves the draft; logout, server switch and revoked access cannot reveal it to another session |
| **V0.6.6** | 🔜 | AJ. Daily knowledge | ✨ | Review ownership | Add an accountable reviewer and next-review date to evidence and memory cards | Owners can delegate review, departed owners are surfaced, and the queue distinguishes overdue review from changed evidence |
| **V0.6.7** | 🔜 | AJ. Daily knowledge | 🔧 | Review reminders | Schedule review reminders through existing preferences, inbox and digest delivery | A due review produces one deduplicated reminder per policy window; timezone, snooze and muted-mail behavior are predictable |
| **V0.6.8** | 🔜 | AJ. Daily knowledge | ✨ | Approval workflow | Add optional reviewer approval tied to a specific knowledge revision | Editing approved content invalidates its approval; configured independent review cannot be self-approved and decisions retain attribution |
| **V0.6.9** | 🔜 | AJ. Daily knowledge | 🔧 | Change explanation | Show the evidence changes that caused a memory card to need review | The reviewer can inspect previous/current passages or an explicit unavailable-source reason and resolve the affected citation in context |
| **V0.6.10** | 🔜 | AJ. Daily knowledge | 🔧 | Citation durability | Bind saved references and portable exports to inspectable content revisions | A source update cannot silently make a historical citation appear current; unavailable snapshots and access refusals are stated accurately |
| **V0.6.11** | 🔜 | AJ. Daily knowledge | ✨ | Deletion recovery | Add bounded trash and restore for locally authored/uploaded evidence and memory | Accidental deletion can be undone with references restored; purge is explicit and audited; remotely revoked or removed source content is not resurrected by trash |
| **V0.6.12** | 🔜 | AJ. Daily knowledge | 🔧 | Image intake | Allow image storage and preview with no AI provider, showing its searchability state | A screenshot can be kept and found by metadata without leaving the server; later optional vision indexing never implies it was already text-searchable |
| **V0.6.13** | 🔜 | AJ. Daily knowledge | ✨ | Local OCR (conditional) | Add optional bounded local OCR for scanned PDFs and images, retaining original/page provenance | Pilot scans yield searchable page-linked text; missing OCR gives a clear limitation, low-quality extraction is labelled and resource limits hold |
| **V0.6.14** | 🔜 | AJ. Daily knowledge | ✨ | Organization templates | Let organization admins save and version templates using the existing validated template model | A workspace previews and adopts an org template without restart; template updates never change access or silently move existing content |
| **V0.6.15** | 🔜 | AJ. Daily knowledge | 🔧 | Folders | Move evidence folders with a preview of descendants and preserved references | Cycles and depth overflow are refused; paths update consistently and citations/comments remain attached |
| **V0.6.16** | 🔜 | AJ. Daily knowledge | ✨ | Search feedback | Capture an unanswered question or unhelpful result as a permission-scoped improvement request | The user can submit without losing query context; an owner can resolve it to evidence or a memory, and query retention is explicit |
| **V0.6.17** | 🔜 | AJ. Daily knowledge | 🔧 | Journey UX | Make the onboarding, personal queue and review flow keyboard/narrow-screen usable | Focus, context and drafts survive navigation, retry and reconnect; the specified journeys work at 200% zoom and in both themes with recorded manual checks |
| **V0.6.18** | 🔜 | AJ. Daily knowledge | 🔧 | Portability | Extend workspace archives for knowledge revisions, review policy, templates and recovery state | Round-trip preserves citation meaning and records, rechecks identities and never imports a privileged approval as a trusted local actor |
| **V0.6.19** | 🔜 | AJ. Daily knowledge | 📘 | Closing | Record no-AI activation, review, edit-conflict and recovery walkthroughs | The daily-knowledge exit journey passes; guides distinguish implemented OCR or its explicitly deferred scope |

## Verification and handoff

Before opening the next line, record the exit journey, changed-data upgrade and export/import checks, role-denial cases and supported-client behavior. Tests must demonstrate the user outcome, including failure and recovery. Follow the repository's execution rules; unrun checks remain explicitly unrun and the maintainer supplies live evidence.

Any new storage field, enum or entity must include its archive compatibility and migration handling in its own implementation; the line's portability row verifies the complete journey, rather than postponing data exit until the end.

When a row ships, update this index and the matching roadmap row together; put the actual behavior and limitations in the changelog. The version range is an ordered plan, not a calendar forecast.
