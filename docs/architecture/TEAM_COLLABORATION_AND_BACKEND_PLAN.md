# Team Collaboration And Backend Architecture Plan

Date: 2026-06-17

## Context

RepoMemo currently follows a local-first desktop architecture:

- Tauri desktop app
- React and TypeScript UI
- Rust in-process core
- Rust workspace crates for domain, storage, ingestion, indexing, and API facade
- SQLite metadata database in the local app data directory
- Local filesystem blob storage
- SQLite FTS and local embedded vector storage planned before external vector services

This is a good shape for proving the single-user memory loop:

```text
create workspace -> import files -> store artifacts -> index chunks -> search -> ask with citations -> save memory
```

However, the current architecture does not yet provide real shared team collaboration. Each installation owns its own local database and local blob store. That means two team members can import the same repository or documents, but they are not actually working in the same shared workspace unless we add a shared authority for data, identity, permissions, and sync.

## Short Answer

Yes, we should review and extend the architecture before positioning RepoMemo as a true team workspace.

The current local-first design is still useful and should not be thrown away. It should become one supported runtime mode:

```text
Local mode:
  single user, local SQLite, local blobs, optional local AI

Team mode:
  desktop/web clients, shared server API, shared PostgreSQL/object storage/vector index, permissions, jobs, audit log

Hybrid mode:
  local cache plus server sync for offline or privacy-sensitive workflows
```

The main architectural decision is not simply "microservices or not." The first decision is where the source of truth lives for a shared workspace.

For team collaboration, the source of truth should move from each desktop app to a shared backend service. The desktop app can remain valuable as a client, local cache, and local/private mode.

## Current Limitation

Today, RepoMemo can model a workspace locally, but it cannot safely model a shared workplace because it lacks:

- shared user identity
- organization or team tenancy
- workspace membership
- role-based permissions
- shared artifact ownership
- server-side ingestion jobs
- server-side indexing jobs
- shared search indexes
- shared memory cards
- audit trail
- conflict handling
- sync protocol
- remote blob storage
- central provider settings and policy controls

The existing data model is a strong starting point because most records already include `workspace_id`. But for multi-user work we would need to add at least:

- `organizations`
- `users`
- `memberships`
- `workspace_members`
- `roles`
- `permissions`
- `audit_events`
- `sync_events` or durable job/event tables
- `artifact_versions` or source revision metadata
- server-owned provider/policy settings

## Recommended Direction

The recommended path is a modular server architecture first, not a distributed microservices architecture first.

In practical terms:

```text
Keep:
  one monorepo
  Rust crates as internal boundaries
  local desktop mode
  SQLite for local mode
  content-addressed blob model

Add:
  server runtime
  HTTP or RPC API
  PostgreSQL for shared metadata
  object storage for shared blobs
  background worker process
  permission-aware retrieval
  organization/workspace model
```

This gives the project a real team backend without taking on the full operational weight of microservices too early.

## Why Not Microservices Immediately?

Microservices can be useful later, but they would be expensive now because the product has not yet fully stabilized its domain boundaries.

The current crates already express useful boundaries:

- `domain`
- `storage`
- `ingestion`
- `indexer`
- `api`

Future crates can preserve the same direction:

- `retrieval`
- `ai`
- `connectors`
- `jobs`
- `auth`
- `server`

These can run in one server process first. If load or team ownership demands it, individual capabilities can later split into independent services.

## Proposed Target Architecture

### Phase 1: Local-First MVP

This is the current path and should continue until the core memory loop works well.

```text
Desktop UI
  -> Tauri commands
  -> RepoMemoCore
  -> SQLite
  -> local blobs
  -> local indexes
```

Primary goal:

```text
Prove that RepoMemo can ingest, index, search, answer, cite, and save useful memory locally.
```

Advantages:

- fast iteration
- simple install story
- strong privacy story
- low infrastructure cost
- easier debugging
- no distributed system complexity

Drawbacks:

- no real shared workspace
- no central team memory
- no shared permissions
- no shared connector sync
- local data can drift between users
- difficult to support organization-wide policies

### Phase 2: Server Mode Inside The Same Monorepo

Add a server binary that uses the existing domain and engine crates.

```text
Desktop UI or Web UI
  -> Server API
  -> RepoMemo server core
  -> PostgreSQL
  -> object storage
  -> embedded or external vector index
  -> background jobs
```

Recommended stack:

- Rust server with Axum
- PostgreSQL for shared metadata, jobs, permissions, audit events
- S3-compatible object storage for blobs
- Qdrant only when vector search needs a dedicated service
- Redis only if the job queue or caching needs it
- OpenTelemetry-style tracing before splitting services

Advantages:

- enables real team collaboration
- creates a shared source of truth
- preserves existing Rust domain logic
- avoids premature service sprawl
- supports both self-hosted and hosted futures
- provides a stepping stone to microservices later

Drawbacks:

- requires auth, tenancy, and permission design
- requires migrations from SQLite concepts to PostgreSQL
- creates infrastructure and deployment work
- introduces server availability as a product concern
- requires security hardening earlier

### Phase 3: Hybrid Desktop Sync

Once server mode exists, decide whether the desktop app should remain a thin online client or support offline/local cache.

Option A: Thin team client

```text
Desktop app -> server API -> shared backend
```

Option B: Hybrid sync client

```text
Desktop app -> local cache -> sync engine -> shared backend
```

Recommended default:

Start with a thin team client for shared workspaces. Keep fully local workspaces as a separate mode. Add offline sync only after real customer need appears.

Advantages of thin team client:

- simpler consistency model
- easier permissions enforcement
- fewer conflict scenarios
- faster path to shared team workspace

Drawbacks of thin team client:

- weaker offline story
- server required for team work
- less local-first purity for shared projects

Advantages of hybrid sync:

- better offline support
- can keep local-first ergonomics
- useful for private or air-gapped workflows

Drawbacks of hybrid sync:

- much more complex
- needs conflict resolution
- needs durable sync protocol
- requires careful permission handling for cached data
- can create hard-to-debug data divergence

### Phase 4: Split Services Only Where Needed

After server mode is stable, split services based on actual pressure:

```text
api-service:
  auth, workspace APIs, search APIs, memory APIs

worker-service:
  ingestion, indexing, embeddings, connector sync

retrieval-service:
  hybrid search, reranking, context packing

ai-orchestrator-service:
  provider routing, redaction, prompt execution, citations

connector-workers:
  GitHub, GitLab, Jira, Linear, document systems
```

This should happen only when there is clear need:

- independent scaling
- independent deployment cadence
- security isolation
- long-running workloads affecting API latency
- different team ownership
- different runtime requirements

## Microservices: Advantages

Microservices may help later because RepoMemo naturally has different workload types:

- API requests are latency-sensitive.
- indexing is CPU and IO heavy.
- embeddings can be expensive and slow.
- connector sync is bursty and failure-prone.
- AI orchestration may need rate limits, redaction, and provider-specific handling.
- vector search may need a dedicated database.

Potential advantages:

- scale expensive workers independently
- isolate connector failures from the main API
- deploy indexing improvements without redeploying the main API
- separate security boundaries around AI/provider access
- support larger enterprise deployments
- let specialized teams own specialized services

## Microservices: Drawbacks

The drawbacks are serious, especially before the product domain stabilizes:

- more deployment complexity
- more local development complexity
- network failure modes between services
- harder transactions across boundaries
- distributed tracing becomes mandatory
- API versioning between services
- service discovery and configuration overhead
- more secrets and credentials to manage
- more complicated test environments
- harder debugging for early product behavior
- risk of splitting the wrong boundaries too early

For RepoMemo specifically, the highest risk is splitting before the retrieval, indexing, memory, and permission models are mature. If we split too early, we may lock ourselves into awkward APIs around concepts that are still changing.

## Recommended Backend Service Choice

Use a modular monolith server first.

That means:

```text
One deployable server
Multiple internal Rust crates
Clear module boundaries
Database transactions remain simple
Background worker can be a second process when needed
Services can split later
```

Suggested initial deployables:

```text
repomemo-desktop:
  current Tauri local app

repomemo-server:
  shared API, auth, workspace management, search endpoints

repomemo-worker:
  ingestion, indexing, embeddings, connector sync
```

This is not "no microservices ever." It is "one server boundary before many service boundaries."

## Data Ownership Model

For shared team workspaces, the backend should own canonical state:

```text
PostgreSQL:
  organizations
  users
  workspaces
  sources
  artifacts
  chunks
  symbols
  links
  memory_cards
  provider_settings
  jobs
  permissions
  audit_events

Object storage:
  raw blobs by content hash

Vector index:
  chunk embeddings
  optional memory card embeddings
```

The desktop app may own:

```text
local workspace mode:
  full local SQLite and blobs

team workspace mode:
  local cache only, if needed
```

Avoid mixing those semantics silently. The UI should make it clear whether a workspace is local or shared.

## Key Decisions To Make Before Implementation

1. Product mode

Decide whether shared workspaces are required soon or whether local-first MVP remains the priority until Phase 1 is complete.

2. Source of truth

Decide whether team workspaces are server-authoritative from day one. Recommendation: yes.

3. Client shape

Decide whether the first team version is desktop-only, web-only, or both. Recommendation: keep desktop, but expose the same server API a web client could use later.

4. Tenancy

Decide whether the main unit is `organization -> workspace -> source/artifact`, or whether workspaces can exist without organizations. Recommendation: support organizations for shared mode; keep local workspaces independent.

5. Permissions

Decide the first permission model. Recommendation: simple roles first:

```text
owner
admin
member
viewer
```

6. Ingestion location

Decide whether imports run on the client or server for shared workspaces. Recommendation: server-side ingestion for shared connector sources; client upload can stream files to the server.

7. AI policy

Decide whether AI settings are per user, per workspace, or organization-wide. Recommendation: organization policy plus workspace settings, with user-level provider keys only if explicitly needed.

8. Sync complexity

Decide whether offline sync is a product requirement. Recommendation: not for the first shared backend.

## Implementation Plan

### Step 1: Finish The Local Memory Loop

Complete the existing Phase 1 roadmap:

- import and store
- chunking
- full-text search
- symbol indexing
- AI provider layer
- semantic ask with citations
- memory cards

Reason:

The shared backend should preserve a working product loop, not invent one while also solving collaboration.

### Step 2: Write A Team Architecture ADR

Create a formal architecture decision for:

- local mode vs shared mode
- server-authoritative shared workspaces
- modular monolith server before microservices
- PostgreSQL/object storage as shared backend foundation
- background worker as the first separately deployable component

### Step 3: Introduce Server Runtime

Add a new server crate or app:

```text
apps/server
```

or:

```text
crates/server
```

It should expose:

- health endpoint
- auth placeholder
- organization/workspace endpoints
- artifact endpoints
- search endpoints
- job status endpoints

The server should reuse existing crates wherever possible.

### Step 4: Add PostgreSQL Storage Adapter

Keep the storage interface conceptually stable, but add a server storage implementation backed by PostgreSQL.

Recommended approach:

```text
storage:
  shared traits and model mapping

storage-sqlite:
  local desktop implementation

storage-postgres:
  team/server implementation
```

This prevents local mode and server mode from fighting over one database assumption.

### Step 5: Add Shared Blob Storage

Preserve content-addressed storage, but add adapters:

```text
local filesystem adapter
S3-compatible adapter
MinIO/dev adapter
```

The content hash model should remain the same across local and server modes.

### Step 6: Add Auth And Workspace Membership

Add:

- users
- organizations
- memberships
- workspace membership
- basic roles
- API authorization checks

Do this before shared retrieval, because search results must be permission-aware.

### Step 7: Move Jobs Server-Side

Add a server-side job model for:

- import
- indexing
- embeddings
- connector sync
- reindexing

Start with a database-backed worker. Add Redis or another queue only when PostgreSQL-based jobs are insufficient.

### Step 8: Add Permission-Aware Retrieval

Search and ask must filter by user permissions before returning chunks, citations, artifacts, or memory cards.

This is critical because AI answers can leak retrieved context if permission checks happen too late.

### Step 9: Add Team Memory Cards

Memory cards in shared workspaces should include:

- creator
- last editor
- version or updated timestamp
- linked citations
- audit events
- optional approval state

### Step 10: Decide Whether To Split Services

After server mode and worker mode are stable, decide whether to split based on operational evidence.

Likely first split:

```text
api server
worker service
```

Likely later splits:

```text
connector workers
ai orchestration
retrieval/indexing
```

## Practical Recommendation

Proceed in this order:

1. Keep the current local-first desktop MVP going until the memory loop is complete.
2. In parallel, document the shared-workspace architecture decision.
3. Build a server-authoritative team mode as a modular monolith.
4. Add a separate worker process before adding many microservices.
5. Split into microservices only when scale, security, deployment, or ownership pressure makes the split obviously valuable.

This path protects the current local-first advantage while giving the project a credible team collaboration future.

## Summary Position

RepoMemo should not become a microservices system immediately.

RepoMemo should become a local-first product with a server-backed shared workspace mode.

The architecture should evolve like this:

```text
local desktop app
  -> modular local core
  -> shared server mode
  -> server plus worker
  -> selective microservices
```

That keeps the project simple while the product is still being proven, but it does not trap the project in a single-user architecture.
