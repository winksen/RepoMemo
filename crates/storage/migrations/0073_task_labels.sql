-- Labels (V0.7.10): names a workspace gives to kinds of work, with a colour from a fixed set whose text always reaches the
-- contrast a reader needs. A label only organizes: nothing reads it to decide who may see or change a task. Renaming keeps every
-- assignment (the link holds the label's id), and merging moves the links of one label to another.
CREATE TABLE IF NOT EXISTS task_labels (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  -- the name in lower case, so two labels cannot differ only by case
  name_key TEXT NOT NULL,
  color TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE (workspace_id, name_key)
);

CREATE TABLE IF NOT EXISTS task_label_links (
  task_id TEXT NOT NULL REFERENCES workspace_tasks(id) ON DELETE CASCADE,
  label_id TEXT NOT NULL REFERENCES task_labels(id) ON DELETE CASCADE,
  created_at TEXT NOT NULL,
  PRIMARY KEY (task_id, label_id)
);

CREATE INDEX IF NOT EXISTS idx_task_label_links_label ON task_label_links (label_id);
