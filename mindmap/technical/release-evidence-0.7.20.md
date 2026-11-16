# V0.7.20 closing record: projects connected to their evidence

Prepared 2026-11-18. Baseline: V0.6.30. This closes the 0.7.x line locally. It is a local candidate, not a published artifact.
**None of the walkthroughs below has been performed**: no server was started, no browser or desktop app opened and nothing was
built or run by the author. They are written so the maintainer can do them and record the result; every result cell is
pending, so this row is 🧪 (built, not validated) and not ✅.

## What the line delivers

The line was built **API-first**: every release has its routes, its storage, its documentation and a server test that compiles.
**No web screen of the 0.7 line exists yet** (see [Scope not built](#scope-not-built)).

| Releases | Capability | Where it is described |
|---|---|---|
| V0.7.0 | The vocabulary, six rules for the line, what is excluded, the nine-step journey | [plan](../plans/2026-11-17-0.7-projects.md) |
| V0.7.1–V0.7.2 | Projects inside a workspace (owner, goal, lifecycle, target date, archive and restore); a brief that cites decisions and evidence by reference, with backlinks | [API guide § 4a](../../docs/API.md#4a-projects-07) |
| V0.7.3–V0.7.6 | A task number that names a task for good (`T-12`), a detail route with `410` for a deleted task; comments, mentions that notify once and watch or mute; several citations per task; a version on every save and a conflict answer that keeps the attempted change | API guide § 4a |
| V0.7.7–V0.7.11 | Milestones with an explained count; sub-tasks; blockers with a plain explanation; labels with accessible colours; named workflow states mapped to the four stable categories | API guide § 4a |
| V0.7.12–V0.7.14 | A paged task list and saved views that carry no rights; a board from the same query with checked moves; previewed bulk changes with an outcome for every task | API guide § 4a |
| V0.7.15–V0.7.16 | Due and overdue reminders (UTC, once for each due value); recurring tasks with an occurrence identity, a missed-run policy and a stop that keeps history | API guide § 4a |
| V0.7.17 | Project updates: a person chooses the health, gives the reason and links what it rests on; the counts and what changed are kept as published | API guide § 4a |
| V0.7.18 | Task CSV out (safe in a spreadsheet) and in (mapped, previewed, duplicate-safe) | API guide § 4a |
| V0.7.19 | Workspace archives carry the work (format 4); relations stay inside the new workspace; schedules come back paused | API guide § 4a, user guide § Taking your data with you |

Migrations added by the line: **0064–0079** (projects, project references, task numbers, task discussion, task references, task
versions, milestones, task hierarchy, dependencies, labels, workflow states, saved views, reminders, recurring tasks, project
updates, import ledger). All are locked and additive; none rewrites existing access, and every existing task keeps its status,
its link and its place. The archive format moved from 3 to 4.

## What was checked, and what was not

For each release, with the maintainer's rules (nothing was built, no server was started):

- **Run:** `cargo fmt`, `cargo check --tests` for the server (which builds every crate it uses), `cargo clippy -D warnings` for the
  domain, storage, API and server crates with all targets, the generated-types check, the migration lock, the style, accessibility,
  container and documentation checks, the version check and the web client's TypeScript check.
- **Not run:** **any test.** About twenty server tests and a handful of unit tests (schedule dates, due rules, CSV cells, label
  contrast) were written for the line and compile; none has been executed. No migration has been applied to an existing
  database. The SQL that is assembled at run time (the task list's filters, the milestone and project counts, the label
  aggregate, the CSV ledger) has therefore never run. No mail was sent, no archive was written or read, no spreadsheet opened
  an export and no CI ran. The clippy of the templates crate's own test target has one older warning from a newer Rust lint,
  unchanged by this line.

The first thing to do with this record is the [test run](release-evidence-0.6.13.md): `cargo test` is where a mistake in any of
the above will show.

## Scope not built

- **The web screens of the whole line.** Projects, briefs, task pages, discussions, milestones, the list, saved views, the
  board with keyboard movement, bulk selection, the CSV upload and mapping, schedules and updates are reachable through the
  [API](../../docs/API.md) only. The rows that name a screen behaviour (focus kept on a keyboard move, a card put back visibly,
  the selection that stays after a refused bulk item) are met by what the API returns, **not** by a screen, and are not claimed.
- Time of day in a schedule (an occurrence is a date), a schedule on a day after the 28th, a recurring task that makes
  sub-tasks or labels, and notifications for project updates.
- Anything for another tracker: the line contains no connector (that is the 0.8 line).

## Projects next to an external tracker

Until the 0.8 line, nothing in a project reads a tracker, and nothing writes to one.

- **The tracker stays the authority for its own issues.** A task here is a separate record with its own owner and state. Closing
  one never closes a remote issue, and a remote change never changes one.
- **Moving work in or out is the CSV path** (V0.7.18): export the tasks as a CSV a spreadsheet opens, or import a tracker's CSV by
  mapping its columns, preview every row and import it once (the id of the source row is the task's identity, so a second file
  with the same ids creates nothing twice). The import is a copy at a moment, not a sync.
- **From 0.8** a mirrored issue arrives as read-only evidence; a local task that follows it up links to it and keeps its own state.
- **What this line does not do** is stated plainly in the guides: no Gantt chart or critical path, no resource or capacity
  accounting, no time tracking or budgets, no sprints or velocity, no portfolio across workspaces and no custom field types.
  Milestone dates are statements, and a dependency only informs.

## The exit journey

A small team with no tracker, driven through the API or `repomemo-console` (`$auth` is a token header, `$W` the workspace id).
Record what happened in the last column.

| # | Step | Do | Expect | Result |
|---|---|---|---|---|
| 1 | Brief | `POST /v1/workspaces/$W/projects`, then `POST /v1/projects/{id}/references` for a decision memory and a file | The brief lists both by title; a card and a file list the project as a backlink | pending |
| 2 | Plan | Make a milestone with a date and tasks with `project_id` and `milestone_id`; read `GET /v1/workspaces/$W/tasks/T-1` | Keys `T-1…`; the milestone says how many tasks are done, remain and are overdue, in words | pending |
| 3 | Discuss | Comment with an `@` mention; watch and mute | The mentioned person is told once; a muted task stops the optional notice and an assignment still arrives | pending |
| 4 | Block | `POST /v1/tasks/{id}/dependencies`; close the blocker | The explanation changes; the blocked task is not completed | pending |
| 5 | Conflict | Two `PUT /v1/tasks/{id}` with the same `expected_version` | One succeeds; the other is `409` with the current task and its own attempt | pending |
| 6 | Organize | Labels, a saved view, the board, a previewed bulk change | The board matches the list; a refused task stays as it was while the others change | pending |
| 7 | Remind | A task due yesterday and a weekly schedule; run `POST /v1/system/maintenance/run` twice | One notice for the task, one task for the occurrence, nothing twice | pending |
| 8 | Report | `POST /v1/projects/{id}/updates` choosing `at_risk` with a reason and links | The update shows who, why, the counts and what changed; the draft offered no health | pending |
| 9 | Move | Export the CSV, then the archive; import the archive as a new workspace | Same keys and relations; the schedule is paused; a forged archive is refused before anything is written | pending |

Then, once, by keyboard only at 200% zoom in both themes **when the screens exist**: the same journey in the web app.

## Corrections to the plan

[The plan](../plans/2026-11-17-0.7-projects.md) listed 0.7.13, 0.7.14, 0.7.18 and 0.7.19 as having no migration. As built: 0.7.13
and 0.7.14 have none; 0.7.18 added a ledger (**0079**, the retry-safe import); 0.7.19 changed the archive format to 4 instead of
adding a table. The tables of 0.7.6 (a version column), 0.7.8 (a parent column) and 0.7.11 (a state column) were columns on the
task, as planned.
