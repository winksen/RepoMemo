# Phase 1F: AI Provider Layer

## Goal

Add optional AI capabilities without coupling RepoMemo's core memory system to any single provider.

## Product Behavior

- User can keep AI disabled.
- User can configure a local Ollama-compatible provider first.
- Cloud/OpenAI-compatible providers are explicit opt-in.
- Summaries cite the chunks/artifacts used.
- UI always shows provider mode: `No AI`, `Local`, or `Cloud`.

## Backend Changes

- Add `crates/ai`.
- Define provider trait:

```text
generate(prompt, context, options)
embed(texts, options)
summarize(target, options)
rerank(query, candidates, options)
```

- Implement Ollama-compatible local provider first.
- Add provider settings read/write through existing `provider_settings` table.
- Add connection test command.
- Add artifact/chunk summary command.

## Tauri Commands

```text
list_provider_settings(workspace_id) -> Vec<ProviderSettings>
save_provider_settings(settings) -> ProviderSettings
test_provider(provider_id) -> ProviderTestResult
summarize_artifact(artifact_id, provider_id) -> SummaryResult
```

## Frontend Changes

- Enable provider settings section.
- Add local provider form:
  - base URL
  - chat model
  - embedding model
  - enabled state
- Add test connection action.
- Add summarize action in artifact detail.
- Use design system status badges: `Cloud off`, `Local`, `Failed`, `Ready`.

## Privacy Rules

- Cloud provider cannot be enabled silently.
- UI must state when content would leave the machine.
- No AI call should run without an explicitly enabled provider.
- Failed provider calls should not block normal search/browse.

## Tests

- Provider settings validation.
- Provider request construction.
- Disabled provider prevents calls.
- Summary result includes citation references.
- Failed provider returns recoverable error.

## Acceptance Criteria

- App works unchanged with AI disabled.
- User can configure and test a local Ollama-compatible provider.
- User can summarize selected artifact/chunks.
- Summary output includes citations.
