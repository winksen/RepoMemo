-- Recurring work (V0.7.16): a task that is made again on an explicit schedule. A schedule is a kind (`daily`, `weekly` on a
-- weekday, `monthly` on a day 1 to 28), an interval and a start date; every occurrence is a date, and the pair (recurring task,
-- date) is the occurrence's identity, so a retry or a restart cannot make it twice. Missed occurrences follow the stated policy;
-- nothing is made while a schedule is paused or after it is stopped, and stopping keeps every task it made.
CREATE TABLE IF NOT EXISTS recurring_tasks (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  title TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  priority TEXT NOT NULL DEFAULT 'medium',
  assignee_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  project_id TEXT REFERENCES workspace_projects(id) ON DELETE SET NULL,
  -- each task made is due this many days after its occurrence date; none means it has no due date
  due_after_days INTEGER,
  schedule_kind TEXT NOT NULL CHECK (schedule_kind IN ('daily', 'weekly', 'monthly')),
  interval_count INTEGER NOT NULL DEFAULT 1,
  -- weekly: 0 (Monday) to 6 (Sunday); monthly: the day of the month, 1 to 28
  weekday INTEGER,
  day_of_month INTEGER,
  start_date TEXT NOT NULL,
  -- `latest` makes only the newest missed occurrence; `skip` makes an occurrence only on its own day
  missed_policy TEXT NOT NULL DEFAULT 'latest' CHECK (missed_policy IN ('latest', 'skip')),
  status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'paused', 'stopped')),
  -- every occurrence up to and including this date has been looked at (made or skipped)
  processed_through TEXT NOT NULL,
  created_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  stopped_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_recurring_tasks_workspace ON recurring_tasks (workspace_id, status);

-- One row for each occurrence date that was looked at. The task survives the schedule being stopped, and the row survives the
-- task being deleted.
CREATE TABLE IF NOT EXISTS recurring_occurrences (
  recurring_id TEXT NOT NULL REFERENCES recurring_tasks(id) ON DELETE CASCADE,
  occurrence_date TEXT NOT NULL,
  outcome TEXT NOT NULL CHECK (outcome IN ('created', 'skipped')),
  task_id TEXT REFERENCES workspace_tasks(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY (recurring_id, occurrence_date)
);
