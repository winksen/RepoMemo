# Phase 1G: Semantic Search And Ask

## Goal

Add hybrid retrieval and cited answers over indexed local context.

## Product Behavior

- User can ask questions about imported docs/code.
- RepoMemo retrieves context first, then generates an answer.
- Retrieved context remains visible and inspectable.
- Every answer includes citations or an insufficient-context message.

## Backend Changes

- Store embeddings locally in SQLite as float32 blobs for the MVP.
- Add chunk embedding jobs using the configured local provider first.
- Implement hybrid retrieval:
  - FTS candidates
  - embedding similarity
  - metadata boosts for path/type/language/source
  - context packing with citations
- Add ask orchestration in `crates/ai` or `crates/retrieval` with a clear boundary: retrieval prepares evidence, AI generates prose.

## Tauri Commands

```text
embed_workspace(workspace_id, provider_id) -> IndexingJobStatus
ask_workspace(request) -> AskAnswer
```

`AskRequest` fields:

```text
workspace_id
question
provider_id
filters
limit
```

`AskAnswer` fields:

```text
answer_markdown
citations
retrieved_context
confidence
warnings
```

## Frontend Changes

- Enable `Ask` navigation.
- Show prompt input, provider mode, retrieved context, answer, and citations.
- Citation chips open artifact preview at the relevant chunk.
- Weak retrieval state should say: `Indexed context is insufficient for a reliable answer.`

## Edge Cases

- No provider configured: Ask page explains setup and keeps search available.
- No embeddings yet: ask can fall back to FTS-only retrieval with warning.
- Provider failure returns recoverable UI state.
- Low-confidence retrieval should avoid confident generated answers.

## Tests

- Embedding serialization/deserialization.
- Hybrid ranking combines FTS and vector results.
- Ask response includes citations.
- Insufficient-context path.
- Provider disabled path.

## Acceptance Criteria

- User can ask about imported content.
- Answers cite exact artifacts/chunks.
- Search remains useful without semantic/AI features.
- No cloud calls happen without explicit opt-in.
