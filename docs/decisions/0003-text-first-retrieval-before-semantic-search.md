# 0003: Text-First Retrieval Before Semantic Search

Status: accepted

Date: 2026-06-11

## Context

RepoMemo must work without AI and without external services. Keyword and metadata retrieval are also more explainable than vector-only retrieval, especially for paths, symbols, line ranges, and exact technical terms.

## Decision

Implement SQLite FTS5 retrieval before semantic search. Semantic search should enhance local full-text retrieval later, not replace it.

## Consequences

- Phase 1D delivers search without AI.
- Result snippets, paths, and line ranges become the citation substrate for later ask/summarization.
- Hybrid retrieval in Phase 1G will combine FTS, embeddings, and metadata boosts.
