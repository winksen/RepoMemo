-- Account lifecycle (roadmap F15a, V0.2.16): a deactivated account cannot sign
-- in or use tokens, but everything it wrote stays. NULL means active, so every
-- account that exists keeps working.
ALTER TABLE users ADD COLUMN deactivated_at TEXT;
