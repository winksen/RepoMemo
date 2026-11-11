-- Who is accountable for reviewing a file or a memory card, and when it is next due (V0.6.6). The item is named by kind
-- (`evidence` or `memory_card`) and id, as the review queue names it. The reviewer may differ from the item's owner. When
-- the reviewer's account is removed the column is cleared and `reviewer_label` keeps the name they had, so the queue can say
-- the reviewer left instead of showing nobody. A row whose item is gone is ignored by every query that joins the item.
CREATE TABLE IF NOT EXISTS review_assignments (
  item_kind TEXT NOT NULL CHECK (item_kind IN ('evidence', 'memory_card')),
  item_id TEXT NOT NULL,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  reviewer_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  reviewer_label TEXT,
  next_review_at TEXT,
  last_reviewed_at TEXT,
  assigned_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (item_kind, item_id)
);

CREATE INDEX IF NOT EXISTS idx_review_assignments_workspace_due ON review_assignments (workspace_id, next_review_at);
CREATE INDEX IF NOT EXISTS idx_review_assignments_reviewer ON review_assignments (reviewer_user_id);
