-- First-run setup of a new server. The row appears when the onboarding
-- creates the first system administrator; `completed_at` is set once, when
-- that administrator finishes the onboarding, and never cleared by the
-- application. No row and no accounts means a brand-new server.
CREATE TABLE IF NOT EXISTS server_setup (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  admin_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  admin_created_at TEXT,
  completed_at TEXT,
  completed_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL
);

-- A server that already has accounts was set up before onboarding existed:
-- it must never show the onboarding.
INSERT OR IGNORE INTO server_setup (id, completed_at)
SELECT 1, strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
WHERE EXISTS (SELECT 1 FROM users);
