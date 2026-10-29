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
automatically*), and choose it when creating a workspace (*Restricted*). Only
the workspace's owner changes it (corrected in V0.5.18: an earlier version of
this guide said a system administrator could); the change is recorded in the
workspace activity and the system audit. Organization owners and
administrators are not workspace members unless added. A full system
administrator can still open every workspace as an administrator, but is not its
owner and cannot change the policy: hand the workspace to someone first, for
example by offboarding the owner. That power is separate from, and unchanged by,
this policy. Migration 0050 adds the policy.

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

## Offboarding

V0.5.14 adds **Offboard account…** to System › Users, one guarded step that
joins what used to be separate: transferring ownership, handing over work and
deactivating. The dialog first shows what the account owns, its open tasks and
reviews (reviews are tasks tied to a file) and the sessions, personal tokens and
sent invitations that will end. If there is anything to hand over, you choose
who receives it; that person must be able to open each workspace that has open
tasks and must belong to each organization or workspace that changes owner.

Everything that could refuse is checked **before** anything changes: your own
account, an already deactivated account, the last system administrator (which
cannot be offboarded, so the server is never left without one), and a recipient
who cannot take over. Then ownership moves (the previous owner stays an
administrator, as with any transfer, so no workspace or organization is ever
left without an owner), open tasks and reviews are reassigned with a line in
each workspace's activity, and the account is deactivated. Whoever created a
task, file or comment stays its creator: attribution is not rewritten.

The result states whether revocation was **verified**: no active session,
personal token, sent invitation or open task remains for the account. The audit
trail records `user_offboarded` with the counts. Anonymizing remains a separate
step afterwards. The older *Deactivate account* action still works and still
refuses an owner. `GET /v1/system/users/{user_id}/offboarding` is the preview
and `POST …/offboard` the action; both are for full system administrators.

## Workspace-limited tokens and the token inventory

V0.5.15 lets a personal access token be **limited to chosen workspaces**.
When creating a token (Profile › API tokens) switch on *Limit to chosen
workspaces* and tick the workspaces; each must be one you can open now. A limited
token cannot read, search, export or stream any other workspace, and does not see
other workspaces in lists, the dashboard, tasks or notices. A token with the
`system` scope cannot be limited.

A token never outlives its grant. It still ends at its expiry (7, 30, 90 or 365
days), and because every request is checked against its owner's current
membership, it stops working in a workspace the moment its owner loses access
there (removal, guest expiry, offboarding or a restricted-workspace change), with
no separate cleanup. Deleting a workspace removes it from every token's list; a
limited token left with none reaches nothing.

**Existing tokens (migration 0052).** Tokens made before this release are not
limited: they reach everything their owner can open, as before. Their owner can
narrow one in place with **Limit…** (`PUT /v1/profile/tokens/{id}/workspaces`);
a limited token can never be made unlimited again, so create a new token for
that. Nothing was changed for you automatically.

**Inventory.** System › Users lists every live token with its owner, scope,
whether it is limited, expiry and last use (`GET /v1/system/tokens`, full
system administrators), and lets an administrator revoke any of them
(`DELETE /v1/system/tokens/{id}`, audited as `token_revoked_by_administrator`).
Secrets are never shown. A per-workspace token view is not included.

## Two-step sign-in for local accounts

V0.5.16 adds optional **time-based one-time passwords (TOTP)** for accounts that
sign in with a local password. The enrollment screens arrive in V0.5.17; this
release delivers the enforcement and the API, and the sign-in page already asks
for the code when an account needs one.

Turning it on: `POST /v1/profile/mfa/start` returns a new secret and the
`otpauth://` address an authenticator app scans; `POST /v1/profile/mfa/confirm`
with a first valid code turns it on and returns **ten recovery codes, shown
once** and kept only as hashes. Until confirmed, an unconfirmed secret enforces
nothing. `GET /v1/profile/mfa` says whether it is on and how many recovery codes
remain.

From then on a correct password alone never opens a session. Local sign-in, and
the sign-in that ends a password reset (`mfa_code` in the body), answer
`403 mfa_required` until a current six-digit code or an unused recovery code is
sent. Each time step is accepted once, so a code that was seen cannot be
replayed; a recovery code works once, in any case and with or without its dash.
A wrong code counts as a failed sign-in and locks the account like a wrong
password. Turning it off or replacing the recovery codes needs the password and a
code. Sessions, refresh tokens and API tokens that already exist are not asked
again, and creating a token needs a signed-in session; sign-in through the
identity provider is not covered, because the provider is responsible for its
own second factor.

A person who lost both the device and the codes is helped by a full system
administrator with `POST /v1/system/users/{user_id}/mfa-reset`: it removes the
second factor, ends the person's sessions and is audited
(`mfa_reset_by_administrator`). There is no other bypass. Migration 0053 holds
the sealed secret, the last accepted step and the recovery code hashes.

## Two-step sign-in screens

V0.5.17 adds the screens for the V0.5.16 enforcement.

**For you (Profile › Two-step sign-in).** *Set up* shows a key to type into an
authenticator app, and an `otpauth://` link for apps that open it (there is no
QR code). Two-step sign-in is turned on only when you type a right first code.
You are then shown ten recovery codes **once**: copy them, and confirm you have
saved them before the panel lets you close it; they are not kept in the browser.
*Replace recovery codes* and *Turn off* ask for your password and a code; the
panel warns when two or fewer codes are left.

**At sign-in and password reset.** The sign-in page and the reset-link page ask
for an *Authentication code* (an app code or a recovery code) when the account
needs one. A wrong code keeps the reset link valid.

**For administrators (System › Users).** Accounts with it on are marked
"two-step on". *Remove two-step sign-in…* is the only recovery for someone who
lost both their device and their codes: it asks the administrator to confirm,
outside RepoMemo, that it really is that person, and to type the person's email
address; it signs them out everywhere and is audited. There is no support
backdoor and no way to read or bypass a code. Not included: a server-wide
requirement that everyone (or all administrators) use two-step sign-in, and QR
codes.

## The access matrix

V0.5.18 writes down, and tests, who can do what in a **restricted** workspace.
One test (`access_matrix`) has every actor try the same routes: reading
(overview, files, tasks, search, the event stream), administering (people,
who-can-open-it, export) and owning (the access policy).

| Actor | Read | Administer | Change the policy |
|---|---|---|---|
| Owner | yes | yes | yes |
| Workspace administrator | yes | yes | no |
| Member, viewer | yes | no | no |
| Guest within their end date | yes | no | no |
| Token limited to this workspace (owner is a member) | yes | no | no |
| System administrator, not a member | yes, as an administrator | yes | no |
| Guest after the end date | no | no | no |
| Someone never added, an organization owner or administrator not added | no | no | no |
| Token limited to another workspace | no | no | no |

Every refusal is `403` with the same answer a workspace that does not exist
gives; the workspace is absent from the refused person's list, dashboard and
cross-workspace search. The system administrator row is the one stated
exception, not a gap. The test also checks that the probing changed nobody's
membership. A second test checks that what was created before 0.5 keeps its
meaning after the upgrade: shared workspaces, no end dates, unlimited tokens, no
second factor, and organization members still enrolled in shared workspaces.

What the matrix does not cover, because it needs a live provider or browser:
provider sign-in and group sync against a real provider, the desktop app, the
event stream seen from a browser, and the exact moment a stream closes at a
guest's deadline (it is rechecked at the deadline and at the periodic recheck).
**Correction:** the V0.5.11 section above said a system administrator could
change the policy; the matrix showed only the owner can, and this guide was fixed.
