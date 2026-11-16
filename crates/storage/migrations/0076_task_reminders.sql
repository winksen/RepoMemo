-- Due reminders (V0.7.15): a ledger of the reminders already sent for a task, so a retry, a restart or the next maintenance pass
-- never sends the same one twice. There is one row for each task, person and kind (`due`, `overdue`); it remembers the due
-- value it was sent for, so moving the due date allows a new reminder and nothing else does.
CREATE TABLE IF NOT EXISTS task_reminders (
  task_id TEXT NOT NULL REFERENCES workspace_tasks(id) ON DELETE CASCADE,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK (kind IN ('due', 'overdue')),
  due_key TEXT NOT NULL,
  sent_at TEXT NOT NULL,
  PRIMARY KEY (task_id, user_id, kind)
);
