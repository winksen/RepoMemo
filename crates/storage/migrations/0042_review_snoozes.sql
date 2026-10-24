-- A person's snoozed items of the review queue (roadmap F23b, V0.3.10). A snoozed item leaves that
-- person's list until `until` and then returns by itself; nobody else's list changes. The item is a file,
-- a memory card or a health finding, named by kind and id as the queue names it. A row is only a
-- reminder: it is deleted with the person or the workspace, and expired rows are cleared when the
-- person snoozes something again.
CREATE TABLE IF NOT EXISTS review_snoozes (
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  item_kind TEXT NOT NULL,
  item_id TEXT NOT NULL,
  until TEXT NOT NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY (user_id, workspace_id, item_kind, item_id)
);
