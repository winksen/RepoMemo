# Connectors (issues and pull requests)

> **Branch of:** [TECHNICAL_MINDMAP.md](../TECHNICAL_MINDMAP.md) · **Roadmap:** F17, decision 12 · **Status:** V0.2.55. GitHub issues (V0.2.27) and pull requests (V0.2.28), GitLab issues and merge requests (V0.2.29), links to code (V0.2.30), webhooks and rate limits (V0.2.31) are shipped; V0.2.52 caps a tracker's answer at 20 MiB while it is read and remembers webhook delivery ids

A connector keeps a tracker's items as evidence. It is a source of `type = 'connector'`: each item (an issue, or a pull request) becomes one markdown artifact, indexed like any file, so it is searchable, citable and linkable to memory cards. The tracker stays the source of truth: nothing is ever written to it.

## How a sync works

1. The connector asks the tracker for items **updated since the last one** (`since`, oldest update first), so an interrupted sync resumes where it stopped. At most 30 pages of 100 are read per sync; the report says when more is waiting.
2. For each item the comments are read (up to 500), the item is rendered as markdown (title, state, author, labels, assignees, link, body, discussion) and stored as a blob. A **pull request** also reads its own record, commits (up to 250), reviews and inline review comments (`render_pull`: branches, figures, commits, reviews, review comments, discussion); the commit ids go to the item's metadata. A sync stops after 500 requests and resumes from its cursor.
3. `connector_items` (migration 0034) maps the tracker's id to the artifact. **New** → a new artifact; **changed** → the same artifact with new content, queued for re-indexing (a *verified* one is flagged *needs review*); **unchanged** → nothing.
4. State drives the lifecycle: a **closed** item (an issue, or a pull request closed without merging) becomes *outdated*, a **reopened** one *active* again; a **merged** pull request stays active.
5. Changed artifacts are indexed in the same job, then the workspace's embeddings are requested.

Syncs run in the background through a runner of their own (one at a time, never two for the same connector), on demand (*Sync now*) and on the repository polling interval. A connector that failed waits for a person.

## Links to code (V0.2.30)

`artifact_mentions` (migration 0035) records which file artifact an item's text names: a path (or a file name only one file has), a backticked symbol with at most three definitions, or a commit that is the last change of a file (by hash prefix of 7+ characters, or one of a pull request's own commits). `MentionIndex` is built from `repo_files` (path suffixes, `last_change_commit`) and `symbols`; `find_mentions` is a pure function. Mentions are recomputed for the workspace's items after a connector sync that changed something and after every repository sync, so order does not matter. `GET /v1/artifacts/{id}/mentions` returns `mentioned_in` and `mentions`.

## GitLab (V0.2.29)

`connectors/gitlab.rs` reads a project's `issues` and `merge_requests` (and their `notes`, `commits`) with `updated_after`, each with its own cursor (`cursor` and `pulls_cursor`). The project is addressed by its path with `/` as `%2F`, so groups and subgroups work, on gitlab.com or a self-hosted address. GitLab's records are reshaped into the GitHub-shaped ones that `render_issue` and `render_pull` read, so both providers give the same text, metadata and lifecycle; system notes are dropped and `DiffNote`s become review comments. The provider is chosen when connecting or told by the address (`gitlab` in the host name). The token goes in `PRIVATE-TOKEN`.

## Webhooks and rate limits (V0.2.31)

**Webhooks.** `repo_webhooks` serves repositories and connectors (its source id points at `sources`). `apps/server/src/webhooks.rs` has one `authenticate` (sender by header, constant-time signature or token check, one `401` for anything unverifiable, 20 failures a minute per address, 60 calls a minute per source, 1 MiB) and two receivers: `POST /v1/webhooks/repositories/{id}` (push events) and `POST /v1/webhooks/connectors/{id}` (GitHub `issues`, `issue_comment`, `pull_request*`; GitLab *Issue*, *Note* and *Merge Request* hooks; generic always). A verified call queues one sync.

**Pauses.** The client treats `429`, and `403` with no requests remaining, as a rate limit and reads `Retry-After`, `X-RateLimit-Reset` or `RateLimit-Reset`. A wait of 30 s or less is slept inside the request (twice at most); a longer one ends the sync with `Pause::RateLimited { seconds }` as the error's context, and the connector records `paused { reason, until }` and status `paused` (until capped at 24 h). A `401` is `Pause::TokenRefused`: paused with no end until the token is replaced. Background polling skips paused connectors (a rate-limit pause ends by itself when `until` passes), manual and webhook starts answer `409` while a rate-limit pause is in the future, and a successful sync or a new token clears the pause.

## Preview and approval (V0.8.1)

`check_tracker_link` returns a `ConnectorPreview`: the partition, what is copied and what is not, the audience (the workspace's name and member count) and `sharing` (`uniform` or `refused` with the reason). It holds no token. A GitLab project with confidential issues (`issues?confidential=true` finds one) is refused; a GitHub repository has no item-level restriction. `connect_tracker` takes `approved_by`: the route passes the administrator only when the request says `approve_sharing: true`, otherwise it is `400`; a refusal is `409` whatever the request says. The approver and time are kept in the connector's metadata (`sharing_approved_by`, `sharing_approved_at`), shown on `ConnectorSource`. See [the 0.8 plan](../plans/2026-11-18-0.8-connections.md).

## Quarantine (V0.8.2)

`connector_quarantine` (migration 0080, one row per connector: reason, detail, items held back, since) holds a connector's evidence back. `Denial` (`Unauthorized` for 401, `Forbidden` for a 403 with allowance left, `NotFound` for 404) rides on the tracker client's error beside `Pause`. When a sync ends in a denial, `confirm_access_loss` asks for the partition once more with the stored token: a second denial quarantines (reason `access_revoked`, `access_denied` or `not_found`) and sets the status `quarantined`; an answer means the refusal was transient and the sync only failed. The `NOT_QUARANTINED` condition is in `push_search_filters` (keyword, vector and the other searches that share it, so Ask and the assistant too), `list_artifacts`, `list_artifacts_page` (and its count), `list_artifacts_after`, the workspace export (artifacts and the citations kept) and `get_artifact` (which answers not found). A quarantined connector is not synced (409) and polling skips it. `POST /v1/connectors/{id}/release` (admin) reads the partition with the stored token; a denial is 409 and keeps it held, otherwise the row is deleted and the status is `ready`. Replacing the token does not release it.

**Stale-access window.** Access loss is found when a sync runs: at the repository polling interval (300 s by default, `REPOMEMO_REPO_POLL_SECONDS`), after a webhook, or on *Sync now*, plus the time of a sync in progress. With polling off, until the next webhook or manual sync. Loss of one item is not loss of access; deletion of one item is told by 0.8.6.

## Health (V0.8.3)

`ConnectorSource.health` is computed when the connector is read (no migration): `connector_health` takes the first state that applies, from the most serious: `quarantined`, `paused`, `syncing`, `failed` (status `error`), `pending` (never synced), `partial` (the last report has `more_to_fetch` or `index_failed`), `stale` (the last successful sync is older than 24 h, `STALE_AFTER_SECS`) and `complete`. It also carries `last_success_at` (`last_synced_at`), `last_complete_at` (new in the metadata: a sync that read and indexed everything), items kept and seen, the backlog flag, `resume_from` (the cursor), the exclusions (the comment, commit, page, request and text caps, attachments and code, GitLab system notes) and, for a failed or paused connector, a `problem` whose kind (`token`, `rate_limit`, `unreachable`, `not_found`, `forbidden`, `other`) is read from the client's own messages with the step that recovers. **Checkpoints:** after each page of items is stored, `checkpoint_connector` saves the cursor (GitHub, and GitLab per endpoint), so a sync interrupted by a failure or a restart resumes after that page; the items of the page that read again are found unchanged (`connector_items` is keyed by tracker id), never added twice.

## Outbound events (V0.8.4)

Migration 0081: `outbound_endpoints` (address, sealed secret, events, enabled) and `outbound_deliveries` (one event for one endpoint: event id, type, payload, status `pending`/`delivered`/`failed`/`cancelled`, attempts, next attempt, last status). The server's `outbound.rs` checks an address (the operator's `REPOMEMO_OUTBOUND_HOSTS`, `https`, no credentials, no metadata or link-local address) when it is added and again at every send, signs `{timestamp}.{body}` with HMAC-SHA256 (`X-RepoMemo-Signature: v1=…`), sends `X-RepoMemo-Event`, `-Event-Id`, `-Delivery`, `-Attempt` and `-Timestamp`, follows no redirect and times out at 10 s. A failure is retried after 30 s, 2 min, 10 min and 1 h, then `failed`. Turning an endpoint off or an address losing its approval cancels what was queued. The connector runner queues `connector.sync_completed`, `connector.sync_failed`, `connector.quarantined` and `connector.released`; payloads hold identifiers and counts only. See the [0.8 plan](../plans/2026-11-18-0.8-connections.md).

## Jira Cloud (V0.8.5)

`connectors/jira.rs` reads one project with `GET` requests only. The link is `https://site/browse/KEY` (or `/projects/KEY`, `/jira/software/projects/KEY/...`; a host ending `.atlassian.net` is told as Jira); the API is `/rest/api/3`; the token is `email:token` sent as basic authentication (`GitHub.jira`), stored sealed like any token. `check_jira` is the dry run: the project (`/project/KEY`), the permission (`/mypermissions`, Browse projects), the issue security scheme (`/project/KEY/issuesecuritylevelscheme`: levels mean **refused**) and a count of open issues, with no write. `sync_jira` reads `/search/jql` with `project = "KEY" AND updated >= "…" ORDER BY updated ASC` (cursor minus 26 h because JQL reads minutes in the token owner's time zone; repeats are unchanged by content hash), pages with `nextPageToken`, checkpoints each page, reads the first 100 comments of each issue and flattens Atlassian document format to text. The remote id is Jira's numeric `id` (`issue:10001`), so a rename or a move does not make a second copy; a closed status category is `closed`. An issue with `fields.security` is skipped and counted (`restricted_skipped`). Webhooks from Jira are not supported.

**Lifecycle (V0.8.6).** *Edits:* an issue read again with new content is `Updated` in place (same artifact, re-indexed, a verified one flagged for review). *Removals:* after a sync that reached the end of the project, `reconcile_jira` lists every issue of the project (ids and `security` only, paged, at most 30 pages) and takes out of the evidence (`remove_connector_item`: file, index and links) each stored issue that is no longer listed or that now carries a security level; the report counts them in `removed`. A list too long to read in one sync is not trusted, and neither is one that would remove every stored issue (it is left to the access check). *Access:* a denial of the project is the generic `Denial` path (confirmed twice, then quarantine); the rate limit is the generic `Pause`. *Repeated page:* `next_page_token` stops a sync whose token was seen before. *Ownership:* Jira owns status, assignee, priority and text; RepoMemo never writes back and never edits a copy, and a local follow-up is a separate task (0.8.8). The conformance fixtures of the 0.8 plan that this covers: `id-repeat`, `cur-resume`, `del-vs-fail`, `rate-pause`, `perm-lost`, `own-readonly` (tests in `tests/connectors_jira_lifecycle.rs`).

## Tracker context (V0.8.8)

No migration: the link is the existing `task_references` citation (and `project_references` for a brief). `TaskReference.remote` (`RemoteItemInfo`: provider, source, the tracker's `state`, URL, the tracker's update time and `owner: "remote"`) is read from `connector_items` and the artifact's metadata for an evidence citation of a mirrored item, and is empty (with `available: false`) when the item is held back by a quarantine. `GET /v1/artifacts/{id}/follow-ups` returns the remote state with the tasks (`tasks_following`) and projects (`list_projects_citing`) that cite the item; `POST /v1/artifacts/{id}/follow-up` (`follow_ups.rs`) makes a task through the normal creation path with the item cited, refusing a second open one unless `allow_another`. Remote state and local status never share a field: a connector item is never edited locally, and a task's status never reaches the tracker.

## Chat digests (V0.8.9)

Migration 0082: `chat_destinations` (provider `slack`, name, host, **sealed** address, digests, enabled, last sent and error) and `chat_digest_log` (primary key destination, kind, window: the ledger). `chat.rs` accepts an address only when the host is on `REPOMEMO_OUTBOUND_HOSTS` (the same list and check as outbound webhooks) and is `hooks.slack.com/services/...` (a loopback `http` address is accepted for a test receiver). `deliver_digests` counts (`count_due_reviews`, `count_overdue_tasks`), skips a zero, **claims** the (destination, kind, UTC day) row before the call, posts `{"text": …}` (no redirect, 10 s), and releases the claim when the call fails, so the next pass retries and a success is never repeated in a window. `digest_text` makes only a count and `state.mail.link(...)` (the signed-in app). The pass runs in the maintenance loop (`send_due`) and on `POST /v1/chat-destinations/{id}/digest`; an off or deleted destination is not read.

## Directory migration (V0.8.11)

Not a connector (the source is a `manual` one named `Import: <source_key>`), but the same rules of identity and safety. Migration 0083 `directory_imports` (workspace, source key, path → artifact, hash of the file as sent). `directory_import.rs`: `normalize_path` (relative only; no `..`, backslash, control character, hidden segment, `:`), `classify` (md, txt, png, jpg, gif, webp, pdf), `scan_links` (`[t](target)` and `![a](target)` outside fenced code; `[[wiki]]` counted) and `resolve_link` (file of the import, missing, unsafe, other). Commit: folders by `ensure_folder`; pass 1 stores or replaces each file as sent (`store_artifact`, or `replace_artifact_content` for a changed file so its id, comments and versions stay) and records it in the ledger; pass 2 rewrites resolved links to `/workspaces/{ws}/artifacts/{id}` (with the fragment) and replaces the content when it differs; touched files are queued for indexing. The route takes up to 40 MB of JSON (500 files, 5 MiB each, 25 MiB in all).

## Rules for the outside connection (S6)

| Rule | How |
|---|---|
| Hosts are named by the operator | The API host must be on `REPOMEMO_GIT_ALLOWED_HOSTS` (`api.github.com`, or the Enterprise host); unset, connectors are off. Link-local and metadata addresses are never allowed |
| `https` only | `http` is accepted for `localhost` and loopback addresses, for a tracker beside the server |
| The token goes to one host | Redirects are not followed, and a `Link: rel="next"` that leaves the API's address is refused |
| Tokens are secrets | Stored sealed in `repo_credentials` (the same store as repository tokens), never returned or logged |
| Answers are bounded | 30 s per request, 20 MiB per response, comment and body text cut off |

## Data

| Table | Meaning |
|---|---|
| `sources` (`type = 'connector'`) | `root_uri` is the normalised repository address; metadata holds the provider, API base, owner, repository, cursor, last report and last error |
| `connector_items` | `(source_id, external_id)` → artifact, kind, state, content hash, the tracker's update time |
| `repo_credentials` | The token, bound to the source id |

## API

| Method | Path | Guard | Notes |
|---|---|---|---|
| GET, POST | `/v1/workspaces/{id}/connectors` | R / A | list; connect (`{link, name?, token?}`) and start the first sync |
| POST | `/v1/workspaces/{id}/connectors/check` | A | what the tracker answers for a link and token, with the import preview (V0.8.1); nothing stored |
| GET, DELETE | `/v1/connectors/{id}` | R / A | detail; remove with its items |
| POST | `/v1/connectors/{id}/sync` | W | queue a sync; 409 if one runs |
| POST | `/v1/connectors/{id}/release` | A | end a quarantine once the source answers again (V0.8.2); 409 while it is still denied |
| PUT | `/v1/connectors/{id}/token` | A | replace the token, tried against the tracker first; clears a pause |
| POST, DELETE | `/v1/connectors/{id}/webhook` | A | make or renew (secret shown once) or remove the webhook secret |
| POST | `/v1/webhooks/connectors/{id}` | pub | a signed tracker event queues one sync (`202 {queued, note}`; `ping` `200`), anything else `401` |

## Not yet

Tokens other than a personal one (a GitHub App). Webhook delivery logs and redelivery exist since V0.3.22; a pull or merge request's approvals are read since V0.3.21.
