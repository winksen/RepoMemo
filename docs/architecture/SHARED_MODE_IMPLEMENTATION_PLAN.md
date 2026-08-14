# Shared Mode Implementation Plan

Date: 2026-08-14

## Decision

RepoMemo will support two explicit workspace modes:

```text
Local workspace
  Desktop app -> Tauri commands -> local SQLite + local blobs

Shared workspace
  Desktop app or web client -> Rust HTTP API -> shared data plane
```

The desktop app remains a first-class client. It must not silently treat local
SQLite data as shared data. Shared workspaces are authoritative on the server.

The implementation remains one Rust monorepo, initially with two deployables:

```text
apps/server  HTTP API, session/auth boundary, permissions, workspace operations
apps/worker  ingestion, indexing, embeddings, connector synchronization
```

The existing Rust crates remain the reusable domain and processing core. This is
not a new orchestration backend in another language.

## Current Foundation

This change introduces a runnable server foundation and worker process:

- `GET /health` identifies an available shared-backend instance.
- `GET /v1/session` returns a dummy session for local development.
- `x-repomemo-user-id` and `x-repomemo-user-name` can override the demo user.
- the worker has an independent lifecycle and is ready to claim durable jobs.
- shared client identity contracts live in `repomemo-domain`.

This is deliberately not yet a collaboration data plane. The current endpoint
does not read or write the desktop SQLite database, and the worker does not yet
claim jobs. That prevents accidentally presenting local single-user storage as
shared, permission-aware storage.

Run the foundation locally:

```powershell
$env:REPOMEMO_ALLOW_DUMMY_SESSIONS = 'true'
cargo run -p repomemo-server
cargo run -p repomemo-worker
Invoke-RestMethod http://127.0.0.1:8787/health
Invoke-RestMethod http://127.0.0.1:8787/v1/session
```

Dummy sessions require `REPOMEMO_ALLOW_DUMMY_SESSIONS=true`. The server rejects
them by default so an externally bound deployment cannot accidentally use demo
identity.

The desktop application continues to run unchanged with `npm.cmd run dev`.

## Delivery Sequence

### 1. Shared data plane

Add a PostgreSQL-backed server storage implementation and migrations. Start
with canonical organization/workspace records rather than trying to migrate all
local data at once.

```text
organizations
users
organization_memberships
workspaces
workspace_memberships
audit_events
jobs
```

Exit criteria:

- a shared workspace and membership survive server restart;
- every workspace query is scoped to an authenticated member;
- audit records exist for membership and workspace changes.

### 2. Dummy sessions become a server identity boundary

Keep the `SharedSession` response shape. Replace only the identity issuer:

```text
now:     development headers -> dummy session
later:   OIDC/session token -> validated user and memberships
```

Do not put authorization decisions in the desktop or web client. The server
must decide membership and role on every shared-workspace request.

### 3. Shared workspace API

Implement versioned endpoints in this order:

```text
GET/POST /v1/workspaces
GET/PATCH /v1/workspaces/{workspaceId}
GET/PUT/DELETE /v1/workspaces/{workspaceId}/members
GET     /v1/workspaces/{workspaceId}/jobs
```

Publish an OpenAPI document and generate the TypeScript client used by both the
future web UI and the shared-mode desktop adapter. Keep response DTOs separate
from SQL rows.

### 4. Shared blobs and artifact ingestion

Add a blob-storage trait with local-filesystem and S3-compatible adapters.
Uploads should stream from clients to object storage through an authorized
server endpoint; a server record and a durable ingest job are committed before
the worker begins processing.

### 5. Durable worker jobs

Use PostgreSQL initially for job persistence and claiming. Each job has a
workspace ID, initiating user, payload, retry policy, state, progress, and
failure reason.

```text
API validates membership -> stores job -> worker claims job
  -> emits progress -> completes/fails job -> API exposes status
```

The first job kinds are `ingest`, `index`, `embed`, and `reindex`. Add Redis or
another broker only after PostgreSQL claiming becomes an observed limitation.

### 6. Permission-aware retrieval

Move the existing retrieval logic behind a server service that first verifies
the caller's workspace membership. Apply the permission filter before chunks,
citations, or AI context are selected.

This is a security boundary: filtering only the final answer is insufficient.

### 7. Client adoption

Add an API-client abstraction to the React desktop app:

```text
LocalWorkspaceClient  -> current Tauri command bridge
SharedWorkspaceClient -> generated HTTP client
```

The UI must visibly identify the selected workspace as Local or Shared. Start
with a thin online client; defer offline cache and conflict resolution.

### 8. Web client and production hardening

Add a web client once the shared workspace API is stable. Before real-user
rollout, complete authentication, TLS, secret management, rate limits, request
tracing, backups, migrations, monitoring, and deployment documentation.

## Non-Goals For This First Server Slice

- migrating desktop-local workspaces to the server;
- production authentication or session cookies;
- PostgreSQL or object-storage implementation;
- making every existing desktop command remotely callable;
- offline synchronization;
- splitting into further microservices.

These restrictions intentionally keep the first boundary small and make the
next implementation milestones independently testable.
