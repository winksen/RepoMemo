-- Who last changed each repository file, and when (roadmap F16b, V0.2.24).
-- Filled by each sync from one `git log` pass; NULL until a sync has run.
ALTER TABLE repo_files ADD COLUMN last_author TEXT;
ALTER TABLE repo_files ADD COLUMN last_authored_at TEXT;
ALTER TABLE repo_files ADD COLUMN last_change_commit TEXT;
