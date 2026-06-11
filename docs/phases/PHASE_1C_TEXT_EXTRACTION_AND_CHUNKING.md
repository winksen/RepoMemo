# Phase 1C: Text Extraction And Chunking

## Goal

Turn stored artifacts into retrievable chunks while preserving file path, headings, and line ranges.

## Product Behavior

- Imported artifacts can be opened in a detail view.
- Detail view shows artifact metadata, raw text preview, and generated chunks.
- Chunks preserve enough context to support search snippets and future citations.
- UI remains dense: list/table plus preview/detail panel, not a document editor.

## Backend Changes

- Add `crates/indexer`.
- Add text extraction for supported text/code formats from stored blobs.
- Add chunking pipeline:
  - Markdown/MDX: split by headings first, then enforce size limits.
  - Code/text/config: split by line windows.
- Store chunks in the existing `chunks` table.
- Existing FTS triggers should populate `chunks_fts` automatically.
- Mark artifact `indexed_at` once chunking succeeds.
- Create or update an `indexing_jobs` row for import/index progress.

## Chunk Defaults

- Markdown target: around `1,200` to `1,800` characters per chunk after heading grouping.
- Code/text target: around `80` to `140` lines per chunk.
- Preserve `start_line`, `end_line`, `heading_path`, `chunk_index`, and `content_hash`.
- Store `embedding_status = not_configured` until semantic search is enabled.

## Tauri Commands

Add or extend:

```text
index_artifact(artifact_id) -> IndexingJobStatus
index_workspace(workspace_id) -> IndexingJobStatus
get_artifact(artifact_id) -> ArtifactDetail
```

`get_artifact` should include chunks once available.

## Frontend Changes

- Artifact detail panel shows:
  - title/path/source metadata
  - indexed status
  - chunk count
  - raw text preview
  - chunk list with line ranges and heading path
- Add `Index`/`Reindex` action for an artifact or workspace.
- Use status badges from the design system: `Ready`, `Indexing`, `Partial`, `Failed`.

## Edge Cases

- Empty files produce zero chunks but remain valid artifacts.
- Invalid UTF-8 is skipped or decoded lossily only when safe; record a warning.
- Very large files are already skipped in Phase 1B.
- Reindex deletes/replaces old chunks transactionally for one artifact.

## Tests

- Markdown heading extraction.
- Line range preservation.
- Empty file handling.
- Reindex replaces old chunks.
- FTS rows are created by triggers.

## Acceptance Criteria

- Imported text/code files produce chunks.
- Chunk line ranges are visible in the UI.
- Reindexing an artifact is deterministic.
- Artifact detail view makes indexed context inspectable.
