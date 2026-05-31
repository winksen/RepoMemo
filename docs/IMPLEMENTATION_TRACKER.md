# RepoMemo Implementation Tracker

This tracker records what has been added, what is partially wired, and what remains intentionally deferred.

## Phase 1A: Skeleton

Status: implemented, pending Rust toolchain verification

| Area | Status | Notes |
| --- | --- | --- |
| Monorepo layout | Done | Root npm workspace and Rust workspace are configured. |
| Architecture doc | Done | Original brief copied to `docs/architecture/RepoMemo_ARCHITECTURE.md` and renamed for RepoMemo. |
| Rust domain models | Done | Initial workspace, artifact, chunk, symbol, link, memory card, and provider types added. |
| SQLite schema | Done | Initial migration covers metadata tables, jobs, provider settings, and FTS-ready chunk index. |
| Storage core | Done | Opens local SQLite DB, creates app data folders, runs migrations, and manages workspaces. |
| Tauri command bridge | Done | Desktop app can call Rust commands to list/create workspaces and read storage location. |
| React UI shell | Done | Initial workbench, workspace creation, workspace list, and settings panel. |
| Tauri capabilities | Done | Default Tauri v2 capability file added for the main window. |
| Development command reference | Done | See `docs/DEVELOPMENT_COMMANDS.md`. |
| Local verification | Partial | Frontend typecheck/build passed. Rust verification requires Cargo on PATH. |

## Phase 1B: Upload And Store

Status: not started

Planned:

- File and folder import commands
- Accepted extensions for Markdown, text, and common code files
- Content hashing and content-addressed blob storage
- Artifact/source records
- Import status UI

## Phase 1C: Text Extraction And Chunking

Status: not started

Planned:

- Text extraction for Markdown, text, and code
- Markdown heading path extraction
- Chunk records with line ranges
- Artifact detail view

## Phase 1D: Full-Text Search

Status: not started

Planned:

- SQLite FTS5 search endpoint
- Search UI
- Type/language/source filters
- Result snippets and artifact navigation

## Phase 1E And Later

Status: deferred

Code symbols, AI providers, semantic search, ask-with-citations, and memory cards are represented in the schema and domain model, but implementation starts after the local import/search loop is working.
