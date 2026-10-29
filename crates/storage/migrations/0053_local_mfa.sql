-- Optional time-based one-time password (TOTP) for local password accounts. The secret is sealed. `last_step` is the
-- newest 30-second step a code was accepted for, so a captured code cannot be used twice. Recovery codes are stored
-- only as hashes and each works once. A row with enabled = 0 is an enrollment that was begun and not yet confirmed.
CREATE TABLE user_mfa (
    user_id TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    secret TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 0 CHECK (enabled IN (0,1)),
    last_step INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    confirmed_at TEXT
);

CREATE TABLE user_recovery_codes (
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    code_hash TEXT NOT NULL,
    used_at TEXT,
    PRIMARY KEY (user_id, code_hash)
);
