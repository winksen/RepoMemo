# RepoMemo Architecture Brief

Private, local-first AI memory layer for technical teams.

This document is intended as an initial architecture and implementation guide for a dev agent. It is written as a phased plan so the product can start small, prove the core idea, and grow into a serious team/enterprise system without being over-engineered on day one.

## 1. Honest Critical Take

The idea is strong, but the product should not be positioned as "a wiki with AI." That category is crowded and too vague. The stronger product is:

> A private team memory engine that indexes code, docs, issues, decisions, and technical artifacts, then lets teams search, retrieve, summarize, and ask questions with citations.

The AI layer should not be the source of truth. The core product is the indexing, storage, retrieval, and relationship engine. AI is one interface on top of that memory.

Recommended principle:

```text
Database = truth
Storage engine = raw artifact durability
Indexes = memory and retrieval
AI = reasoning, summarization, and natural language interface
```

### Critical Take On Microservices

Using one repo is the right choice. Starting with distributed microservices is not.

The better approach is a modular monorepo:

```text
One repo
Multiple Rust crates
Clear service boundaries
Run in-process for local MVP
Allow services to split into processes later
```

This gives the benefits of microservice architecture without the early cost of networking, deployment, observability, versioning, service discovery, and distributed failure modes.

The MVP should run as one local app with internal engines:

```text
Desktop UI
  -> local API/core
  -> ingestion engine
  -> storage engine
  -> indexing engine
  -> retrieval engine
  -> AI provider layer
```

Later, the same boundaries can become independent services:

```text
api-service
storage-service
indexer-service
retrieval-service
ai-orchestrator-service
connector-workers
```

### Critical Take On SQL vs NoSQL

Do not choose NoSQL just because the data may become large.

The data will be large, but it is not mainly a "NoSQL problem." It is a multi-storage problem:

- raw files and blobs need object/content-addressed storage
- metadata and relationships need transactional storage
- text search needs an inverted index
- semantic search needs vector storage
- code structure may need graph-like relationships

For phase 1, use:

```text
SQLite for metadata and local persistence
SQLite FTS5 for keyword search
Local filesystem content-addressed storage for raw artifacts
Qdrant or sqlite-vec/LanceDB for vectors
```

For a future team/server product, use:

```text
PostgreSQL for metadata, JSONB, permissions, jobs, and relationships
Object storage for raw artifacts
Qdrant for vector search
Tantivy/OpenSearch only if full-text needs outgrow Postgres or SQLite
```

NoSQL can be useful later for specific workloads, but it should not be the primary default. Most of the product will need consistency, traceability, migrations, ownership, auditability, and relationships. SQL is very good at that.

## 2. Product Scope

RepoMemo is a local-first memory workspace for technical teams.

It should ingest:

- code repositories
- uploaded code folders
- Markdown files
- technical documents
- architecture decision records
- runbooks
- incident notes
- issue/PR exports
- API specs
- meeting notes

It should provide:

- upload/import
- durable local storage
- metadata extraction
- full-text indexing
- code symbol indexing
- semantic/vector indexing
- relationship extraction
- search
- retrieval
- summarization
- cited AI answers
- saved memory cards

The first phase should focus only on:

```text
Upload documents, Markdown, and code
Store them
Index them
Search them
Retrieve relevant context
Summarize selected artifacts
Ask questions with citations
```

Avoid early scope creep:

- no multiplayer editing
- no Notion-style blocks
- no full issue tracker
- no real-time collaboration
- no enterprise permissions
- no complex sync
- no cloud-first dependency

## 3. Recommended Stack

### Local/Desktop App

Recommended:

```text
Tauri
React
TypeScript
Rust backend/core
```

Why:

- Tauri gives a native desktop shell with a Rust backend.
- Rust is suitable for indexing, parsing, file IO, and performance-sensitive local work.
- React/TypeScript keeps the UI fast to build.
- The product can later expose the Rust core through a server API.

### Backend/Core

Recommended:

```text
Rust workspace
Axum for HTTP API if/when needed
Tokio for async jobs
Serde for serialization
SQLx or rusqlite for SQLite/Postgres access
Tree-sitter for code parsing
```

### Database And Storage

Phase 1 local:

```text
SQLite
SQLite FTS5
Filesystem content-addressed blob store
Optional sqlite-vec or local Qdrant for embeddings
```

Phase 2/3 server:

```text
PostgreSQL
Object storage: S3-compatible, MinIO, or local disk adapter
Qdrant for vectors
Tantivy or OpenSearch only if needed
```

### AI Layer

Provider-based:

```text
Local:
  Ollama
  LM Studio-compatible endpoints
  local embedding models

Cloud optional:
  OpenAI
  Anthropic
  Mistral
  other OpenAI-compatible APIs
```

The app must work without AI:

```text
No AI mode:
  browse
  search
  inspect artifacts
  view extracted symbols
  view related artifacts
  manually create memory cards
```

## 4. High-Level Architecture

```text
                      +----------------------+
                      |   Desktop UI         |
                      |   Tauri + React      |
                      +----------+-----------+
                                 |
                                 v
                      +----------------------+
                      |   Local Core API     |
                      |   Rust commands/API  |
                      +----------+-----------+
                                 |
       +-------------------------+-------------------------+
       |                         |                         |
       v                         v                         v
+-------------+          +---------------+          +---------------+
| Ingestion   |          | Storage       |          | Indexing      |
| Engine      |          | Engine        |          | Engine        |
+------+------+          +-------+-------+          +-------+-------+
       |                         |                          |
       v                         v                          v
+-------------+          +---------------+          +---------------+
| Normalizer  |          | Blob Store    |          | FTS Index     |
| Extractor   |          | Metadata DB   |          | Vector Index  |
+------+------+          +-------+-------+          | Symbol Index  |
       |                         |                  +-------+-------+
       +-------------------------+--------------------------+
                                 |
                                 v
                      +----------------------+
                      | Retrieval Engine     |
                      | Hybrid search        |
                      +----------+-----------+
                                 |
                                 v
                      +----------------------+
                      | AI Orchestrator      |
                      | Optional providers   |
                      +----------+-----------+
                                 |
                                 v
                      +----------------------+
                      | Cited Answer /       |
                      | Summary / Memory     |
                      +----------------------+
```

## 5. Monorepo Layout

Initial monorepo:

```text
RepoMemo/
  apps/
    desktop/
      src/
      src-tauri/

  crates/
    core/
    api/
    ingestion/
    storage/
    indexer/
    retrieval/
    ai/
    connectors/
    domain/
    jobs/

  packages/
    ui/
    shared-types/

  docs/
    architecture/
    decisions/

  examples/
    sample-workspaces/
    sample-docs/

  tests/
    fixtures/
```

Suggested crate responsibilities:

```text
domain:
  shared data models, IDs, artifact types, relation types

storage:
  SQLite access, blob store, content hashing, migrations

ingestion:
  import files, folders, archives, repos, manual uploads

indexer:
  chunking, FTS indexing, symbol extraction, embeddings

retrieval:
  keyword search, vector search, graph expansion, reranking

ai:
  provider abstraction, prompt construction, citations, summaries

connectors:
  GitHub/GitLab/Jira/Linear later

jobs:
  local background queue and task status

api:
  command/API surface for desktop app and future server
```

## 6. Core Domain Model

The first model should be simple but future-proof.

### Workspace

```text
workspace
- id
- name
- created_at
- updated_at
- settings
```

### Source

A source is where artifacts came from.

```text
source
- id
- workspace_id
- type: upload | folder | git_repo | manual | connector
- name
- root_uri
- last_indexed_at
- status
- metadata_json
```

### Artifact

An artifact is a meaningful unit: file, document, code file, issue, decision, incident note, etc.

```text
artifact
- id
- workspace_id
- source_id
- type: file | markdown_doc | code_file | issue | pr | decision | incident | runbook | api_spec | note
- title
- path
- content_hash
- mime_type
- language
- size_bytes
- created_at
- updated_at
- indexed_at
- metadata_json
```

### Blob

Raw stored content, addressed by hash.

```text
blob
- hash
- storage_uri
- size_bytes
- mime_type
- created_at
```

### Chunk

A chunk is a retrievable passage.

```text
chunk
- id
- artifact_id
- workspace_id
- chunk_index
- text
- token_count
- start_line
- end_line
- heading_path
- content_hash
- embedding_status
- metadata_json
```

### Symbol

Extracted from code.

```text
symbol
- id
- artifact_id
- workspace_id
- kind: function | class | method | interface | enum | route | endpoint | config | test
- name
- signature
- start_line
- end_line
- metadata_json
```

### Link

Explicit or inferred relationship.

```text
link
- id
- workspace_id
- from_id
- from_type
- to_id
- to_type
- relation_type: references | imports | implements | documents | fixes | mentions | supersedes | owns | tests | configures
- confidence
- created_by: system | ai | user
- metadata_json
```

### Memory Card

A saved piece of team knowledge.

```text
memory_card
- id
- workspace_id
- title
- body_markdown
- source: user | ai_summary | ai_answer
- confidence
- created_at
- updated_at
- metadata_json
```

## 7. Storage Engine Design

The storage engine should separate raw content from metadata.

### Raw Artifact Storage

Use content-addressed storage:

```text
workspace-data/
  blobs/
    ab/
      cd/
        abcdef123...
```

Each uploaded file is hashed. If the same content appears twice, store it once and reference it many times.

Benefits:

- deduplication
- easy change detection
- reproducible indexing
- safer artifact lifecycle
- future migration to object storage

### Metadata Storage

Use SQLite in phase 1:

```text
workspace-data/
  RepoMemo.sqlite
```

SQLite should store:

- workspaces
- sources
- artifacts
- chunks
- symbols
- links
- memory cards
- indexing jobs
- provider settings

### Why Not Store Everything In A NoSQL Document DB?

Because the app will need:

- joins between artifacts, chunks, sources, and links
- migrations
- transactional updates
- auditability
- deterministic re-indexing
- full-text search integration
- local desktop simplicity

Document-shaped metadata can still live in `metadata_json` columns.

## 8. Indexing Architecture

Indexing should be pipeline-based.

```text
Import
  -> detect type
  -> store blob
  -> create artifact
  -> extract text
  -> extract metadata
  -> chunk
  -> full-text index
  -> parse symbols if code
  -> generate embeddings if enabled
  -> infer links
  -> mark indexed
```

### Index Types

Keyword/full-text:

```text
SQLite FTS5 first
```

Semantic/vector:

```text
sqlite-vec or LanceDB for embedded local phase
Qdrant for heavier local/server phase
```

Code structure:

```text
Tree-sitter symbol extraction
dependency/import graph
route/API detection
test-to-source hints
```

Relationship index:

```text
links table
relation confidence
source of relation: parser, text link, user, AI
```

## 9. Retrieval Architecture

Retrieval should be hybrid, not vector-only.

Pipeline:

```text
User query
  -> classify intent
  -> keyword search
  -> vector search if available
  -> metadata filters
  -> graph expansion
  -> rerank
  -> context packing
  -> return sources
  -> optional AI answer
```

Example retrieval logic:

```text
If query mentions a file/path/symbol:
  prioritize exact metadata and symbol matches

If query asks "why" or "decision":
  prioritize ADRs, issues, PRs, memory cards, incidents

If query asks "where implemented":
  prioritize symbols, code files, imports, tests, routes

If query asks "what changed":
  prioritize source updated_at, git metadata later, issue/PR links
```

## 10. AI Layer

The AI layer should be optional and provider-based.

### Provider Interface

```text
AIProvider
- generate(prompt, context, options)
- embed(texts, options)
- summarize(artifact_or_chunks, options)
- rerank(query, candidates, options)
```

### Modes

```text
No AI:
  search and browse only

Local AI:
  local embeddings
  local chat/completion model through Ollama or LM Studio

Hybrid:
  local storage and indexing
  cloud LLM for answers
  optional redaction layer

Cloud:
  cloud embeddings and cloud LLM, explicit opt-in
```

### AI Answer Requirements

Every AI answer must include citations:

```text
- artifact title
- path
- line range when available
- source type
- confidence/warnings
```

The model should be instructed to say when the indexed context is insufficient.

### Summarization

There are two useful types of summaries:

Artifact summary:

```text
Summarize this file/doc/issue.
```

Workspace memory summary:

```text
Summarize what the project seems to do.
Summarize current architecture.
Summarize known risks.
Summarize onboarding path.
```

Summaries should be saved as memory cards only when the user accepts or edits them.

## 11. First Phase Implementation Plan

### Phase 1A: Skeleton

Goal: create the project foundation.

Build:

- Rust workspace
- Tauri desktop app
- React UI shell
- SQLite database setup
- migrations
- local workspace folder
- basic settings page

Done when:

- user can create/open a local workspace
- app stores workspace metadata locally
- UI can call Rust commands

### Phase 1B: Upload And Store

Goal: import documents and code files safely.

Build:

- file upload/folder import
- accepted types: `.md`, `.txt`, common code files
- content hashing
- blob storage
- artifact records
- source records
- import status UI

Done when:

- user can import Markdown and code
- raw content is stored content-addressably
- artifacts appear in the UI

### Phase 1C: Text Extraction And Chunking

Goal: turn artifacts into retrievable chunks.

Build:

- text extraction for Markdown/text/code
- Markdown heading extraction
- chunking by heading and size
- chunk table
- basic artifact detail view

Done when:

- imported files have chunks
- chunks preserve artifact path and line ranges where possible

### Phase 1D: Full-Text Search

Goal: useful retrieval without AI.

Build:

- SQLite FTS5 index
- search endpoint
- search UI
- filters by type/language/source
- result snippets

Done when:

- user can search across uploaded docs and code locally
- result clicks open artifact context

### Phase 1E: Code Symbol Index

Goal: make code search smarter than plain text.

Build:

- Tree-sitter parser integration for 1-3 languages first
- symbol extraction
- symbol table
- symbol search
- file outline view

Recommended first languages:

```text
TypeScript
Python
Rust
```

Done when:

- code files show functions/classes/modules
- search can find symbols directly

### Phase 1F: AI Provider Layer

Goal: add optional AI without coupling the core to a vendor.

Build:

- AI provider interface
- Ollama-compatible local provider
- OpenAI-compatible cloud provider optional
- embedding job status
- provider settings

Done when:

- user can configure local or cloud provider
- app can generate a summary of selected chunks/artifact

### Phase 1G: Semantic Search And Ask

Goal: retrieve and answer with citations.

Build:

- embeddings for chunks
- vector store
- hybrid retrieval
- ask UI
- citation rendering
- "insufficient context" behavior

Done when:

- user can ask a question about uploaded docs/code
- answer cites exact files/chunks
- answer works with local or configured cloud model

### Phase 1H: Memory Cards

Goal: turn useful answers into durable team memory.

Build:

- save AI answer as memory card
- manual memory card creation
- link memory card to artifacts/chunks
- memory card search

Done when:

- user can save important summaries/answers
- memory cards are searchable and exportable

## 12. Future Phases

### Phase 2: Git-Aware Indexing

Add:

- import Git repo directly
- branch/commit metadata
- changed files
- blame/ownership hints
- PR/commit links

### Phase 3: Issue And PR Connectors

Add:

- GitHub export/import
- GitLab export/import
- Linear/Jira later
- issue-to-code relationship extraction

### Phase 4: Team Server

Add:

- server mode
- PostgreSQL
- object storage
- Qdrant
- users
- permissions
- audit log
- background workers

### Phase 5: Enterprise/Hosted

Add:

- SSO
- permission-aware retrieval
- connector sync
- admin dashboard
- policy controls
- redaction policies
- cloud/self-hosted deployment

## 13. Architecture Decisions To Make Later

Do not decide these too early:

- final product name
- cloud hosting architecture
- multiplayer sync
- enterprise permission model
- exact vector database for all phases
- whether to expose a public API
- whether to support mobile

Decide early:

- local-first guarantee
- citation requirement
- source-of-truth data model
- import/index/retrieve flow
- AI provider abstraction
- raw storage vs metadata separation

## 14. Current Recommended Technical Choices

Recommended now:

```text
Language:
  Rust for backend/core
  TypeScript for UI

Desktop:
  Tauri + React

Local DB:
  SQLite

Full-text:
  SQLite FTS5

Raw storage:
  local content-addressed filesystem store

Code parsing:
  Tree-sitter

Vector:
  Start with sqlite-vec or Qdrant local
  Prefer Qdrant when vector search becomes a serious subsystem

AI:
  Ollama-compatible local provider first
  OpenAI-compatible cloud provider behind explicit opt-in
```

Server later:

```text
PostgreSQL
Object storage
Qdrant
Rust services
Background workers
```

## 15. Why This Architecture Is Better Than "AI Chat Over Files"

An AI-chat-over-files app is easy to build but weak:

- it depends too much on embeddings
- it loses structure
- it struggles with code
- it cannot explain provenance well
- it becomes untrustworthy without citations
- it has no durable team memory

RepoMemo should instead be:

```text
Structured local memory first
AI interface second
```

This is the wedge that can make the product serious.

## 16. References Checked

- SQLite FTS5 official documentation: https://www.sqlite.org/fts5.html
- SQLite implementation limits: https://www.sqlite.org/limits.html
- Qdrant documentation: https://qdrant.tech/documentation/
- Qdrant overview: https://qdrant.tech/documentation/overview/
- PostgreSQL JSON types: https://www.postgresql.org/docs/current/datatype-json.html

