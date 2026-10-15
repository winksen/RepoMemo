-- Anonymizing an account (roadmap F15b, V0.2.17): the row stays, so what the
-- account wrote keeps its creator, but its address, name and password are
-- gone. NULL means the account still has its personal data.
ALTER TABLE users ADD COLUMN anonymized_at TEXT;
