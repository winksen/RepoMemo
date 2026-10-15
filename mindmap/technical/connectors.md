# Connectors (issues and pull requests)

> **Branch of:** [TECHNICAL_MINDMAP.md](../TECHNICAL_MINDMAP.md) · **Roadmap:** F17, decision 12 · **Status:** GitHub issues (V0.2.27) and pull requests (V0.2.28)

A connector keeps a tracker's items as evidence. It is a source of `type = 'connector'`: each item (an issue, or a pull request) becomes one markdown artifact, indexed like any file, so it is searchable, citable and linkable to memory cards. The tracker stays the source of truth: nothing is ever written to it.

## How a sync works

1. The connector asks the tracker for items **updated since the last one** (`since`, oldest update first), so an interrupted sync resumes where it stopped. At most 30 pages of 100 are read per sync; the report says when more is waiting.
2. For each item the comments are read (up to 500), the item is rendered as markdown (title, state, author, labels, assignees, link, body, discussion) and stored as a blob. A **pull request** also reads its own record, commits (up to 250), reviews and inline review comments (`render_pull`: branches, figures, commits, reviews, review comments, discussion); the commit ids go to the item's metadata. A sync stops after 500 requests and resumes from its cursor.
3. `connector_items` (migration 0034) maps the tracker's id to the artifact. **New** → a new artifact; **changed** → the same artifact with new content, queued for re-indexing (a *verified* one is flagged *needs review*); **unchanged** → nothing.
4. State drives the lifecycle: a **closed** item (an issue, or a pull request closed without merging) becomes *outdated*, a **reopened** one *active* again; a **merged** pull request stays active.
5. Changed artifacts are indexed in the same job, then the workspace's embeddings are requested.

Syncs run in the background through a runner of their own (one at a time, never two for the same connector), on demand (*Sync now*) and on the repository polling interval. A connector that failed waits for a person.

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
| POST | `/v1/workspaces/{id}/connectors/check` | A | what the tracker answers for a link and token; nothing stored |
| GET, DELETE | `/v1/connectors/{id}` | R / A | detail; remove with its items |
| POST | `/v1/connectors/{id}/sync` | W | queue a sync; 409 if one runs |
| PUT | `/v1/connectors/{id}/token` | A | replace the token, tried against the tracker first |

## Not yet

GitLab (V0.2.29), links from items to files, symbols and commits (V0.2.30), webhooks and a rate-limit pause (V0.2.31). A rate limit today fails the sync with a message and the next try resumes.
