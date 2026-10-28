# Security administration

For the person who owns a RepoMemo server's secrets, sign-in rules and audit trail. It says what each secret protects, how long each credential lives, what to do to rotate one **safely, in order**, and how to check afterwards that access and the audit trail still behave. The reasons are in [Authentication and authorization](../mindmap/technical/auth-and-authorization.md) and the [production guide](../mindmap/technical/operations.md); the review of the deployment surface is in [the security review](../mindmap/security/2026-10-27-deployment-security-review.md).

## 1. Secrets

| Secret | Protects | Give it as | If it is lost or changes |
|---|---|---|---|
| `REPOMEMO_JWT_SECRET` (32+ characters) | Session tokens and the short-lived file links | variable, or `REPOMEMO_JWT_SECRET_FILE` | Changing it signs **everyone** out; nothing is lost |
| `REPOMEMO_SECRET_KEY` (32+ characters) | AI provider keys and repository tokens stored on the server | variable or `_FILE`; without it a `secret.key` file sits in the data folder | Provider keys and repository tokens cannot be read; **enter them again** |
| `REPOMEMO_BACKUP_KEY` (16+ characters) | Seals new backups | variable or `_FILE` | Sealed backups cannot be restored without the key that sealed them |
| `REPOMEMO_BACKUP_KEY_PREVIOUS` | Opens backups sealed with the key before the last change | variable or `_FILE` | See rotation below |
| `REPOMEMO_SETUP_CODE` (12+ letters or digits) | The first-run setup only | variable or `_FILE` | Irrelevant after the setup; the server prints a code when unset |

Rules: keep them in a secret store (Docker or Kubernetes secrets mounted as `_FILE`), not in a file the web server can read. A variable and its `_FILE` together are refused at start, and so is an unreadable or empty file. Keep a copy of `REPOMEMO_SECRET_KEY` and `REPOMEMO_BACKUP_KEY` **apart from the backups** they protect. The container `deploy/init.sh` generates strong values into `.env`, readable by you alone.

## 2. Credential lifetimes

| Credential | Lives | Ends when |
|---|---|---|
| Access token | 60 minutes (`REPOMEMO_ACCESS_TOKEN_TTL_MINUTES`) | it expires, or the account's *session version* changes |
| Refresh token / web cookie | 30 days (`REPOMEMO_REFRESH_TOKEN_TTL_DAYS`), replaced at every use | sign-out, password change, or reuse of an old one (which revokes the whole family) |
| Personal access token (`rmp_…`) | 7 to 365 days, shown once, read-only or read and write | revoked, expired, or a used password reset |
| Invitation link | 7 days, one use, one email address | used, withdrawn, expired |
| Password reset link | 24 hours, one use | used, expired |
| File link (Open in Office) | 60 seconds, first client | expires or another client uses it |

Changing a password, signing out everywhere, deactivating or deleting an account ends all of that account's sessions at once. Failed sign-ins lock an address after 10 tries, and everyone after 50, for 15 minutes; *System › Users* or `users unlock <email> --yes` frees an account.

## 3. Rotate a secret: the safe sequence

Always **back up first** and copy the backup off the machine. Rotate in a quiet hour. After each, run the checks in § 7.

### Session secret (`REPOMEMO_JWT_SECRET`)

1. Tell people they will be signed out.
2. Generate a new value (32+ random characters) and put it in the secret store.
3. Restart the server.
4. **Expect:** everyone must sign in again; stored data is untouched. Old personal access tokens still work (they are not signed with it).

### Stored-credentials key (`REPOMEMO_SECRET_KEY`)

There is no "previous key" for this one, so a change makes the stored AI provider keys and repository tokens unreadable.

1. Write down which workspaces have providers or repository tokens (*Settings › AI integration*, *Repositories*).
2. Change the key and restart.
3. Enter each provider key and repository token again; run *Test connection*.
4. Keep the old key until nothing needs it, for restoring older backups.

### Backup key (`REPOMEMO_BACKUP_KEY`)

1. Move the current value to `REPOMEMO_BACKUP_KEY_PREVIOUS` and set the new value in `REPOMEMO_BACKUP_KEY` (16+ characters, different).
2. Restart. Older backups show *previous key* in *System › Backups*.
3. Seal each one again: `repomemo-console backup reseal <backup> --yes`.
4. Make a new backup, restore a copy of it on a spare machine, and only then remove `REPOMEMO_BACKUP_KEY_PREVIOUS`.

### A personal access token

Create the new token (*Profile › API tokens*, with the narrowest scope that works), switch the script to it, confirm the script works, then revoke the old one. A leaked token is revoked at once.

### A provider API key

Enter the new key in the workspace's AI integration (the key is encrypted and never sent back to the browser), run *Test connection*, then revoke the old key at the provider. Changing a provider's type or address requires entering the key again.

## 4. Cloud AI consent

Nothing leaves the server for an AI provider until a workspace owner or admin configures one. A **cloud** provider (OpenRouter) additionally needs an **explicit acknowledgement** that workspace excerpts leave the server; a local Ollama keeps them on a machine you run. Providers can only be set at hosts in `REPOMEMO_AI_ALLOWED_HOSTS`; addresses of cloud metadata services and link-local addresses are refused. To withdraw consent: remove the provider in *Settings*, or restrict who may use AI there. Image descriptions and search vectors are built in the background from stored excerpts, so removing the provider is what stops them. Interactive use is limited per person by `REPOMEMO_AI_REQUESTS_PER_HOUR`.

## 5. TLS and trusted proxies

- **TLS** is the front door's job: the container edge (`repomemo-edge`, Let's Encrypt), your own proxy ([Front doors](FRONT_DOORS.md)), or none on a trusted network. Set `REPOMEMO_PUBLIC_URL` to the `https://` address so HSTS is on and the refresh cookie is `Secure`. The server refuses to listen beyond loopback over plain HTTP unless told it is behind TLS.
- **Trusted proxies.** Set `REPOMEMO_TRUST_PROXY=true` only behind a proxy that sets `X-Forwarded-For`. Only loopback and the addresses in `REPOMEMO_TRUSTED_PROXIES` are believed, so a client cannot choose its own address. Without it every user shares the proxy's address and one rate limit and lockout.
- **Allowed origin.** `REPOMEMO_ALLOWED_ORIGIN` names the browser origins allowed to call the API.
- **Mail** is only sent over TLS; a private certificate authority is given with `REPOMEMO_SMTP_CA_FILE`. For the desktop app and a private CA, see the [desktop administrator guide](DESKTOP_ADMIN.md).

The setup's **server check** reports on these; run it again from *System* after a change.

## 6. Audit events

*System › Audit trail* records what system administrators did: role changes, settings, ended sessions, maintenance runs, and denied workspace reads. It is trimmed after `REPOMEMO_SYSTEM_AUDIT_RETENTION_DAYS`. To keep it longer, **export** it (JSON lines or CSV, by range of days) or schedule `GET /v1/system/audit/export` with a `system`-scope token. Security-relevant server events (lockouts, bad tokens, refused requests) are in the `security` log category: `repomemo-console logs --category security --level warn`. Inside a workspace, owners and admins see the **Activity** feed (see [Collaboration](COLLABORATION.md#8-auditing-the-lifecycle)).

## 7. After a rotation: verify access and audit

1. **Access:** sign in as an administrator and as an ordinary member; open a workspace; run a search. A viewer must still be unable to add evidence.
2. **Tokens:** a script that uses a token you kept still works; one you revoked answers `401`.
3. **Providers:** *Test connection* passes for each.
4. **Backups:** a new backup is made and listed; its copy is checked.
5. **Audit:** the trail shows the rotation's settings changes and ended sessions, and the `security` log shows no unexpected warnings.
6. **Health:** `/health` is 200 and *System › Overview* is clean.

If something fails, stop, restore the previous value, and read [Troubleshooting](TROUBLESHOOTING.md).
