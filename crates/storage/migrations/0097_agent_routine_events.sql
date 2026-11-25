-- Event routines (V0.9.64): a routine may run when something happens in the workspace instead of on a date. Event routines keep a
-- placeholder schedule that is never used, and remember up to when they looked, so each run gets only what is new.
ALTER TABLE agent_routines ADD COLUMN event_kind TEXT CHECK (event_kind IN ('files_indexed', 'file_outdated'));
ALTER TABLE agent_routines ADD COLUMN event_folder_id TEXT REFERENCES folders(id) ON DELETE SET NULL;
ALTER TABLE agent_routines ADD COLUMN event_checked_at TEXT;
