-- Outgoing mail (roadmap F6a, V0.2.9): one row of settings for the SMTP
-- server, with the password sealed like AI provider keys, and an outbox with
-- retries so a slow mail server never holds a request.
CREATE TABLE IF NOT EXISTS mail_settings (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  enabled INTEGER NOT NULL DEFAULT 0,
  host TEXT NOT NULL DEFAULT '',
  port INTEGER NOT NULL DEFAULT 587,
  security TEXT NOT NULL DEFAULT 'starttls' CHECK (security IN ('tls', 'starttls')),
  username TEXT NOT NULL DEFAULT '',
  password_sealed TEXT,
  from_address TEXT NOT NULL DEFAULT '',
  from_name TEXT NOT NULL DEFAULT '',
  updated_at TEXT NOT NULL,
  updated_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL
);

-- A message waits here until it is sent, retried or given up on. The body is
-- emptied once it is sent or given up, since it can carry a one-time link.
CREATE TABLE IF NOT EXISTS mail_outbox (
  id TEXT PRIMARY KEY,
  to_address TEXT NOT NULL,
  subject TEXT NOT NULL,
  body TEXT NOT NULL,
  kind TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('queued', 'sending', 'sent', 'failed')),
  attempts INTEGER NOT NULL DEFAULT 0,
  last_error TEXT,
  created_at TEXT NOT NULL,
  next_attempt_at TEXT,
  sent_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_mail_outbox_due ON mail_outbox (status, next_attempt_at);
CREATE INDEX IF NOT EXISTS idx_mail_outbox_created ON mail_outbox (created_at);
