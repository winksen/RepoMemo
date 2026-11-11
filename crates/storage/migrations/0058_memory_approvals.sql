-- Reviewer decisions about one revision of a memory card (V0.6.8). A decision names the revision it was about, so editing the
-- card (which makes a new revision, V0.6.3) leaves the earlier approval attached to the earlier revision and the new
-- revision unapproved. Rows are only added: withdrawing is a later row, and the newest decision for the newest revision
-- says whether the card is approved. A decision goes with its card and keeps the name of who made it while that
-- account exists.
CREATE TABLE IF NOT EXISTS memory_card_approvals (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  card_id TEXT NOT NULL REFERENCES memory_cards(id) ON DELETE CASCADE,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  revision INTEGER NOT NULL,
  decision TEXT NOT NULL CHECK (decision IN ('approved', 'withdrawn')),
  user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  note TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_memory_card_approvals_card ON memory_card_approvals (card_id, revision, id);
