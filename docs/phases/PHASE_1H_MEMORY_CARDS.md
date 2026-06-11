# Phase 1H: Memory Cards

## Goal

Turn useful summaries and answers into durable, searchable team memory.

## Product Behavior

- User can manually create a memory card.
- User can save an AI answer or summary as a memory card.
- Cards link back to artifacts/chunks through evidence links.
- Cards are searchable and exportable as Markdown.

## Backend Changes

- Use existing `memory_cards` table.
- Use existing `links` table to connect memory cards to artifacts/chunks.
- Add storage methods for create/update/list/get/search memory cards.
- Include memory cards in FTS/search results where useful.

## Tauri Commands

```text
create_memory_card(request) -> MemoryCard
update_memory_card(request) -> MemoryCard
list_memory_cards(workspace_id) -> Vec<MemoryCardSummary>
get_memory_card(card_id) -> MemoryCardDetail
save_answer_as_memory_card(answer_id or payload) -> MemoryCard
export_memory_card(card_id) -> String
```

## Frontend Changes

- Enable `Memory cards` navigation.
- Add dense list and detail view.
- Card row shows:
  - title
  - body excerpt
  - source type
  - linked evidence count
  - updated timestamp
- Add explicit `Save memory` action from summaries/answers.
- Add export Markdown action.

## Edge Cases

- AI answer is not saved automatically.
- Cards can exist without AI.
- Missing linked artifact should show stale/missing evidence state.
- Export should include title, body, source, and evidence links.

## Tests

- Manual card creation.
- Save answer as card.
- Link card to artifact/chunk.
- Search finds cards.
- Export Markdown includes evidence.

## Acceptance Criteria

- User can save durable knowledge.
- Memory cards remain tied to evidence.
- Cards are searchable and exportable.
