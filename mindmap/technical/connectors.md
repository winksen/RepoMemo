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
