-- Server identity configuration is deliberately absent from workspace archives.
-- Full server backups retain it; restoring also requires the existing sealing key.
CREATE TABLE identity_provider (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    configuration TEXT NOT NULL,
    client_secret TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
