# Phase 1B: Import And Store

## Goal

Let users import files and folders into a selected workspace, store raw content in content-addressed local blobs, and create source/artifact records in SQLite.

This phase must be useful without AI.

## Product Behavior

- User selects one or more files and/or folders from the desktop app.
- RepoMemo walks selected folders recursively.
- Supported files are imported as artifacts.
- Unsupported, oversized, binary, inaccessible, or ignored files are skipped with a reason.
- Imported artifacts appear in an Artifacts view.
- Workspace overview shows counts for sources, artifacts, indexed chunks, symbols, and memory cards.
- UI follows the design system: compact rows, clear local storage state, no hero layout, no decorative panels.

## Accepted File Types

Initial extensions:

```text
md, mdx, txt, rs, ts, tsx, js, jsx, py, json, toml, yaml, yml, sql, html, css, sh, ps1
```

Skip:

```text
.git, node_modules, target, dist, build, .next, .vite
```

Default file size limit: `5 MB`.

## Backend Changes

- Add `crates/ingestion`.
- Add path discovery, ignored-directory filtering, accepted-extension detection, file size checks, and simple binary detection.
- Add storage methods for:
  - creating/upserting `sources`
  - inserting `blobs`
  - inserting/upserting `artifacts`
  - listing artifact summaries
  - reading artifact details
  - returning workspace overview counts
- Store blobs under:

```text
<app-data>/blobs/<first-2-hash-chars>/<next-2-hash-chars>/<full-sha256>
```

- Reuse existing SHA-256 content hash helper.
- Make imports idempotent by using the existing artifact uniqueness rule: workspace, source, path, content hash.

## Tauri Commands

Add stable snake_case commands:

```text
import_paths(workspace_id, paths) -> ImportReport
list_artifacts(workspace_id) -> Vec<ArtifactSummary>
get_artifact(artifact_id) -> ArtifactDetail
get_workspace_overview(workspace_id) -> WorkspaceOverview
```

Dialog selection can be wired through Tauri dialog support from the UI, but the backend command should accept explicit paths so it remains testable.

## Frontend Changes

- Enable `Artifacts` navigation.
- Add an import action in the workspace workbench.
- Add an Artifacts view with dense list/table rows:
  - title
  - path
  - type
  - language
  - size
  - source
  - indexed state
- Add import report panel with counts:
  - scanned
  - imported
  - skipped
  - duplicate
  - failed
- Show skipped-file reasons in a collapsible activity/log area.
- Rename current workspace list language so workspace rows do not feel like notes.

## Domain DTOs

Add:

```text
Source
ArtifactSummary
ArtifactDetail
ImportRequest
ImportReport
ImportSkippedItem
WorkspaceOverview
```

## Edge Cases

- Empty path list returns a validation error.
- Missing workspace returns a clear error.
- Paths outside the user's selected files/folders are never scanned.
- Permission-denied files are skipped, not fatal to the whole import.
- Duplicate content stores one blob but can still create separate artifact records when path/source differ.

## Tests

- File extension allowlist.
- Ignored directory filtering.
- Oversized file skipping.
- Binary file skipping.
- Content hash stability.
- Blob path layout.
- Artifact type and language detection.
- Import report counts.
- Idempotent re-import.

## Implementation Status

Status: implemented, pending local validation.

Implemented pieces:

- `crates/ingestion`
- content-addressed blob writes
- source/artifact persistence
- Tauri import/artifact/overview commands
- dialog plugin wiring
- compact import/artifact UI aligned with the design system
- initial ingestion/storage unit tests

## Acceptance Criteria

- User can import supported files/folders from the desktop app.
- Raw content is stored content-addressably.
- Artifact/source/blob rows are created.
- Artifacts appear in the UI.
- Import failures are inspectable and do not crash the app.
