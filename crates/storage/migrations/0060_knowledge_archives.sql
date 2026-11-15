-- Knowledge in workspace archives (V0.6.14, archive format 3).
-- A revision that arrived with an import has no account behind its author: the name the author had on the other server is
-- kept as a label. `import_baseline` marks the revision 1 an import wrote itself for a card that carried none, which the
-- import may rewrite while it is still the card's only revision and nobody has edited the card.
ALTER TABLE memory_card_revisions ADD COLUMN author_label TEXT;
ALTER TABLE memory_card_revisions ADD COLUMN import_baseline INTEGER NOT NULL DEFAULT 0;

-- Approval decisions made on another server. They are a record only: a decision here needs an account here, so these
-- never make a card approved and never satisfy or block independent review.
CREATE TABLE IF NOT EXISTS memory_card_imported_approvals (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  card_id TEXT NOT NULL REFERENCES memory_cards(id) ON DELETE CASCADE,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  revision INTEGER NOT NULL,
  decision TEXT NOT NULL,
  person_label TEXT NOT NULL,
  note TEXT NOT NULL DEFAULT '',
  decided_at TEXT NOT NULL,
  imported_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_memory_card_imported_approvals_card ON memory_card_imported_approvals (card_id, id);
