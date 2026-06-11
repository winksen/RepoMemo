# RepoMemo Roadmap

RepoMemo is moving from a working Phase 1A desktop skeleton into the local memory loop: import files, store them durably, index them, search them, and then layer AI on top with citations.

All UI work must follow `docs/design/RepoMemo_UI_DESIGN_SYSTEM.md`. The app should feel like a compact technical workbench: dense, local-first, evidence-oriented, and calm.

## Current State

Phase 1A is implemented and locally running:

- Tauri desktop app opens on Windows.
- React UI can call Rust commands through Tauri.
- Local SQLite database is created under the app data directory.
- Workspaces can be created and listed.
- Blob storage directory is prepared.
- Initial schema already includes sources, blobs, artifacts, chunks, symbols, links, memory cards, jobs, provider settings, and FTS triggers.

What exists today is a workspace shell, not yet a full memory product. The next milestone is the import loop.

## Phase Sequence

| Phase | Goal | Depends On | Done When |
| --- | --- | --- | --- |
| 1B: Import And Store | Bring files/folders into RepoMemo as durable artifacts. | 1A | Supported files are copied into content-addressed blob storage and appear in the Artifacts view. |
| 1C: Text Extraction And Chunking | Turn stored artifacts into retrievable text chunks. | 1B | Imported files have chunks with paths and line ranges. |
| 1D: Full-Text Search | Search local docs/code without AI. | 1C | Query results return snippets and open artifact context. |
| 1E: Code Symbol Index | Make code search aware of structure. | 1D | TypeScript, Python, and Rust files show symbols and outlines. |
| 1F: AI Provider Layer | Add optional local/cloud provider abstraction. | 1D | User can configure a provider and summarize selected indexed context. |
| 1G: Semantic Search And Ask | Add hybrid retrieval and cited answers. | 1F | Questions return answers with citations or clear insufficient-context state. |
| 1H: Memory Cards | Preserve useful knowledge as searchable durable memory. | 1G | Answers/summaries can be saved, linked, searched, and exported. |

## Dependency Order

The implementation should stay storage-first:

```text
import/store -> extract/chunk -> FTS search -> symbols -> AI provider -> semantic ask -> memory cards
```

AI is intentionally later because it must operate on trustworthy local evidence. Search and artifact inspection must remain useful when AI is disabled.

## Phase 2+ Strategy

Phase 2 and later should remain roadmap-level until Phase 1B through 1H prove the local memory loop.

| Future Phase | Direction |
| --- | --- |
| Phase 2: Git-Aware Indexing | Import repositories directly, capture branch/commit metadata, changed files, ownership hints, and commit/PR relationships. |
| Phase 3: Issue And PR Connectors | Add GitHub/GitLab exports first, then Linear/Jira, linking issues and PRs to artifacts and symbols. |
| Phase 4: Team Server | Introduce server mode with PostgreSQL, object storage, background workers, permissions, audit logs, and Qdrant when vector search needs a service. |
| Phase 5: Enterprise/Hosted | Add SSO, permission-aware retrieval, connector sync, policy controls, redaction, admin controls, and hosted/self-hosted deployment. |

## Roadmap Acceptance

Before leaving Phase 1, RepoMemo should support this complete local loop:

```text
Create workspace
Import docs/code
Inspect artifacts
Search locally
Ask with citations
Save durable memory
```

The app should always show where data is stored, whether AI/cloud is enabled, and which source supports each answer.
