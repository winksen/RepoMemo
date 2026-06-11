# 0002: Import And Storage Before AI

Status: accepted

Date: 2026-06-11

## Context

RepoMemo's value depends on trustworthy local memory: files, metadata, chunks, relationships, and citations. AI should reason over that memory, not replace it.

## Decision

Phase 1B prioritizes file/folder import, content-addressed blob storage, and artifact/source records before AI features.

The next implementation pass should make the app useful in no-AI mode by letting users import and inspect artifacts locally.

## Consequences

- The first useful loop is import and browse, then search.
- AI provider work starts only after local artifacts and indexed chunks exist.
- The UI should avoid presenting unavailable AI/search features as primary actions.
