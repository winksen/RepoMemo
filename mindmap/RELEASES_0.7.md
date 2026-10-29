# RepoMemo releases: 0.7 → 0.8 (proposed)

> **Theme:** Projects connected to their evidence · **Baseline:** V0.4.104 · **Related:** [Roadmap](ROADMAP.md#proposed-path-to-10-team-readiness) · [Functional readiness report](analysis/2026-10-29-team-readiness.md)

**Status:** proposed on 2026-10-29; no row below is shipped or a release commitment. One intended commit per row, named `Update V0.N.P`, with a short body describing the change. Split oversized rows before implementation and renumber only unshipped rows. Update the changelog, user/API guides, mindmaps, archive compatibility and focused checks with the implementation, not in this planning change. No product version is bumped by this plan.

**Entry:** The 0.5 access boundary and 0.6 knowledge/revision model are established. A workspace remains the security boundary; projects organize work inside it.

**Exit:** A small team can create a project brief, plan a milestone, assign and discuss tasks, identify dependencies, publish a grounded status update and export/import the result. Existing trackers remain the authority for their mirrored issues.

**Scope control:** local OCR (0.6.13), a second tracker (0.8.7) and a second chat adapter (0.8.10) require a pilot owner before implementation. Jira/Slack are the default first choices; pilot evidence may select Linear/Teams instead before work starts. Record a deferral and narrow the supported feature list when a conditional item is not justified. Access boundaries, trustworthy evidence, recovery and publication isolation are mandatory. Full details and pilot thresholds are in the [readiness report](analysis/2026-10-29-team-readiness.md).

**Type:** ✨ Feature · 🔧 Enhancement · 🐞 Fix · 🔒 Security · 📘 Update (docs, process, evidence, release).
**Status:** ✅ shipped (with date) · 🔄 in progress · 🔜 planned. Every row starts planned; a written plan is not an acceptance-test pass.

## Every release

| Release | Status | Track | Type | Area | What changes | Done when |
|---|---|---|---|---|---|---|
| **V0.7.0** | 🔜 | AK. Project execution | 📘 | Opening | Specify the lightweight project model and its limits with a sample team journey | Project, milestone, task, checklist and evidence have distinct meanings; legacy tasks need no project and advanced portfolio/sprint planning is excluded |
| **V0.7.1** | 🔜 | AK. Project execution | ✨ | Projects | Add workspace-scoped projects with owner, goal, lifecycle and target date | Members can create and archive a project; its access is exactly the workspace's and legacy tasks continue to work |
| **V0.7.2** | 🔜 | AK. Project execution | ✨ | Project brief | Attach decision memories and evidence to a project brief with backlinks | A teammate can explain the project's goal and supporting decisions without copying source text or following a broken access link |
| **V0.7.3** | 🔜 | AK. Project execution | ✨ | Task details | Give tasks stable human-readable identifiers, direct URLs and a dedicated detail view | A link opens the same task after title changes; guests receive an authorized view and missing/deleted tasks have useful states |
| **V0.7.4** | 🔜 | AK. Project execution | ✨ | Task discussion | Add task comments, mentions and explicit watching using existing notification preferences | A discussion stays with its task, mentions notify once and unwatch/mute stops optional delivery without losing assignment notices |
| **V0.7.5** | 🔜 | AK. Project execution | 🔧 | Task context | Replace the single-artifact limit with multiple evidence and memory references | Existing artifact links migrate intact; new links remain permission-checked and the task can cite the decision that created it |
| **V0.7.6** | 🔜 | AK. Project execution | 🔒 | Task editing | Apply optimistic concurrency to task details and status changes | Concurrent edits cannot silently drop assignment, description or status; recoverable conflicts retain the attempted change |
| **V0.7.7** | 🔜 | AK. Project execution | ✨ | Milestones | Group project tasks into named milestones with due dates and completion rules | A lead sees completed, remaining and overdue work with an explainable denominator; changing a date does not silently reschedule tasks |
| **V0.7.8** | 🔜 | AK. Project execution | ✨ | Task hierarchy | Add parent/child tasks separate from checklist steps | Parents and children stay in the same authorized workspace, cycles are refused and parent completion does not hide unfinished children |
| **V0.7.9** | 🔜 | AK. Project execution | ✨ | Dependencies | Add blocks/blocked-by links and a clear blocker explanation | Self-links and cycles are refused; closing a blocker updates the indication without automatically completing dependent work |
| **V0.7.10** | 🔜 | AK. Project execution | ✨ | Labels | Add workspace-managed task labels with accessible colors and filters | Labels can be renamed/merged without losing assignments and labels cannot be used as an access-control substitute |
| **V0.7.11** | 🔜 | AK. Project execution | ✨ | Workflows | Allow named workflow states mapped to stable open/in-progress/blocked/done categories | Existing four states migrate unchanged; reporting and integrations use explicit category mappings, and removal requires remapping occupied states |
| **V0.7.12** | 🔜 | AK. Project execution | ✨ | Task list | Add a paged task list with saved filters for project, milestone, assignee, priority, label and due date | Saved views re-evaluate current rights; URLs retain the chosen view and large lists do not require downloading all tasks |
| **V0.7.13** | 🔜 | AK. Project execution | 🔧 | Task board | Use the same query/view model for the board with keyboard status movement | Board and list show the same filtered work; movement preserves focus and failed changes roll back visibly |
| **V0.7.14** | 🔜 | AK. Project execution | 🔧 | Bulk work | Add previewed multi-task assignment, labels and milestone updates | Each item reports success or refusal, failed items remain selected and authorization/concurrency are checked per task |
| **V0.7.15** | 🔜 | AK. Project execution | ✨ | Due reminders | Add task due/overdue reminders through the existing inbox and digests | Timezone and date-only behavior are defined, completed work is excluded and retries do not duplicate notifications |
| **V0.7.16** | 🔜 | AK. Project execution | ✨ | Recurring work | Create recurring tasks from explicit schedules with an occurrence identity | Retries and restarts create only one occurrence, missed runs have a documented policy and a recurring task can be stopped without deleting history |
| **V0.7.17** | 🔜 | AK. Project execution | ✨ | Project updates | Publish attributed project status updates with milestone, blocker and evidence links | A reader can inspect what changed and why the lead chose on-track/at-risk/blocked; updates are authored or confirmed by a person |
| **V0.7.18** | 🔜 | AK. Project execution | ✨ | CSV migration | Import and export task CSV with field mapping and a dry-run report | Invalid rows and unknown assignees are explained before commit; retries avoid duplicates and exported cells are safe to open in spreadsheets |
| **V0.7.19** | 🔜 | AK. Project execution | 🔧 | Project portability | Extend workspace archives for projects, task relations, comments, workflow and schedules | Round-trip retains relationships, rejects cross-workspace references and imports schedules paused pending explicit activation |
| **V0.7.20** | 🔜 | AK. Project execution | 📘 | Closing | Validate a complete small-team project and document coexistence with external trackers | A team completes the exit journey; no unsupported Gantt, resource accounting, sprint or portfolio capability is claimed |

## Verification and handoff

Before opening the next line, record the exit journey, changed-data upgrade and export/import checks, role-denial cases and supported-client behavior. Tests must demonstrate the user outcome, including failure and recovery. Follow the repository's execution rules; unrun checks remain explicitly unrun and the maintainer supplies live evidence.

Any new storage field, enum or entity must include its archive compatibility and migration handling in its own implementation; the line's portability row verifies the complete journey, rather than postponing data exit until the end.

When a row ships, update this index and the matching roadmap row together; put the actual behavior and limitations in the changelog. The version range is an ordered plan, not a calendar forecast.
