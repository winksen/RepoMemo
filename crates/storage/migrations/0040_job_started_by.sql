-- A job remembers who started it (review S5-9). Any workspace writer could cancel any job of the
-- workspace, an administrator's whole-workspace re-index included. A job is now cancelled by the
-- person who started it or by an administrator. Jobs started by the server itself (a webhook, the
-- sync schedule, a restart) and jobs of earlier versions have no starter, so only an administrator
-- cancels them.
ALTER TABLE indexing_jobs ADD COLUMN started_by TEXT;
