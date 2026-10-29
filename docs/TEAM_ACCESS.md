# Team access and identity

## Current access model

At V0.5.1, accounts sign in with passwords; administrators can prepare OIDC settings. Provider sign-in, local MFA,
restricted workspace policy, guest expiry and workspace-scoped tokens are
planned in the [0.5 release index](../mindmap/RELEASES_0.5.md).
This guide changes with each implementation release; a planned row is not a
shipped capability or a live validation pass.

| Existing capability | Behavior and source |
|---|---|
| Organization membership | Grants inherited access to the organization's workspaces; explicit workspace grants retain their origin |
| Workspace guests | Direct membership grants only that workspace, without organization membership; no expiry yet |
| Owners and offboarding | Ownership transfer and account deactivation already exist; the 0.5 line will bring them into one handover flow |
| System administrator | Administers the server and has workspace access; app administrators have system tooling without universal content access |
| Personal access tokens | Existing read/write/admin/system scopes and expiration; current tokens are not bound to selected workspaces |
| Audit export | Existing System audit CSV/JSON export; identity events will extend that store |
| Citation editing | Memory-card writers can change linked evidence; this is already implemented |

See [administrator tasks](ADMIN_GUIDE.md), [API access](API.md), and
[security operations](SECURITY_ADMIN.md) for the existing workflows.
The implementation is in storage's organization, access and account modules,
server access guards and the existing audit exporter. New identity proofs must
use these same account and authorization boundaries.

## Implementation and release evidence

The maintainer authorized implementation through V0.5.19 while live evidence
stays pending. The [final 0.4 candidate record](../mindmap/technical/release-evidence-0.4.105.md)
defines that boundary. No new ready-distribution claim follows from a version
bump. Real-provider authentication, outage recovery, supported browser/desktop
journeys and artifact-level install/upgrade/restore remain required before
release sign-off.

OIDC linking will require an existing authenticated account or invited new
enrollment. A verified email match alone must never attach an identity to an
existing account. Ownership will never be inferred from a provider claim.

## Configure a provider

System administrators use **System › Settings › Sign-in provider**. App
administrators and personal tokens cannot read or change these settings.
Enter the name, issuer, client ID and secret. Register the callback shown on
the form with your provider; it comes from the existing `REPOMEMO_PUBLIC_URL`
setting, never the incoming request's host. Enabling requires an HTTPS public URL.

**Check discovery** reads discovery and signing keys without saving. Saving an
enabled provider repeats those checks; a failure preserves the previous setting.
Successful discovery does not prove the client secret or a complete sign-in.
V0.5.1 prepares configuration; sign-in follows in the subsequent releases.

Supported configuration is deliberately bounded: public HTTPS issuer and all
endpoints on the same origin, authorization code, S256 PKCE, RS256 ID tokens
and `client_secret_post`. No redirects or environment proxy are used; resolved
IPs are checked and pinned. DNS has five seconds, requests ten seconds, and
JSON responses at most 256 KiB. Private/internal issuers, cross-origin provider
endpoints and other token-endpoint authentication methods are not supported yet.
An operator must not disable these checks to make a provider pass.

The secret is encrypted with the server's existing sealing key and never
returned. Leave the secret blank to retain it; replacing issuer or client ID
requires reentering it. Exact allowed email domains constrain later enrollment,
not access grants. Save events use the existing system audit store.

Migration 0045 adds server-level identity configuration. Workspace archives
exclude it and contain no provider secret. Full backups carry the database;
restore with the same sealing key. Older servers cannot open the migrated
database: use the pre-upgrade backup for rollback.

Protocol reference: [OpenID Connect Core](https://openid.net/specs/openid-connect-core-1_0.html).

## Authorization and handoff API

V0.5.2 implements the code flow for linked `(issuer, subject)` identities.
Explicit account linking/enrollment and client controls follow in V0.5.3–0.5.4;
an email match does not sign in to an existing account.

The initiating client posts to `/v1/auth/identity/start`, holds the returned
state and proof in memory, and opens the authorization URL in a browser.
The provider returns to the fixed callback. A single-use state, S256 verifier,
nonce, RS256 signature, exact issuer, audience/authorized party and fresh
authentication time are required. Token exchange and keys obey the same
egress and response bounds as discovery. No provider token is returned to a client.

The client posts state and proof to `/v1/auth/identity/complete`: 202 means
pending, success returns the existing session format, and failure means start
again. `X-RepoMemo-Client: web` puts the refresh credential in the existing
HttpOnly cookie. The proof never travels through the provider URL; callback
pages contain no access or refresh token. Handoffs expire after five minutes,
are consumed once, and are capped at 1,000 outstanding server-wide. Provider
configuration changes invalidate pending flows. Cancellation and expired or
replayed responses cannot issue a session.

Migration 0046 adds linked identities and sealed ephemeral handoffs. Neither
is included in workspace exports. Full backup restores retain identity links;
pending handoffs still expire at their original deadline. Live provider,
browser and desktop evidence remains pending.

## Link an existing account or enroll an invited person

V0.5.3 adds `GET /v1/profile/identity` and `POST /v1/profile/identity` for a
signed-in session to inspect or explicitly start linking its own account.
The callback requires both that session's authorization and provider proof;
session revocation invalidates the link request. A provider subject already
linked to another account cannot be moved. Changing the provider email does
not change the account behind an established issuer/subject pair.

For new enrollment, supply `invitation_token` in the JSON body of
`POST /v1/auth/identity/start`. The signed ID token must carry a verified email
matching the valid invitation and any configured domain allowlist. An email
already in use requires signing in to that existing account and linking it,
even when the provider calls the email verified. The existing invitation
acceptance path rechecks the inviter's authority and applies ordinary grants.

Migration 0047 adds the enrollment confirmation state. A concurrent callback
cannot sign in using a new identity before invitation acceptance finishes.
If enrollment was interrupted after account creation, the account remains
unconfirmed; recover its local account through the normal reset process and
explicitly link again. No support bypass or automatic email merge exists.

## Sign in with the provider

V0.5.4 adds "Continue with <provider>" to the sign-in page when a provider is
enabled, and "Create the account with <provider>" on an invitation for a new
account. The browser opens the provider page in a new tab; the desktop app
opens it in the system browser. The original window polls for the single-use
result every two seconds and always offers **Open it again** and **Cancel**.

Cancelling, provider refusal, an expired five-minute handoff or a changed
provider setting each return to the sign-in form with a message; local sign-in
stays available. The profile page shows the linked provider and starts explicit
linking, which does not replace the current session. Live browser, desktop and
provider journeys remain pending.

## Require provider sign-in

V0.5.5 lets a full system administrator turn on **Require provider sign-in**
in System settings. It can only be saved when the provider is enabled and
checked, when the administrator saving it has already linked their own account
to that provider, and when one to five active system administrators are named
as break-glass accounts. Local password sign-in, registration and password-reset
sign-in are then refused for everyone else, with a message naming the provider.
Existing sessions are not ended by this release.

Break-glass administrators keep password sign-in at the sign-in page under
"Administrator break-glass sign-in"; every such use is recorded in the system
audit trail. If every named break-glass account is deactivated or demoted, any
active system administrator keeps password access, so a working path always
remains. Turning enforcement off, disabling the provider or changing the issuer
requires removing the requirement first. Password-only servers are unchanged.
