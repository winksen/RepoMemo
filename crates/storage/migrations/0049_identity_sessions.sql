-- A session that began with the provider remembers which provider session it belongs to and the moment after which
-- it can no longer be refreshed, whatever the refresh token's own expiry: the provider's disabled people cannot
-- stay signed in forever. Rotation carries both values on to the new token.
ALTER TABLE refresh_tokens ADD COLUMN idp_issuer TEXT;
ALTER TABLE refresh_tokens ADD COLUMN idp_sid TEXT;
ALTER TABLE refresh_tokens ADD COLUMN reauth_by TEXT;

-- Back-channel logout tokens seen so far, so a captured one cannot be replayed.
CREATE TABLE identity_logout_tokens (
    jti_hash TEXT PRIMARY KEY,
    expires_at INTEGER NOT NULL
);
