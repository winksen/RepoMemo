# Team access and identity

## Current access model

At V0.5.0, accounts sign in with passwords. OpenID Connect, local MFA,
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
