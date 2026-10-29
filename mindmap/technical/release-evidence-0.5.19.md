# V0.5.19 closing record: team access and identity

Prepared 2026-10-29. Baseline: V0.5.18. This closes the 0.5.x line locally. It is a local candidate, not a published
artifact. **None of the walkthroughs below has been performed**: no identity provider, browser, desktop app or container
deployment was available, and the author did not execute any test. They are written so the maintainer can do them and
record the result; every result cell is pending.

## What the line delivers

| Releases | Capability | Where it is described |
|---|---|---|
| V0.5.0 | Opening: capability reconciliation | [Team access](../../docs/TEAM_ACCESS.md) |
| V0.5.1–V0.5.4 | One OIDC provider; authorization code flow with state, nonce, PKCE and RS256 validation; linking and invited enrollment; browser sign-in and desktop system-browser handoff | TEAM_ACCESS sections 1–4 |
| V0.5.5–V0.5.8 | Required provider sign-in with break-glass; group mapping; session deadline and logout; identity audit events and export scope | TEAM_ACCESS |
| V0.5.9–V0.5.10 | [Security review](../security/2026-10-29-identity-review.md); [identity checkpoint](release-evidence-0.5.10.md) | those records |
| V0.5.11–V0.5.15 | Restricted workspaces; effective-access previews; guest expiry; guarded offboarding; workspace-limited tokens and inventory | TEAM_ACCESS |
| V0.5.16–V0.5.17 | Local TOTP two-step sign-in, recovery codes and screens | TEAM_ACCESS |
| V0.5.18 | The access matrix | TEAM_ACCESS |
| V0.5.19 | [Operating guide](../../docs/TEAM_ACCESS_OPERATIONS.md); this record | |

Migrations added by the line: 0045–0053 (provider, flows, enrollment, group grants, provider sessions, workspace policy,
guest expiry, token workspaces, two-step sign-in). All are locked; none rewrites existing access.

## Checks run for this record

| Check | Result | Scope |
|---|---|---|
| Version, migration lock (53 files), generated types, styles, accessibility rules, documentation links | Pass | Local source checks |
| TypeScript | Pass | `npm run typecheck` |
| Clippy with warnings denied, server and storage crates, all targets | Pass | Includes compiling every test |
| Rust format | Pass | `cargo fmt --all` |
| Executing any test (Rust or web) | **Not run** | The maintainer's own run is owed |
| `repomemo-templates` clippy | Existing failure | A newer-toolchain lint in `crates/templates/src/tests.rs`, untouched by this line |
| Desktop crate, container jobs, npm audit, cargo deny, published artifacts | **Not run** | Outside this record |

## The exit journey (do this on a test server)

One continuous journey for a small team, with a test provider tenant. Record each step in the sheet below.

1. **Set up** following the [operating guide §1](../../docs/TEAM_ACCESS_OPERATIONS.md#1-set-up-in-this-order): two administrators with two-step sign-in, the provider checked and enabled, yourself linked, a second ordinary account signed in through the provider with its confirmation code.
2. **Join:** invite a new address; it opens the invitation and creates its account through the provider. Invite an address that already has an account: it is told to link instead.
3. **Restricted workspace:** create one; add one member; a second organization member joins the organization and does *not* appear in it; the People page explains each person's access.
4. **Groups:** map a provider group to a workspace role, preview, sign in with and without the group, and see a hand-made membership untouched.
5. **Guest:** invite a contractor to the workspace only, give them an end date a few minutes ahead, see the workspace disappear at the date (list, search, an open event stream, a token of theirs), then renew and see it return.
6. **Tokens:** create a token limited to the workspace; it reads there and is refused elsewhere; the inventory lists it; remove its owner from the workspace and the token is refused.
7. **Two-step sign-in:** set it up for a member; sign in with a code; replay the same code (refused); use a recovery code (works once, then refused); a password reset link without a code does not sign in.
8. **Leave:** offboard a member who owns a workspace and has an open task and review; read the preview; the work moves; the result says revocation was verified; their old session and token stop.
9. **Require provider sign-in:** turn it on; a member's password sign-in is refused; a break-glass administrator signs in and the use is in the identity audit export.
10. **Audit:** export *Identity and sign-in events* and check that it explains steps 1–9 and contains no token, code or secret.

## Recovery rehearsals

| Rehearsal | Steps | Expected |
|---|---|---|
| **Lost authenticator and codes** | A member with two-step on says they lost both; an administrator confirms identity outside RepoMemo, uses *Remove two-step sign-in…* and types the email | The member is signed out everywhere, signs in with the password, sets it up again; the audit shows `mfa_reset_by_administrator`; no other way exists |
| **Lost authenticator, has a recovery code** | Sign in with a recovery code, turn two-step off, set it up again | The code works once |
| **Provider outage while sign-in is required** | Block the server's access to the provider (or disable the tenant) | Provider sign-in fails with a message; a break-glass administrator signs in, turns the requirement off, members use passwords; the audit shows `break_glass_sign_in` and the enforcement change |
| **Provider account disabled** | Disable a signed-in person at the provider | Their session ends at the provider session limit, or at once with back-channel logout registered; offboarding ends it immediately |

## Result sheet

| Step | Browser | Desktop | Container | Result |
|---|---|---|---|---|
| 1 setup and first provider sign-in | Pending | Pending | Pending | Pending |
| 2 join and the existing-account refusal | Pending | n/a | Pending | Pending |
| 3 restricted workspace | Pending | n/a | Pending | Pending |
| 4 groups | Pending | n/a | Pending | Pending |
| 5 guest end and renewal | Pending | n/a | Pending | Pending |
| 6 limited token | Pending | n/a | Pending | Pending |
| 7 two-step sign-in | Pending | Pending | Pending | Pending |
| 8 offboarding | Pending | n/a | Pending | Pending |
| 9 required provider sign-in | Pending | Pending | Pending | Pending |
| 10 audit export | Pending | n/a | Pending | Pending |
| Each recovery rehearsal | Pending | Pending | Pending | Pending |

## Residual limits

The explicit list is [§6 of the operating guide](../../docs/TEAM_ACCESS_OPERATIONS.md#6-limits-you-should-know-about).
In short: nothing was live-validated; there is one OIDC provider and no SAML or SCIM; provisioning happens at provider
sign-in; provider sign-in does not use local two-step sign-in; enforcement does not end existing sessions or tokens;
two-step sign-in is TOTP only with no server-wide requirement; system administrators open every workspace.

## Distribution decision

Unchanged from [V0.4.105](release-evidence-0.4.105.md) and [V0.5.10](release-evidence-0.5.10.md): no distribution is newly
certified, and the line's exit journey has **not** passed on the supported distribution; it is written and waiting for the
maintainer. Linux and macOS and platform signing remain outside the validated scope. No push, tag, publication or release
sign-off is authorized.
