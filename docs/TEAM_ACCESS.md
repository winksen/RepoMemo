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

## Map provider groups to memberships

V0.5.6 lets a system administrator map allowlisted provider groups to existing
organization or workspace memberships in System settings, one line each:
`group | organization or workspace | target ID | role`. Organizations accept
`admin` or `member`; workspaces accept `admin`, `member` or `viewer`. Ownership
is never mapped. The ID-token claim that lists groups defaults to `groups`.

The mapping is applied at each provider sign-in. A token without the claim
changes nothing. **Preview group changes** shows, for a named person and a list
of groups, what would be added, changed, removed or kept, without doing it.
Only memberships a mapping created (recorded in the server-level table added by
migration 0048) are ever changed or removed; a membership held directly, or by
an owner, is kept as it is, and one whose role someone changed after the sync
is released from the sync rather than overwritten. A failed sync never blocks
sign-in. A sign-in that changes memberships records one system audit event.
Memberships are not re-evaluated between sign-ins (see session lifecycle).

## Provider logout and the session limit

V0.5.7 gives every session that began with the provider an **absolute
reauthentication deadline**: 12 hours by default, 1 to 720 hours in System
settings (*Provider session limit*). Renewing a session carries the deadline on
unchanged, so a person disabled at the provider cannot stay signed in by
refreshing; once it passes they must sign in at the provider again. This is the
documented maximum access window when a provider sends no logout notice.

Signing out of RepoMemo after a provider sign-in also leads the browser to the
provider's end-session page when the provider advertises one (the desktop app
opens the system browser). The provider can also end sessions itself with
OpenID Connect **back-channel logout**: register
`<public URL>/v1/auth/identity/backchannel-logout` with the provider. A signed,
single-use logout token for a known subject or session ends every session of the
linked account at once, because access tokens are not tracked individually. A
bad, unsigned, wrong-audience, expired or repeated token changes nothing. Front-
channel logout is not supported. Migration 0049 holds the session fields and the
used-token record; it is server-level data excluded from workspace archives.

## Identity audit events

V0.5.8 records identity activity in the one existing system audit trail; there
is no second audit store. Events are `identity_linked`, `identity_enrolled`,
`identity_sign_in`, `identity_sign_in_failed`, `identity_provider_enabled`,
`identity_provider_disabled`, `identity_provider_replaced`,
`identity_domains_updated`, `identity_group_mappings_updated`,
`identity_session_limit_updated`, `identity_provider_updated`,
`identity_enforcement_changed`, `identity_groups_synced`,
`identity_backchannel_logout` and `break_glass_sign_in`.

Details are fixed sentences. They never contain tokens, authorization codes,
client secrets, provider subjects or raw provider responses, and a failed
sign-in records the same refusal text the person saw. In System › Audit, the
export has an **Events** choice: *Identity and sign-in events* (or
`?scope=identity` on `GET /v1/system/audit/export`) returns only these events
as JSON lines or CSV; the default is still every event. The download itself is
recorded as before, naming the identity scope. Only full system administrators
export, as before.

## Confirmation code and the identity review

V0.5.9 closes a review of the identity work
([findings](../mindmap/security/2026-10-29-identity-review.md)). The one change
you will see: after you sign in at the provider, its return page shows an
**eight-digit confirmation code**, and the window that started the sign-in asks
you to type it. Without it the sign-in is never released, and three wrong codes
cancel it. This stops a sign-in link that someone else started from handing
them your session if you complete it. Only type a code from a page you opened
yourself from RepoMemo. `POST /v1/auth/identity/complete` accepts an optional
`code` and answers `202 {"status":"confirm"}` until it is given, `409` for a
wrong code and `400` once three are used. Linking an account from the profile
uses the same step.

## Rehearsing the provider journeys

V0.5.10 closes the identity track as built but **not live-validated**: no real
provider was available. The [evidence record](../mindmap/technical/release-evidence-0.5.10.md)
has a ten-step rehearsal for an administrator with a test provider tenant
(configure, link, browser and desktop sign-in with the confirmation code,
enrollment, group mapping, required sign-in with break-glass, outage recovery,
logout and the session limit, and the container path) and a result sheet whose
cells are all pending. Do the outage-recovery step before relying on required
provider sign-in.

## Shared and restricted workspaces

V0.5.11 gives every workspace an access policy. **Shared** is how workspaces
have always worked: people who join the organization are added to the workspace
with a role that follows their organization role. **Restricted** never adds
anyone because of an organization role, now or later; its people are added one
by one (directly, by invitation or by a group mapping), and it starts with its
creator alone as its owner.

Every workspace that existed before the upgrade is **shared**, so nobody gains
or loses access. Changing the policy moves nobody: restricting a workspace keeps
everyone in it and turns the memberships that followed an organization role into
the workspace's own, so a later organization role change does not alter them;
making a restricted workspace shared adds nobody who is already in the
organization. Owners change it in the workspace's People page (*Who joins
automatically*), and choose it when creating a workspace (*Restricted*). A
full system administrator who changes it does so as the owner, and the change
is recorded in the workspace activity and the system audit. Organization
owners and administrators are not workspace members unless added, but a full
system administrator can still open every workspace; that power is separate
from, and unchanged by, this policy. Migration 0050 adds the policy.

## Effective access and previews

V0.5.12 explains access before it is changed. A workspace's **People** page now
lists everyone who can open it and why: *added directly*, *follows the
organization role* (with the provider group behind that organization role when
there is one) or *granted by a provider group*, and names the system
administrators who can open every workspace without being members. Changing the
shared/restricted policy first shows its effect and asks to confirm: nobody is
added or removed, and the count of people whose access stops following the
organization role (or the organization members who stay outside) is stated.

In System settings, **Preview group changes** now also lists the existing
group-granted memberships that a changed mapping would remove at the holders'
next provider sign-in; leave the email empty to see only that.

A restricted workspace is not disclosed to people who were not added: it is
absent from their workspace list, dashboard and cross-workspace search results,
and the address of a hidden workspace answers exactly as the address of one that
does not exist. Organization owners and administrators are covered by this too.
System administrators remain the explicit exception.

## Guest expiry and renewal

V0.5.13 lets a workspace owner or administrator give a **guest** (someone in
the workspace who is not in its organization) an end date, in the workspace's
*People › Guests* panel. The date is the end of a day in UTC, must be in the
future and at most a year ahead, and can be renewed with a later day or removed.
People in the organization and the workspace owner cannot have one.

From that moment the guest is no longer a member: the workspace is gone from
their list, dashboard, search, tasks and picture visibility; exports, reads and
events are refused; an open event stream is closed at the deadline; and their
personal tokens stop reaching it, because every one of these asks the same
membership check. The membership stays on record, so a renewal restores the
same role with no re-invitation. Setting, renewing and removing a date are
recorded in the workspace activity and the system audit; the end itself is
recorded once as `guest_access_expired` by the hourly maintenance pass (access
ends at the date whether or not that pass has run). Administrators cannot
change an administrator's date. Existing memberships have no end date.
Migration 0051 adds the two fields.
