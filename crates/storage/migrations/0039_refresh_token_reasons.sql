-- A refresh token remembers why it was revoked and, for a rotation, when (review S5-4). Two tabs of
-- one browser share the refresh cookie: when both present it at the same moment, the second finds
-- it rotated by the first, which used to read as theft and end every session of the account. A token
-- rotated less than 30 seconds ago and presented again is now only refused (and the second tab
-- renews with the cookie the first one received); an older rotated token, or one revoked for any
-- other reason, is still reuse and ends them all. Existing rows have no reason, so they count as reuse.
ALTER TABLE refresh_tokens ADD COLUMN revoked_reason TEXT;
ALTER TABLE refresh_tokens ADD COLUMN replaced_at TEXT;
