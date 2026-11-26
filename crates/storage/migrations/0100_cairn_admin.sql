-- A system administrator can make a person a CAIRN administrator (V0.9.71): they administer CAIRN's models, limits and settings
-- (the administration rows of the 0.9 line) without administering the server. System administrators are CAIRN administrators by right.
ALTER TABLE users ADD COLUMN is_cairn_admin INTEGER NOT NULL DEFAULT 0 CHECK (is_cairn_admin IN (0,1));
