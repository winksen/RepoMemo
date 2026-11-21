# Using the API from scripts

How to read and write RepoMemo from a script or CI job with a personal access token. Every route, with its method, path, guard and body type, is in the [API reference](../mindmap/technical/api-reference.md); this page is the route through it for a person with a token. Who may call what is in [Authentication and authorization](../mindmap/technical/auth-and-authorization.md).

## 1. Get a token

Tokens are made by a person signed in to the browser (never by another token): *Profile › API tokens*, or `POST /v1/profile/tokens` with a session.

| Scope | A script with it can |
|---|---|
| `read` | Look and search: `GET`, and the `POST` routes that only read (search, ask, query) |
| `write` | Also do content work: add evidence, tasks, comments, memory cards |
| `admin` | Also change who has access or how a workspace is set up: invitations, roles, deleting, AI providers, repository and connector links, webhooks, export and import, the approval rule, removing an item from the trash for good, answering search requests and saving organization templates. Offered with a warning |
| `system` | Administrators only: also change things in the System area |

A token lives 7 to 365 days, is shown **once**, is stored only as a hash, and is revoked at any time from the same page. Choose the narrowest scope that works. It acts as you: it can never do more than your role in a workspace allows, and a viewer's `write` token still cannot write.

```bash
export REPOMEMO_URL=https://memo.example.com
export REPOMEMO_TOKEN=rmp_...            # shown once, when created
auth="Authorization: Bearer $REPOMEMO_TOKEN"
```

## 2. Conventions

- JSON in and out: `Content-Type: application/json`. Routes are under `/v1`.
- **Errors** are `{"error": {"code": "...", "message": "..."}}`, with `bad_request` (400), `unauthorized` (401), `forbidden` (403), `not_found` (404), `conflict` (409), `rate_limited` (429, with `Retry-After`) and `internal_error` (500). Quota codes include `workspace_full` (413), `workspace_limit` (409) and `disk_low` (507). A read-only token on a write route is refused with `403`.
- **Paging.** A memory card's revisions (`GET /v1/memory-cards/{id}/revisions`) come newest first, 50 by default (`?limit=`, at most 200); `?before=<revision number>` continues below it, the next position is in `X-Next-Cursor` and the number of revisions in `X-Total-Count`. Artifact lists accept `?limit=&offset=` or `?limit=&cursor=<last id>`; the next position is in the `X-Next-Cursor` header and the total in `X-Total-Count`. Workspace and System job lists accept a cursor too. Prefer the cursor for long lists.
- **Search budget.** Workspace search stops after 5 seconds and cross-workspace search after 10; the `X-RepoMemo-Query-State` header says `complete` or `timed_out`, and a timeout returns no results rather than claiming full coverage. Query text is capped at 512 characters.
- **Limits.** Heavy routes (search, reports) are limited per person (`REPOMEMO_HEAVY_REQUESTS_PER_MINUTE`, 240) and AI routes per hour (120). On `429` or `503` wait for `Retry-After`.
- **Write preconditions.** Two writes can say which state they started from, and a write that started from an older state is refused with `409 conflict` instead of overwriting a newer one: a memory card update (`PUT /v1/memory-cards/{id}`) sends `expected_revision` (the newest revision you read), and a new file version (`POST /v1/artifacts/{id}/versions`) sends `X-RepoMemo-Expected-Updated-At` (the file's `updated_at` you read). In both the check is part of the write itself, so a change that lands between your read and your write is still caught. A task update (`PUT /v1/tasks/{id}`) sends `expected_version` (the task's `version`). **A request that sends none of these writes unchecked and overwrites whatever is there.** That unchecked mode exists today and is kept until the compatibility promise of V0.9.19, which will state whether it stays; a script that edits shared content should send the value. Restoring a revision or a version always adds a new one, so it overwrites nothing.
- **Uploads** send the file bytes as the body with `X-RepoMemo-Filename` (percent-encoded) and, optionally, `X-RepoMemo-Folder-Id`; the limit is `REPOMEMO_MAX_UPLOAD_MB` (10).

## 3. A read workflow: find a workspace and search it

```bash
# 1. The workspaces you can read
curl -s -H "$auth" "$REPOMEMO_URL/v1/workspaces"

# 2. Search one of them (keyword, and by meaning if the workspace has an embedding provider)
curl -s -H "$auth" -H 'Content-Type: application/json' \
  -d '{"query":"failover runbook","limit":5}' \
  "$REPOMEMO_URL/v1/workspaces/$WORKSPACE_ID/search"

# 3. List its evidence, one page at a time
curl -si -H "$auth" "$REPOMEMO_URL/v1/workspaces/$WORKSPACE_ID/artifacts?limit=50"
# read X-Next-Cursor, then:
curl -si -H "$auth" "$REPOMEMO_URL/v1/workspaces/$WORKSPACE_ID/artifacts?limit=50&cursor=$NEXT"

# 4. One item with its detail
curl -s -H "$auth" "$REPOMEMO_URL/v1/artifacts/$ARTIFACT_ID"
```

**Expected:** step 1 returns an array with your workspaces' `id`; step 2 returns results with the title, path, snippet and line range. An item with no match is simply absent. Ask a cited question the same way, with `POST /v1/workspaces/{id}/ask` and `{"question":"…"}`: it answers *"Indexed context is insufficient"* without calling the AI when nothing matches, and needs a workspace AI provider you are allowed to use.

## 4. A write workflow: add a note, a task and a memory card

This needs a `write` token and a `member` role or above in the workspace.

```bash
# 1. A note, as a text file
curl -s -H "$auth" -H 'Content-Type: application/json' \
  -d '{"title":"failover-notes.md","content":"# Failover\nRestart the pool after a switch-over."}' \
  "$REPOMEMO_URL/v1/workspaces/$WORKSPACE_ID/artifacts/text"

# 2. Or upload a file
curl -s -H "$auth" -H 'X-RepoMemo-Filename: failover.md' \
  --data-binary @failover.md \
  "$REPOMEMO_URL/v1/workspaces/$WORKSPACE_ID/artifacts/upload"

# 3. A task linked to the evidence (status: open, in_progress, blocked, done; priority: low, medium, high, urgent)
curl -s -H "$auth" -H 'Content-Type: application/json' \
  -d '{"title":"Check step 4","description":"","status":"open","priority":"medium","artifact_id":"'"$ARTIFACT_ID"'"}' \
  "$REPOMEMO_URL/v1/workspaces/$WORKSPACE_ID/tasks"

# 4. A memory card
curl -s -H "$auth" -H 'Content-Type: application/json' \
  -d '{"title":"Restart the pool after a failover","body_markdown":"Agreed in the Oct review.","source":"script"}' \
  "$REPOMEMO_URL/v1/workspaces/$WORKSPACE_ID/memory-cards"
```

**Expected:** each call returns the created object with its `id`. The note or file appears in the workspace with *Indexing…*, then *Indexed*, and a search for a word in it finds it within a minute or so. The same file uploaded twice is stored once. The task is on the board; the card is under *Memory*. Check by `GET` on the list route of each. A viewer's token answers `403`; a token with `read` scope answers `403`.

## 4a. Projects (0.7)

A project is a named piece of work **inside one workspace**: a name, a goal, a lifecycle (`active`, `on_hold`, `done`) and an optional target date (`YYYY-MM-DD`), with an optional owner who must be a member. It grants no access: whoever can read the workspace can read its projects, and a role that may write content can change them (a viewer's token answers `403`).

```bash
# Create a project, list the active ones, archive it (the archived are left out unless you ask: ?status=archived)
curl -s -X POST -H "$auth" -H "Content-Type: application/json" \
  -d '{"name":"Queue migration","goal":"Move the worker to the managed queue.","target_date":"2027-02-01"}' \
  "$REPOMEMO_URL/v1/workspaces/$WORKSPACE_ID/projects"
curl -s -H "$auth" "$REPOMEMO_URL/v1/workspaces/$WORKSPACE_ID/projects"
curl -s -X POST -H "$auth" "$REPOMEMO_URL/v1/projects/$PROJECT_ID/archive"
```

A task joins a project with `project_id` on the task routes. **Left out, a save keeps the task's project; an empty string removes it.** The project must belong to the task's workspace (`400` otherwise), and an archived project keeps its tasks but takes no new ones (`409`; restore it first with `POST /v1/projects/{id}/restore`). Tasks made without a project work as before. Each project reports its `task_count` and `open_task_count`.

**The brief.** `GET /v1/projects/{id}/brief` returns the project with the decision memories and evidence it cites. Cite with `POST /v1/projects/{id}/references` (`{"kind":"memory_card"|"evidence","target_id":"…","note":"…"}`, a note up to 500 characters, at most 100 citations) and remove with `DELETE …/references/{reference_id}`. Nothing is copied: a citation holds the id and your note, and the title is read from the target each time, so a deleted target comes back with `"available": false` and no title. A target of another workspace answers `404`; citing the same item twice answers `409`. `GET /v1/workspaces/{id}/projects/citing?kind=memory_card&target_id=…` is the backlink: the projects (not archived) whose brief cites that card or file.

**Task identifiers.** Every task has a `number` that is unique in its workspace and never reused, and a `key` built from it (`T-12`). A title change does not change them. `GET /v1/workspaces/{id}/tasks/T-12` (also `12`, or the task's id) returns the task, its checklist and its web address (`path`); anyone who can read the workspace gets it, a guest included. A task that was **deleted** answers `410` with "Task T-12 was deleted."; one that never existed answers `404`. Tasks made before 0.7 were numbered in the order they were created.

**Task discussion.** `POST /v1/tasks/{id}/comments` (`{"body":"…"}`, 1 to 5,000 characters; a role that may write) adds a comment that stays with its task; `GET` lists them, `PUT /v1/task-comments/{id}` edits one (its author) and `DELETE` removes it (its author or an administrator). Type `@` and a member's email to mention them: they are told once (a later edit tells only the people it adds), and are not also told "new comment". **Watching** is `PUT /v1/tasks/{id}/watch` with `{"mode":"watching"|"muted"|"default"}`, open to anyone who can read the workspace: a task's creator and assignee follow it by default, anyone else by asking, and `muted` stops the optional "new comment" notices (a choice of its own under the profile's notifications) but never the notice that a task was assigned to you.

**Task context.** A task cites any number (up to 50) of evidence files and memory cards of its own workspace: `POST /v1/tasks/{id}/references` with `{"kind":"evidence"|"memory_card","target_id":"…","note":"…"}`, `GET` to list and `DELETE …/references/{reference_id}` to remove. When you create a task you can pass the same objects as `"references"` (for example the decision that created it); every one is checked first, so a bad one means no task. The task's older single `artifact_id` link keeps working and is one of the references. As for projects, a reference holds the id and your note, the title is read each time, and a deleted target comes back with `"available": false`. The task detail lists them.

**Safe task edits.** Every task has a `version` that starts at 1 and grows by one with each save. Send it back as `expected_version` on `PUT /v1/tasks/{id}`: if the task changed since, nothing is written and the answer is `409` with `error` (the usual `code` and `message`), `current` (the task as it is now, with its new `version`) and `attempted` (the title, description, status, priority, assignee, file, project and due date you sent), so a client can show both and let the person keep their change on top of the current task. The check is part of the write, so two saves from the same version cannot both succeed. Without `expected_version` the save is unchecked, as before.

**Milestones.** `POST /v1/projects/{id}/milestones` (`{"name":"…","due_date":"YYYY-MM-DD","description":"…","completion_rule":"all_tasks"|"manual"}`) groups the project's tasks; a task joins with `milestone_id` on the task routes (left out keeps it, an empty string removes it, and it must be a milestone of the project the task is in; moving the task to another project drops it). Each milestone reports `task_count` (the denominator), `done_count`, `remaining_count`, `overdue_task_count` (tasks not done and past their own due date), `overdue` (the milestone's date has passed and it is not complete), `completed` and an `explanation` in words, such as "1 of 3 tasks are done and 2 remain, 1 of them overdue." The rule `all_tasks` completes it when it has tasks and every one is done; `manual` is completed and reopened by a person (`POST …/complete`, `…/reopen`; `409` for the other rule). **Dates are statements:** changing a milestone's date, or completing it, moves no task and completes no task. Dates and times are in UTC; a task due on a date alone (`2027-02-01`) is overdue from the next day. Removing a milestone leaves its tasks in the project.

**Sub-tasks.** A task can have a parent: send `parent_task_id` on the task routes (left out keeps it, an empty string removes it). The parent must be a task of the same workspace (`400` otherwise), not the task itself or anything below it (a cycle is refused), and the tree is at most five levels deep. A sub-task is a task of its own with its own assignee, status and discussion, unlike a checklist step. A task reports `parent_task_id`, `parent_key`, `child_count` and `open_child_count`; `GET /v1/tasks/{id}/children` lists the direct sub-tasks (also in the task detail). **Completing a parent does not complete or hide its sub-tasks**: they stay open and listed, and the count of unfinished ones stays on the parent. Removing a parent leaves its sub-tasks as ordinary tasks.

**Dependencies.** `POST /v1/tasks/{id}/dependencies` with `{"blocker_task_id":"…"}` (this task is blocked by that one) or `{"blocked_task_id":"…"}` (this task blocks that one); name exactly one. Both tasks must belong to the workspace; a task cannot block itself, the same link twice is `409`, and a link that would close a cycle, directly or through other tasks, is `400` with the reason. `GET` returns `blocked_by`, `blocks` and an `explanation` in words ("Blocked by T-1 Backfill (open).", or "Not blocked: 1 of its blockers is done."); a task also reports `open_blocker_count` and `blocks_count`. **A dependency only informs:** closing a blocker changes what the blocked task shows and never its status, and a deleted task takes its links with it. Remove one with `DELETE /v1/task-dependencies/{id}`.

**Labels.** A workspace names its kinds of work: `POST /v1/workspaces/{id}/task-labels` with `{"name":"Bug","color":"red"}` (a name up to 40 characters, unique whatever its case; a colour from gray, red, orange, yellow, green, teal, blue, purple, pink, each with a `background_color` and a `text_color` that reach a contrast of 4.5 to 1). Anyone who can edit creates, renames and recolours a label and sets a task's labels (`PUT /v1/tasks/{id}/labels` with `{"label_ids":[…]}`, at most ten); only an owner or administrator deletes a label or merges one into another (`POST /v1/task-labels/{id}/merge` with `{"into_label_id":"…"}`). Renaming and merging keep every assignment (a task that had both keeps one). `GET …/task-labels` lists them with `task_count`, and `GET …/tasks?label={id}` lists the tasks that carry one. **A label organizes and never grants or limits access**: no route reads one to decide who can see or change a task.

**Workflow states.** A task's `status` is always one of four stable **categories**: `open`, `in_progress`, `blocked`, `done`. `GET /v1/workspaces/{id}/workflow` lists the states with their category and task count: the four built-in ones (their `id` is the category, they always exist and cannot be changed, so existing tasks keep their status unchanged) and any the workspace named. An owner or administrator names one with `POST …/workflow/states` (`{"name":"Review","category":"in_progress"}`, a name up to 40 characters, unique whatever its case and not a built-in name, at most 20), renames or re-maps it with `PUT /v1/workflow-states/{id}` (its tasks follow the new category) and removes it with `DELETE`. **Removal needs a place for the tasks:** `409` while any are in the state, unless `?move_to=` names a built-in category or another state of the workspace; moved tasks take that state's category (entering `done` sets their completion time, leaving it clears it) and count as a new version. A task save names a state with `state_id` (a built-in id or a named state; its category becomes the status, and a `status` that disagrees is `400`); a save with only `status` keeps the named state while it still matches and otherwise puts the task in the built-in one. **Reports, the board and integrations read the category, never the name.**

**The task list.** `GET /v1/workspaces/{id}/task-list` reads a page: `limit` (1 to 200, default 50) and `offset`, and the answer gives `tasks`, `total` (everything that matches) and `next_offset` (none on the last page), so a large list is never downloaded whole. Filters, all optional and combinable: `project_id` and `milestone_id` (or `none`), `assignee` (a user id, `me` or `none`), `priority` and `status` (comma-separated), `label`, `due_before` and `due_after` (`YYYY-MM-DD`, inclusive), `overdue=true` (not done and past its own due date), `q` (words in the title) and `sort` (`priority`, the usual order; `due`; `updated`; `number`). Every order ends on the task number, so pages do not shuffle. **Saved views** (`/v1/workspaces/{id}/task-views`) store a set of these filters under a name, never task ids or rights: `?view={id}` applies one, any other parameter replaces the view's own, `me` means whoever reads it, and each use is checked against the reader's *current* access, so someone who has lost the workspace gets `403` and nothing else. A view is private to its owner until they share it (`PUT …/sharing` with `{"shared":true}`); a shared view is read by everyone in the workspace and changed only by its owner. Anyone who can read the workspace can keep views. The unpaged `GET …/tasks` still works.

**The board.** `GET /v1/workspaces/{id}/task-board` takes the **same filters, view and sort** as the task list and returns one column for each workflow state (the four built-in ones, then the workspace's named ones), each with its `total` and its first `column_limit` tasks (1 to 100, default 25) and a `next_offset`; the rest of a column is the list with `?state={column id}&offset=…`. Because both read one query, the board and the list show the same filtered work: every task is in exactly one column, and its status is that column's category. `POST /v1/tasks/{id}/move` with `{"state_id":"blocked","expected_version":3}` moves a card: it is the usual task save (it changes nothing but the column), so it needs a role that may write, adds a version and is refused with `409` and the task as it is now when `expected_version` is out of date, or `400` for a column that does not exist. A client that moved the card first puts it back on any refusal and says why.

**Bulk changes.** `POST /v1/workspaces/{id}/tasks/bulk` applies one change to up to 100 tasks: `{"task_ids":[…],"changes":{"assignee_user_id":"…"|"","add_label_ids":[…],"remove_label_ids":[…],"milestone_id":"…"|""},"expected_versions":{"<task id>":3},"dry_run":true}`. An empty string unassigns or removes the milestone; a left-out field changes nothing. **`dry_run` is the preview**: it runs the same checks and writes nothing. The answer holds an `items` entry for every task, with `outcome` `ok` (changed, or "would be changed" in a preview), `unchanged` or `refused`, a `reason` and, once applied, the task after the change, and the counts `succeeded`, `refused` and `unchanged`. **Each task is checked for itself:** it must be a task of this workspace (another workspace's is "not found"), its `expected_versions` entry (when sent) must still be current, a milestone must be in the task's project and a task may carry ten labels at most; a refused task is left exactly as it was and the others still go through. What is wrong for every task at once (no change named, a label that is both added and removed, an assignee who is not a member, a milestone or label of another workspace, more than 100 tasks) is `400`, and a reader gets `403`. A new assignee is told once for each task assigned.

**Due reminders.** The person a task is assigned to is told in their inbox (and by mail, and in the daily digest, as they chose) when it falls due and again when it is overdue. A new choice, **A task assigned to you is due or overdue**, sits under the profile's notification preferences (kind `task_due`; in-app, mail, both or none, both by default). **Time is UTC.** A due date without a time (`2027-02-01`) is due from 00:00 UTC that day and overdue from 00:00 UTC the next; a due time is due from 24 hours before it and overdue once it has passed. A task that is done, has no assignee or whose assignee left, was deactivated or lost access gets none. Each reminder is sent **once for each task, person, kind and due value**: a repeated maintenance pass, a retry and a restart send nothing twice, and moving the due date allows one new reminder. The notices have the types `task_due` and `task_overdue` and lead to the task's page. The reminders go out in the periodic maintenance pass (`POST /v1/system/maintenance/run` runs it by hand).

**Recurring tasks.** `POST /v1/workspaces/{id}/recurring-tasks` makes a task again on an explicit schedule: `{"title":"Stand-up notes","priority":"low","assignee_user_id":"…","project_id":"…","due_after_days":2,"missed_policy":"latest","paused":false,"schedule":{"kind":"weekly","interval":1,"weekday":0,"start_date":"2027-01-04"}}`. The schedule is `daily`, `weekly` (on `weekday` 0 Monday to 6 Sunday) or `monthly` (on `day_of_month` 1 to 28, so every month has it), every `interval` (1 to 60) days, weeks or months from `start_date`; dates are calendar dates in UTC. A schedule answers with its `next_occurrences`, so it can be checked before it runs. **Each occurrence is a date, and (recurring task, date) is its identity:** the maintenance pass makes the tasks that are owed, one for each occurrence, and a repeated pass, a retry or a restart makes none again, because the occurrence record, the task and the date looked at are written in one transaction. **Missed runs:** when several occurrences were owed (the server was off, or a schedule starts in the past), `latest` (the default) makes only the newest and records the others as skipped, and `skip` makes one only on its own day; either way nothing is flooded. **Paused** (`POST …/pause`) makes nothing, and `…/resume` makes nothing up for the time paused. **`…/stop` is final and keeps history:** the tasks it made stay, and `GET /v1/recurring-tasks/{id}` still lists every occurrence (made or skipped, with its task). A stopped schedule is not edited (`409`); an edit to an active one never makes the past again. Each task made is open, has the schedule's assignee and project, is due `due_after_days` after its occurrence date and says in its description which date made it; its assignee is told as for any assignment. Anyone who can read the workspace sees the schedules; a role that may write content changes them; a workspace has at most 50 that are not stopped.

**Project updates.** A person says where a project stands: `POST /v1/projects/{id}/updates` with `{"health":"on_track"|"at_risk"|"blocked","summary":"what changed","reason":"why this health","milestone_ids":[…],"task_ids":[…],"evidence_ids":[…],"memory_card_ids":[…]}` (summary and reason 1 to 2,000 characters each, at most 30 links). **The health is always the author's choice** and is required: nothing in the product computes or defaults it, and `GET /v1/projects/{id}/update-draft` only offers counts, milestones and the blocked tasks as a starting point, with no health in it. The update is attributed to whoever published it (a role that may write content), is checked first (milestones must be the project's, tasks, evidence and cards the workspace's) and is never edited. It keeps `snapshot` (the task counts and milestones as they were) and `changes` (what differs from the update before: the health, the counts, milestones added, completed, moved or removed), so a reader can see later what changed and why. Each link keeps its `label` as published, with the `title` now and `available: false` when the target is gone. A project reports the `health` of its newest update that was not retracted. `POST /v1/project-updates/{id}/retract` (its author or an administrator) hides an update from the list (`?include=retracted` shows it) and keeps the record with who retracted it. Anyone who can read the workspace reads updates; an archived project takes no new one (`409`).

**CSV out and in.** `GET /v1/workspaces/{id}/tasks/export.csv` (the task list's filters and views apply) is a UTF-8 CSV with a byte order mark and the columns key, title, description, status, priority, state, assignee_email, due_at, project, milestone, labels, created_at and completed_at. **Cells are safe to open in a spreadsheet:** text that starts with `=`, `+`, `-`, `@`, a tab or a carriage return is written with a leading apostrophe so it stays text (the import removes it again), and commas, quotes and line breaks are quoted. `POST /v1/workspaces/{id}/tasks/import` takes `{"csv":"…","mapping":{"title":"Summary","status":"State","assignee":"Owner email","due_at":"Due","project":"Area","labels":"Tags","description":"Details","priority":"Priority","external_id":"Issue ID"},"dry_run":true,"skip_invalid":false,"import_key":"…"}` (up to 1,000 rows and about 2 MB). Fields left out of the mapping are found by their own name or a common one. **`dry_run` is the default**: the report gives `mapping` (which column fed which field), `rows`, `valid`, `invalid`, `duplicates`, the `unknown_assignees` and an entry for every row with its `problems` in words (an empty title, a status that is not open, in progress, blocked, done or one of the workspace's states, a date that is not a date, an assignee who is not a member, an unknown project or label), and writes nothing. With `dry_run: false` one invalid row refuses the whole import (`409`, nothing written) unless `skip_invalid` is true. **A retry creates nothing twice:** a row is identified by its id in the file (`external_id`) or, without one, by what it says within the import, and each row is written in one transaction with its entry in a ledger, so a retry, a second attempt after fixing other rows and a restart skip what was imported; a row whose id is the key of a task here (`T-12`, from a file exported from this workspace) is that task already. Imported tasks are made by the person who imports and do not send assignment notices.

**Moving projects with an archive.** A workspace archive (`GET /v1/workspaces/{id}/export`, format 4) now also carries the work of the 0.7 line in a `work` section: projects with their brief citations, milestones, task labels, named workflow states, each task's **number**, project, milestone, parent, labels, state, citations, comments and the tasks that block it, recurring schedules with their occurrence history, and project updates with their links. Everything is named by the archive's own ids and by name and address, never by account id. **Importing it** (`POST /v1/workspaces/import`, which checks first and can be a `dry_run`) is refused, with the reasons, when a record points at something the archive does not hold (a task in a project that is not there, a milestone of another project, a parent or blocker that is missing or loops, a schedule this server cannot run), so nothing can point at a record of another workspace; a citation of a file or card that was left out is dropped with a warning. Tasks keep their number and the counter continues above the highest number ever given out. **A recurring schedule always comes back paused** (or stopped, if it was) and makes nothing until it is resumed; an occurrence that was already made is not made again. Comments and updates come back credited to the importer with the original author named in the text, and an update keeps the health its author chose. An archive older than format 4 imports as before.

## 5. Live updates

`GET /v1/workspaces/{id}/events` (and `GET /v1/system/events` for system administrators) is a **server-sent events** stream, so use `curl -N`:

```bash
curl -N -H "$auth" "$REPOMEMO_URL/v1/workspaces/$WORKSPACE_ID/events"
```

The events are `job` (a background job changed, for example indexing finished), `activity` (something happened in the workspace) and `resync` (re-read the lists you hold). A reader who is not an owner or administrator receives `changed` in place of `activity`, which says what kind of thing changed and not who changed it. Streams are long-lived and exempt from the request timeout; a person may hold `REPOMEMO_MAX_EVENT_STREAMS_PER_USER` open. If you miss events, re-list rather than assume.

## 6. Which routes are for scripts

The reference marks each section by **audience**:

| Audience | Sections | Use from scripts? |
|---|---|---|
| Workspace content | Dashboard and workspaces; Evidence, folders and files; Retrieval, memory and assistant; Collaboration; Jobs and events | **Yes**, with a token of the scope the route needs |
| Administration | Organizations; Workspace settings and AI; Repositories; System administration | Only with `admin` or `system` scope, for the operator who owns them |
| Web and desktop client | Sign-in, sessions and access links (cookies, refresh, invitations); Profile and notifications | **No.** They exist for the apps and use sessions |
| Internal | Health and setup (`/v1/setup/*`, `/v1/platform/*`), the System console route, avatars, webhook receivers (called by Git hosts with a signed secret) | **No.** `/health` and `/health/ready` are fine for probes; the rest may change between releases |

Until 1.0, any route can change between minor releases. The [changelog](../CHANGELOG.md) says so when one does.

## 7. The command console

`repomemo-console` is the same API as a command line, for administrators; it reads `REPOMEMO_TOKEN`. Commands that change something preview their effect until you add `--yes`. See [Admin console](../mindmap/technical/admin-console.md). The Postman collections in `docs/api/` are old and are not maintained.
