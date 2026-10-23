# RepoMemo releases: 0.3 → 0.4

> **Line:** `V0.3.1` to `V0.3.27`, then `V0.4.0` (theme to be chosen, decision 33) · **Current:** `V0.3.0` *Connected* (2026-10-24) · **Next:** `V0.3.1`, the fixes the tests of 0.3.0 found
> **Previous line:** [Releases 0.2 → 0.3](RELEASES_0.2.md) · **Related:** [Roadmap, Road to V0.4.0](ROADMAP.md#-road-to-v040) · [Changelog](../CHANGELOG.md) · [Functional mindmap](FUNCTIONAL_MINDMAP.md) · [Technical mindmap](TECHNICAL_MINDMAP.md)

Every release of the 0.3 line, planned and later shipped, one row each. The plan comes from the [Road to V0.4.0](ROADMAP.md#-road-to-v040), proposed on 2026-10-24: **one change per release**, each shippable alone, committed as `Update V0.3.N`. Decisions 29–33 of the roadmap are still to confirm, so any row can move, change or go.

**Type:** ✨ Feature · 🔧 Enhancement · 🐞 Fix · 🔒 Security · 📘 Update (docs, process, dependencies, release).
**Status:** ✅ shipped (with its date) · 🔄 in progress · 🔜 planned

When a release ships, set its status to ✅ with the date and make *What changes* say what was built.

---

## The tracks

| Releases | Track | What's in it |
|---|---|---|
| V0.3.1–V0.3.4 | O. After 0.3.0 | Fixes from the tests of 0.3.0, every test written since V0.2.46 run once, formatting made clean and blocking, the performance, browser and desktop figures recorded |
| V0.3.5–V0.3.7 | P. The retired local edition removed | The local app, then its Tauri commands and the move, then the documents |
| V0.3.8–V0.3.18 | Q. Small gaps closed | Restore a version, mentions in tasks and cards, review count and snooze, assistant memory for lookups, cross-workspace filters, palette recents, citations at a commit from cards, owners matched to members, last author in answers, more symbols, template Overview cards |
| V0.3.19–V0.3.22 | R. Portable archives and connector depth | Versions and saved searches in archives, import as a background job, issue references and approvals, webhook deliveries |
| V0.3.23–V0.3.25 | S. Operations and experience | Compose tested in CI and an `arm64` image, backup passphrase rotation, accessibility residuals |
| V0.3.26–V0.3.27 | T. Closing | Security pass, release checks, the theme of V0.4.0 recorded |
| V0.4.0 | — | Theme to be chosen (decision 33) |

---

## Every release

| Release | Status | Track | Type | Area | What changes | Done when |
|---|---|---|---|---|---|---|
| **V0.3.1** | 🔜 Planned | O. After 0.3.0 | 🐞 Fix | Fixes | Corrections that the maintainer's tests of 0.3.0 found; a finding that needs its own release takes the next number | The step that failed passes on a second run |
| **V0.3.2** | 🔜 Planned | O. After 0.3.0 | 🐞 Fix | Tests | Every test written since V0.2.46 is run once; those that fail are corrected, in the test or in the code | `cargo test --workspace --exclude repomemo-desktop` and `npm run test:web` pass (the maintainer's run) |
| **V0.3.3** | 🔜 Planned | O. After 0.3.0 | 📘 Update | Process | `cargo fmt --all --check` and clippy made clean in one formatting release, then blocking in CI; line endings kept | CI's formatting step passes on `main`; a second formatting run changes nothing |
| **V0.3.4** | 🔜 Planned | O. After 0.3.0 | 📘 Update | Release record | The performance runs, the browser pass and the desktop try recorded; the clean-install result table filled and its pre-tag list ticked | The figures are in `performance.md` and `clean-install.md`; no box is left unticked without a reason |
| **V0.3.5** | 🔜 Planned | P. Local edition removed | 🔧 Enhancement | Desktop | `LocalDesktopApp`, `lib/repomemoApi.ts` and the local-only styles deleted; the app opens on the server address screen only | No local-workspace code under `apps/desktop/src`; the app connects, signs in and shows the web client as before |
| **V0.3.6** | 🔜 Planned | P. Local edition removed | 🔧 Enhancement | Desktop shell | The 26 local Tauri commands, `RepoMemoCore::boot`, the move's commands and the `repomemo-api` dependency removed | `cargo tree -p repomemo-desktop` no longer reaches `repomemo-api`; the shell keeps only the credential store and HTTP commands |
| **V0.3.7** | 🔜 Planned | P. Local edition removed | 📘 Update | Documents | `UPGRADING.md` names V0.3.4 as the last release that can move a local workspace; the mindmaps, the user guide and the commands page stop describing it | No document except `UPGRADING.md` and the changelog mentions the local workspace |
| **V0.3.8** | 🔜 Planned | Q. Small gaps | ✨ Feature | Artifact page | Restore an earlier version of a file as a new version | Restoring version 2 of three makes a fourth with version 2's text; the history keeps all four |
| **V0.3.9** | 🔜 Planned | Q. Small gaps | 🔧 Enhancement | Discussion | `@mentions` in task descriptions and memory cards; organization members who are not in the workspace are not suggested | A mention in a task notifies that member; a non-member is neither suggested nor notified |
| **V0.3.10** | 🔜 Planned | Q. Small gaps | ✨ Feature | Review | A count in the Review tab and a snooze for an item | A snoozed item returns on its date; the tab shows the count |
| **V0.3.11** | 🔜 Planned | Q. Small gaps | 🔧 Enhancement | Assistant | Lookups and searches read earlier turns | "Find the file about login", then "search for it", searches what the first message named |
| **V0.3.12** | 🔜 Planned | Q. Small gaps | 🔧 Enhancement | Retrieval | Search across workspaces takes filters (type, language, date) and finds memory cards | Each result is still checked against its workspace's role and AI policy |
| **V0.3.13** | 🔜 Planned | Q. Small gaps | 🔧 Enhancement | Web shell | The command palette keeps recent items and offers *New note* | Recent items survive a reload; *New note* opens the editor in the open workspace |
| **V0.3.14** | 🔜 Planned | Q. Small gaps | 🔧 Enhancement | Citations | *View at commit* offered from memory-card links and repository summaries | A card's link opens the cited text at the commit it names, labelled as such |
| **V0.3.15** | 🔜 Planned | Q. Small gaps | ✨ Feature | Artifact page | `CODEOWNERS` owners matched to workspace members | A matched owner shows as a member and can be asked to review; an unmatched one stays a hint |
| **V0.3.16** | 🔜 Planned | Q. Small gaps | 🔧 Enhancement | Assistant | Answers and citations name the last author and date of a repository file | A citation of a repository file shows who last changed it |
| **V0.3.17** | 🔜 Planned | Q. Small gaps | 🔧 Enhancement | Indexing | More code symbols: Go `const`, `var` and type aliases; Java fields; C# properties; PHP properties and constants | Each is found in the Code symbols box |
| **V0.3.18** | 🔜 Planned | Q. Small gaps | 🔧 Enhancement | Templates | The Overview cards the templates wanted and could not name: repositories in sync, evidence reviewed in 90 days, needing review, duplicates | A template can list them; General's nine cards are unchanged |
| **V0.3.19** | 🔜 Planned | R. Archives and connectors | 🔧 Enhancement | Archives | Archives carry earlier file versions and saved searches | An export imported on another server shows the version history and the saved searches |
| **V0.3.20** | 🔜 Planned | R. Archives and connectors | 🔧 Enhancement | Import | Import runs as a background job with progress; the browser no longer holds the whole archive | A 1 GiB archive imports with a progress bar and flat browser memory |
| **V0.3.21** | 🔜 Planned | R. Archives and connectors | 🔧 Enhancement | Connectors | Issue references (`#12`) and mentions between items are read; approvals of a pull request are shown | A pull request that names `#12` links to that issue |
| **V0.3.22** | 🔜 Planned | R. Archives and connectors | 🔧 Enhancement | Connectors | Webhook deliveries are listed and can be sent again from the connector's page | A failed delivery is listed with its reason and sent again by a writer |
| **V0.3.23** | 🔜 Planned | S. Operations and experience | 🔧 Enhancement | Container | `docker compose up` tested in CI, and an `arm64` image job | CI starts the compose file and `/health/ready` answers |
| **V0.3.24** | 🔜 Planned | S. Operations and experience | 🔧 Enhancement | Backups | A backup passphrase can be rotated: new backups use the new one, old ones are re-sealed on request | A backup sealed with the old passphrase opens only after re-sealing, or with the old one |
| **V0.3.25** | 🔜 Planned | S. Operations and experience | 🔧 Enhancement | Web client | Accessibility residuals: a skip link in dialogs, roles for the note editor's toolbar, icon buttons of at least 24 px | `a11y:check` covers the three; a screen reader pass finds no serious issue |
| **V0.3.26** | 🔜 Planned | T. Closing | 🔒 Security | Security | Security pass over what 0.3.x added and over the removal of the local edition, with its fixes | No high or medium finding open |
| **V0.3.27** | 🔜 Planned | T. Closing | 📘 Update | Release | Release checks repeated; the theme of V0.4.0 recorded (decision 33); the line closed | Every check passes; the next plan is written |
