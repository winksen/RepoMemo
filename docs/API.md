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
- **Write preconditions.** Two writes can say which state they started from, and a write that started from an older state is refused with `409 conflict` instead of overwriting a newer one: a memory card update (`PUT /v1/memory-cards/{id}`) sends `expected_revision` (the newest revision you read), and a new file version (`POST /v1/artifacts/{id}/versions`) sends `X-RepoMemo-Expected-Updated-At` (the file's `updated_at` you read). In both the check is part of the write itself, so a change that lands between your read and your write is still caught. **A request that sends neither writes unchecked and overwrites whatever is there.** That unchecked mode exists today and is kept until the compatibility promise of V0.9.10, which will state whether it stays; a script that edits shared content should send the value. Restoring a revision or a version always adds a new one, so it overwrites nothing.
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
