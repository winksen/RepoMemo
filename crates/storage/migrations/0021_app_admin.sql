-- App administrators use the system pages and API like system administrators,
-- but do not act as an administrator in every organization and workspace.
ALTER TABLE users ADD COLUMN is_app_admin INTEGER NOT NULL DEFAULT 0;
