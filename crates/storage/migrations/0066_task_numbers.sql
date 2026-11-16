-- Task identifiers (V0.7.3): every task gets a number that is unique in its workspace and never reused, so `T-12` names the
-- same task after its title changes and cannot later name another one. Existing tasks are numbered in the order they were
-- made. A deleted task's number is remembered, so a link to it can say "deleted" and not "never existed".
ALTER TABLE workspace_tasks ADD COLUMN number INTEGER;

UPDATE workspace_tasks SET number = (
  SELECT COUNT(*) FROM workspace_tasks AS earlier
  WHERE earlier.workspace_id = workspace_tasks.workspace_id
    AND (earlier.created_at < workspace_tasks.created_at
         OR (earlier.created_at = workspace_tasks.created_at AND earlier.id <= workspace_tasks.id))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_workspace_tasks_number ON workspace_tasks (workspace_id, number);

-- The last number given in each workspace; a number is only ever added to it.
CREATE TABLE IF NOT EXISTS workspace_task_counters (
  workspace_id TEXT PRIMARY KEY REFERENCES workspaces(id) ON DELETE CASCADE,
  last_number INTEGER NOT NULL
);

INSERT INTO workspace_task_counters (workspace_id, last_number)
  SELECT workspace_id, MAX(number) FROM workspace_tasks GROUP BY workspace_id;

CREATE TABLE IF NOT EXISTS workspace_task_tombstones (
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  number INTEGER NOT NULL,
  deleted_at TEXT NOT NULL,
  PRIMARY KEY (workspace_id, number)
);
