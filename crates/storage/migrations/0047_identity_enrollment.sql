-- Enrollment cannot authenticate until invitation acceptance succeeds.
ALTER TABLE linked_identities ADD COLUMN confirmed INTEGER NOT NULL DEFAULT 1 CHECK (confirmed IN (0,1));
