# 0004: Embedded Vector Storage Before Qdrant

Status: accepted

Date: 2026-06-11

## Context

The architecture allows Qdrant later, but requiring a vector service during the MVP would make local setup heavier and weaken the simple desktop-first story.

## Decision

Phase 1 semantic search stores embeddings locally in SQLite as float32 blobs. Qdrant remains a future option for larger local/server deployments.

## Consequences

- The MVP stays single-app and local-first.
- Users do not need to run a separate vector database.
- Future migration to Qdrant should be treated as a scale/server decision, not a Phase 1 prerequisite.
