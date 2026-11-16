-- Task editing (V0.7.6): every save of a task adds one to its version, and a save that says which version it started from is
-- applied only if that is still the current one. Tasks that already exist start at 1.
ALTER TABLE workspace_tasks ADD COLUMN version INTEGER NOT NULL DEFAULT 1;
