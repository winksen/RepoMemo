# V0.6.30 closing record: trusted daily knowledge

Prepared 2026-11-17. Baseline: V0.5.19. This closes the 0.6.x line locally. It is a local candidate, not a published artifact.
**None of the walkthroughs below has been performed**: no browser, desktop app, screen reader or second machine was
available, and the author executed no test and built nothing. They are written so the maintainer can do them and record the
result; every result cell is pending, so this row is 🧪 (built, not validated) and not ✅.

## What the line delivers

| Releases | Capability | Where it is described |
|---|---|---|
| V0.6.0–V0.6.2 | The journeys; a getting-started checklist that needs no AI; *My work* across workspaces, filtered by current access | [plan](../plans/2026-11-01-0.6-trusted-knowledge.md), [user guide](../../docs/USER_GUIDE.md) |
| V0.6.3–V0.6.5 | Attributed memory revisions with compare and restore; stale saves refused; interrupted edits kept as drafts in the tab | user guide § Memory cards |
| V0.6.6–V0.6.9 | Reviewer and next review date, reminders, approval tied to a revision with an independent-review rule, *What changed* behind a "needs review" flag | user guide § Review |
| V0.6.10–V0.6.12 | Citations bound to the content they cited; an author written with the revision; imported cards given a history | changelog |
| V0.6.13 | The [test checkpoint](release-evidence-0.6.13.md) (results pending) | |
| V0.6.14, V0.6.28 | Workspace archives carry history, recorded content, approvals as records, review and the template, and say what they leave behind | user guide § Moving a workspace |
| V0.6.15 | Bounded trash for cards and uploaded files, restore, audited purge | user guide § Trash |
| V0.6.16–V0.6.18 | The release record corrected, one tag format, upload version checks made atomic and the unchecked write mode named | [API guide](../../docs/API.md) |
| V0.6.19, V0.6.20 | Revision paging and the retention rule; web tests for five panels | |
| V0.6.21, V0.6.22 | Images kept and found by name with no AI; **local OCR deferred** (no pilot owner) | [search guide](../../docs/SEARCH_AND_ASK.md) |
| V0.6.23–V0.6.25 | Organization templates; a folder move preview; search feedback to the owners | user guide |
| V0.6.26, V0.6.27 | Keyboard and narrow-screen fixes with a [check sheet](journey-checks-0.6.26.md); one date and number formatter and a message catalogue (no translation shipped) | |
| V0.6.29 | The [security review](../security/2026-11-17-0.6-security-review.md): 0 high, 1 medium, 3 low, 5 info, none open | |

Migrations added by the line: 0055–0063 (revisions, review assignments, reminders, approvals, imported card history, knowledge
archives, trash, organization templates, search feedback). All are locked and additive; none rewrites existing access.

**Scope that was not built:** OCR of scans and pictures of text. The guides say scans are kept and previewed but not read, and
what to do instead. A pilot owner reopens it.

## Checks run for this record

| Check | Result | Scope |
|---|---|---|
| Version, migration lock (63 files), generated types, styles, accessibility rules, documentation links | Pass | Local source checks |
| TypeScript | Pass | `npm run typecheck`; it compiles every web test |
| Clippy with warnings denied, storage, api and server crates, all targets | Pass | Compiles every Rust test, runs none |
| Rust format | Pass | `cargo fmt --all` |
| Executing any Rust or web test | **Not run** | The maintainer's own run is owed ([record](release-evidence-0.6.13.md)) |
| Any migration's SQL | **Not run** | It runs when the server first starts on a database |
| CI on a pushed commit | **Not run** | Nothing was pushed |
| Browser, screen reader, 200% zoom, a second machine | **Not run** | Owed ([sheet](journey-checks-0.6.26.md)) |

## The exit journey (do this on a test server, with no AI provider configured)

One continuous journey for a small team of three people: an owner, a member and a guest who can only read. Record each step.

1. **Activate with no AI.** The owner creates a workspace and follows *Getting started*: uploads a file (a text file and a
   screenshot), sees *Make it searchable* complete, saves a memory card that cites the file. Search for a word in the file and
   for the screenshot's name; confirm the screenshot page says it is searchable by name and path only.
2. **Review a changed conclusion.** The owner assigns the member as reviewer with a date, then uploads a new version of the
   cited file. The card shows *Needs review* and **What changed** with the earlier and current text. The member opens the file,
   edits the card, and approves the new revision; the owner cannot approve their own revision once *Require independent
   review* is on. *My work* shows the review to the member while it is due and drops it afterwards.
3. **Edit conflict and draft.** The owner and the member open the same card and both edit. The second save is refused with the
   first person's revision named, their text is kept, and they can retry from the newer revision or overwrite knowingly. Close
   the tab mid-edit and reopen the card: the draft is offered and nothing was saved automatically.
4. **Recovery.** The owner deletes the card and the file, finds both in **Settings › Trash**, restores them, and checks that the
   card's revisions, approval and review date came back and that the file is cited again. An administrator removes one item for
   good and the activity feed records it. A guest cannot restore or remove.
5. **Move the workspace.** Export the workspace, import the archive as a new workspace: the history, recorded content, review
   date and *Require independent review* come across, the approval shows as *Approved elsewhere* and approves nothing, and the
   import report mentions the trash it left behind. Export the new workspace again and import that.
6. **By keyboard.** Repeat steps 1, 2 and 4 using only the keyboard, at 200% zoom and in both themes (the
   [check sheet](journey-checks-0.6.26.md) lists the conditions).

## Record the result

| Step | Result | Date | Notes (what stopped you, if anything) |
|---|---|---|---|
| 1. Activate with no AI | pending | | |
| 2. Review a changed conclusion | pending | | |
| 3. Edit conflict and draft | pending | | |
| 4. Recovery | pending | | |
| 5. Move the workspace | pending | | |
| 6. By keyboard, zoom and themes | pending | | |

A step that fails becomes a numbered fix row in the roadmap; do not edit the step to pass. When every step passes and the
[test run](release-evidence-0.6.13.md) is recorded, change V0.6.30 and the tests row from 🧪 to ✅.

## Distribution decision

No distribution is newly certified ready by this candidate. The intended validation scope remains a single-node server and web
deployment and the Windows desktop client. The release tag for a future publication is `vX.Y.Z`
([release tags](../../docs/DEVELOPMENT_COMMANDS.md#release-tags)); nothing was tagged, built or published.
