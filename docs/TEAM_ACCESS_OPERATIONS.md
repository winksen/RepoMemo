# Team access: operating guide

For the people who run a RepoMemo server for a team: how to bring people in, move them, end their access and recover
when something goes wrong. It is a procedure; for what each feature does, see [Team access and identity](TEAM_ACCESS.md).

> **What this guide has not been checked against.** Every procedure below was built and compiled, but no real identity
> provider, browser or the desktop app was used to rehearse it. The [closing record](../mindmap/technical/release-evidence-0.5.19.md)
> has the rehearsal as a checklist with every result still pending. Do the rehearsal on a test server before relying on
> enforced provider sign-in.

## 1. Set up in this order

1. **Public HTTPS address first.** Provider sign-in needs the server's public HTTPS URL (the callback address in System settings is built from it).
2. **Two administrators, both with two-step sign-in.** Turn two-step on for every system administrator (Profile › Two-step sign-in) and keep each one's recovery codes outside RepoMemo. They are your break-glass accounts.
3. **Configure the provider** (System settings › Sign-in provider): issuer, client ID and secret, then *Check discovery*, then enable. Register the shown callback and, for logout, the back-channel address at the provider.
4. **Link yourself and sign in once** through the provider (Profile › Sign-in provider › Link, then sign out and *Continue with the provider*). Type the confirmation code the provider's return page shows.
5. **Try it with a second, ordinary account** before anyone else depends on it.
6. **Group mappings next**, previewed for a real person before saving (*Preview group changes*).
7. **Require provider sign-in last**, only after the break-glass administrators are named, and only after you have done §5 once.

## 2. Joiners, movers, leavers

| When | Do |
|---|---|
| **Someone joins** | Invite them to the organization or workspace; with the provider on, they open the invitation and *Create the account with the provider* (their provider email must match and be verified, and match the allowed domains if you set any). An existing account is never merged by email: they sign in locally and link the provider from their profile. |
| **Someone changes team** | Change their organization role or workspace membership; if you use group mappings, change their group at the provider and they follow at their next provider sign-in. Memberships you set by hand are never changed by a mapping. |
| **A contractor or guest** | Add them to the workspace only, then give them an end date (People › Guests). Renew by choosing a later day; after the date they have no access, and the membership stays so a renewal restores it. |
| **Someone leaves** | System › Users › *Offboard account…*: read the preview (what they own, their open tasks and reviews, sessions and tokens), choose who receives the work, confirm. It checks everything first, hands over, deactivates and reports whether revocation was verified. The audit trail records `user_offboarded`. Anonymize afterwards if policy requires it. |
| **The leaver is the only owner** | The recipient must belong to each organization or workspace that changes owner; the dialog says which one blocks it. |
| **The leaver is the last system administrator** | Make someone else a system administrator first. The server never offboards the last one. |

Remember that a **restricted** workspace adds nobody automatically and that group mappings never make anyone an owner.
Check *Who can open this workspace* (People page) before and after a change: it lists each person and why.

## 3. Tokens

- Create tokens **limited to the workspaces they need**; a token without a limit reaches everything its owner can open.
- Tokens made before V0.5.15 are unlimited. Review System › Users › *API token inventory* after upgrading and ask owners to use **Limit…** on long-lived ones.
- A token stops working in a workspace the moment its owner loses access there. An administrator can revoke any token from the inventory.
- Two-step sign-in and enforced provider sign-in do not apply to a token that already exists; revoke the tokens of anyone whose account you think is compromised (offboarding and *Sign out everywhere* do).

## 4. Recovery

| Problem | Procedure |
|---|---|
| **Someone lost their phone and has recovery codes** | They sign in with a recovery code (each works once), then Profile › Two-step sign-in › *Turn off* and set it up again. |
| **Lost the phone and the codes** | An administrator confirms identity **outside RepoMemo** (a call, in person), then System › Users › *Remove two-step sign-in…* and types the person's email. They are signed out everywhere and set it up again. There is no other bypass, and an administrator cannot read or skip a code. |
| **Locked out by wrong codes** | Wrong codes count like wrong passwords. Wait for the lockout to end, or use System › Users › *Lift sign-in lockout*. |
| **Provider outage or wrong settings while sign-in is required** | A break-glass administrator signs in under *Administrator break-glass sign-in* with password and code, turns *Require provider sign-in* off in System settings (or fixes the provider), and tells people. Every break-glass use is in the audit export (`break_glass_sign_in`). |
| **All named break-glass administrators are gone** | Any active system administrator can still use a password; the server never leaves a server without a path. Name new break-glass accounts straight away. |
| **A person's provider account was disabled** | They can keep a session only until the provider session limit (12 hours unless you changed it) unless the provider sends back-channel logout, which ends their sessions at once. Offboard them for immediate effect. |
| **An enrollment stopped half-way** | The account exists but cannot sign in with the provider. Issue a password reset (System › Users), then the person signs in locally and links the provider. |
| **A group mapping removed access it should not have** | Fix the mapping, preview, save, and have the person sign in again; or add their membership by hand, which a mapping never changes. |

## 5. Audit

Export **Identity and sign-in events** from System › Audit (JSON lines or CSV) on a schedule your policy needs.
It covers linking, enrollment, provider sign-in and failures, provider and mapping changes, enforcement, break-glass use, group
sync and provider logout. Details are fixed sentences: no tokens, codes, secrets or provider responses. Guest endings and
offboarding appear in the full export.

## 6. Limits you should know about

- **No live validation.** One provider, OpenID Connect with RS256, S256 PKCE and `client_secret_post`; no SAML, no SCIM, no several providers.
- **Provisioning is at sign-in.** Group-granted access changes only when the person signs in with the provider (or is removed by logout, the session limit or offboarding). There is no push deprovisioning.
- **Provider sign-in is not local two-step sign-in.** The provider must provide its own second factor.
- **Enforcement does not end existing sessions or tokens** when you switch it on.
- **Back-channel logout ends every session of the account**, not one device, because access tokens are not tracked individually.
- **Two-step sign-in is TOTP (SHA-1, six digits)** from an authenticator app: no QR code, no hardware keys, and no server-wide requirement that everyone use it.
- **System administrators open every workspace**, including restricted ones, as administrators; they can neither be restricted nor change the policy unless they own the workspace.
- **A guest's ending is audited hourly**, though access ends on time.
- **Pending provider sign-ins are capped at 1,000** at a time; local sign-in and break-glass are unaffected.
- **Limited tokens receive no notices**, because notices can name other workspaces.
- **Tests were compiled, not executed, by the author** of this line; the maintainer's own run is owed. One existing lint failure in the templates crate (a newer toolchain) is unrelated.
