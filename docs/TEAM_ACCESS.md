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
