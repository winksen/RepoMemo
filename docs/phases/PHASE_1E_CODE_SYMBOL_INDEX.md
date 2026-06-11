# Phase 1E: Code Symbol Index

## Goal

Make code artifacts searchable and browsable by structure, not only raw text.

## Product Behavior

- Code artifact detail view shows an outline of symbols.
- Search can return direct symbol matches.
- Users can inspect where functions/classes/modules are defined.

## Backend Changes

- Extend `crates/indexer` with Tree-sitter parsing.
- Add parsers for:
  - TypeScript/TSX
  - Python
  - Rust
- Extract symbols into the existing `symbols` table.
- For unsupported code languages, keep text chunking only.

## Symbol Defaults

Initial symbol kinds:

```text
function, class, method, interface, enum, config, test
```

Store:

```text
name
kind
signature
start_line
end_line
metadata_json
```

## Tauri Commands

```text
list_symbols(artifact_id) -> Vec<Symbol>
search_symbols(workspace_id, query) -> Vec<SymbolSearchResult>
```

## Frontend Changes

- Add file outline panel in artifact detail.
- Add symbol result row style in Search.
- Use mono font for symbols, signatures, and line ranges.
- Keep rows compact and keyboardable.

## Edge Cases

- Parse failures should not fail artifact indexing.
- Unsupported language shows `No symbol index for this language yet.`
- Duplicate symbol names are allowed when line ranges differ.

## Tests

- Extract TypeScript functions/classes/interfaces.
- Extract Python functions/classes.
- Extract Rust functions/enums/impl methods where practical.
- Parse failure does not break chunk indexing.
- Symbol search returns expected line ranges.

## Acceptance Criteria

- TypeScript, Python, and Rust files show symbols.
- Symbol search can find definitions directly.
- Code artifacts still work as normal text artifacts.
