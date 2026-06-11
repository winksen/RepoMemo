# RepoMemo Implementation Tracker

This tracker records what has been added, what is partially wired, and what remains intentionally deferred.

## Phase 1A: Skeleton

Status: implemented and running locally

| Area | Status | Notes |
| --- | --- | --- |
| Monorepo layout | Done | Root npm workspace and Rust workspace are configured. |
| Architecture doc | Done | Original brief copied to `docs/architecture/RepoMemo_ARCHITECTURE.md` and renamed for RepoMemo. |
| Rust domain models | Done | Initial workspace, artifact, chunk, symbol, link, memory card, and provider types added. |
| SQLite schema | Done | Initial migration covers metadata tables, jobs, provider settings, and FTS-ready chunk index. |
| Storage core | Done | Opens local SQLite DB, creates app data folders, runs migrations, and manages workspaces. |
| Tauri command bridge | Done | Desktop app can call Rust commands to list/create workspaces and read storage location. |
| React UI shell | Done | Initial workbench, workspace creation, workspace list, and settings panel. |
| UI design system | Done | Grey-first Spotify-like block layout, blue highlight rules, light/dark tokens, and compact UX guidance documented in `docs/design/RepoMemo_UI_DESIGN_SYSTEM.md`. |
| Tauri capabilities | Done | Default Tauri v2 capability file added for the main window. |
| Tauri icon generator | Done | `npm run icons` creates the required Windows `icon.ico` asset. |
| Development command reference | Done | See `docs/DEVELOPMENT_COMMANDS.md`. |
| Local verification | Done | Frontend typecheck/build passed from user terminal; desktop app opens locally and can create workspaces. |
| Next-phase roadmap | Done | See `docs/ROADMAP.md` and `docs/phases/`. |

Latest verification:

- `npm run typecheck`: passed
- `npm run build`: passed
- `cargo test`: passed
- `cargo check`: native toolchain reached after installing Rust/MSVC dependencies
- `npm run dev`: desktop app opens locally

## Phase 1B: Import And Store

Status: implemented, pending local validation

Spec: `docs/phases/PHASE_1B_IMPORT_AND_STORE.md`

| Area | Status | Notes |
| --- | --- | --- |
| Ingestion crate | Done | `crates/ingestion` discovers selected files/folders, skips ignored directories, filters by extension/size, and detects binary files. |
| Accepted extensions | Done | `md`, `mdx`, `txt`, `rs`, `ts`, `tsx`, `js`, `jsx`, `py`, `json`, `toml`, `yaml`, `yml`, `sql`, `html`, `css`, `sh`, `ps1`. |
| Blob storage | Done | Imported file bytes are written to content-addressed blob paths under the app data folder. |
| Source/artifact records | Done | Imports create or reuse sources and upsert artifact records in SQLite. |
| Tauri commands | Done | Added `import_paths`, `list_artifacts`, `get_artifact`, and `get_workspace_overview`. |
| Dialog support | Done | Added Tauri dialog plugin dependency and default permission. |
| Artifacts UI | Done | Enabled import/artifact workbench, import report, artifact rows, workspace counts, and stored-content preview. |
| Visual rework | Done | Existing shell now uses separate rounded grey blocks, blue-only highlights, no selected-row left stripe, and a functional light/dark toggle. |
| Validation | Pending | Run `npm install`, `npm run typecheck`, `npm run build`, `cargo test`, `cargo check`, and `npm run dev`. |

## Phase 1C: Text Extraction And Chunking

Status: implemented, pending local validation

Spec: `docs/phases/PHASE_1C_TEXT_EXTRACTION_AND_CHUNKING.md`

| Area | Status | Notes |
| --- | --- | --- |
| Indexer crate | Done | `crates/indexer` extracts UTF-8/lossy text and generates deterministic chunks. |
| Markdown chunking | Done | Markdown/MDX split by heading path first, then size-limited windows. |
| Code/text chunking | Done | Text, code, and config files split by stable 100-line windows. |
| Chunk metadata | Done | Chunks preserve artifact/workspace IDs, index, line range, heading path, token estimate, content hash, and `not_configured` embedding status. |
| Storage integration | Done | Reindexing replaces old chunks transactionally and updates `artifacts.indexed_at`. |
| Indexing jobs | Done | `indexing_jobs` rows track artifact/workspace indexing status and progress. |
| Tauri commands | Done | Added `index_artifact` and `index_workspace`. |
| Artifact detail UI | Done | Detail panel shows indexed status, chunk count, and generated chunk previews with line ranges. |
| Validation | Done | `cargo test`, `cargo check`, `npm run typecheck`, and `npm run build` pass. |

## Phase 1D: Full-Text Search

Status: not started

Spec: `docs/phases/PHASE_1D_FULL_TEXT_SEARCH.md`

- SQLite FTS5 search endpoint
- Search UI
- Type/language/source filters
- Result snippets and artifact navigation

## Phase 1E: Code Symbol Index

Status: not started

Spec: `docs/phases/PHASE_1E_CODE_SYMBOL_INDEX.md`

- Tree-sitter parser integration for TypeScript, Python, and Rust
- Symbol extraction into the existing symbols table
- File outline view
- Symbol-aware search results

## Phase 1F: AI Provider Layer

Status: not started

Spec: `docs/phases/PHASE_1F_AI_PROVIDER_LAYER.md`

- Provider abstraction
- Ollama-compatible local provider first
- Explicit opt-in cloud provider path later
- Summary command with citations

## Phase 1G: Semantic Search And Ask

Status: not started

Spec: `docs/phases/PHASE_1G_SEMANTIC_SEARCH_AND_ASK.md`

- Local embedding storage in SQLite for MVP
- Hybrid retrieval
- Ask UI with citations
- Insufficient-context behavior

## Phase 1H: Memory Cards

Status: not started

Spec: `docs/phases/PHASE_1H_MEMORY_CARDS.md`

- Manual memory card creation
- Save cited answers/summaries as durable memory
- Link memory cards to artifacts/chunks
- Memory card search/export

## Phase 2 And Later

Status: deferred

Strategic roadmap: `docs/ROADMAP.md`

Git-aware indexing, issue/PR connectors, team server mode, and enterprise/hosted capabilities remain deferred until Phase 1 proves the local memory loop.
