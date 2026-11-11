-- The last reminder sent to a reviewer for one item (V0.6.7), so a due review produces one reminder per window and not one
-- per maintenance pass. `due_at` is the review date the reminder was about: moving the date makes a new reminder
-- possible at once. A row goes with the person and the workspace.
CREATE TABLE IF NOT EXISTS review_reminders (
  item_kind TEXT NOT NULL,
  item_id TEXT NOT NULL,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  due_at TEXT,
  last_sent_at TEXT NOT NULL,
  PRIMARY KEY (item_kind, item_id, user_id)
);
