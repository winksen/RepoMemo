# Phase 1D: Full-Text Search

## Goal

Provide useful local search across imported and chunked docs/code without AI.

## Product Behavior

- User can search the selected workspace.
- Results show artifact title, path, type, language, line range, and snippet.
- Clicking a result opens artifact detail at the matched chunk.
- Search page uses the design system's command/search pattern and avoids chatbot-first UI.

## Backend Changes

- Add `crates/retrieval`.
- Implement SQLite FTS5 search over `chunks_fts`.
- Join search results back to chunks and artifacts.
- Add filters by artifact type, language, and source.
- Return a compact ranked result DTO.

## Tauri Commands

```text
search_workspace(request) -> Vec<SearchResult>
```

`SearchRequest` fields:

```text
workspace_id
query
artifact_types
languages
source_ids
limit
```

`SearchResult` fields:

```text
artifact_id
chunk_id
title
path
artifact_type
language
snippet
start_line
end_line
score
source_name
```

## Frontend Changes

- Enable `Search` navigation.
- Add prominent search field with scope/status affordance.
- Add compact filters for type/language/source.
- Add ranked result list and right-side preview.
- Highlight matched snippets when possible.
- Show empty state: `No indexed chunks matched this query.`

## Edge Cases

- Empty query should not run FTS; show guidance.
- No indexed artifacts should direct user to import/index.
- Malformed FTS syntax should be escaped or converted to safe tokens.
- Very broad queries should be limited and remain responsive.

## Tests

- Basic term search.
- Multi-word search.
- Filter by language/type/source.
- Empty query handling.
- Snippet includes matched text.
- Result click target points to artifact/chunk.

## Acceptance Criteria

- User can search local imported docs/code.
- Search works with AI disabled.
- Results include citations-ready path and line range data.
- Result selection opens inspectable source context.
