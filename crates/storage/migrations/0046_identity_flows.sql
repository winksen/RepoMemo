CREATE TABLE linked_identities (
    issuer TEXT NOT NULL,
    subject TEXT NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL,
    PRIMARY KEY (issuer, subject),
    UNIQUE (issuer, user_id)
);
-- Ephemeral handoffs are sealed and expire in five minutes. Never archived.
CREATE TABLE identity_flows (
    state_hash TEXT PRIMARY KEY,
    proof_hash TEXT NOT NULL,
    payload TEXT NOT NULL,
    phase TEXT NOT NULL CHECK (phase IN ('pending','exchanging','done','failed')),
    expires_at INTEGER NOT NULL
);
CREATE INDEX identity_flows_expiry ON identity_flows(expires_at);
