# V0.5.10 identity checkpoint evidence

Prepared 2026-10-29. Baseline: V0.5.9 (`aeacf59`). This closes the identity
**track** of 0.5.x (V0.5.1–V0.5.9), not the expanded 0.5 line. It is a local
candidate, not a published artifact, and **no live provider was available**:
the provider sign-in and outage-recovery journeys below are written as a
rehearsal for the maintainer and are recorded as pending, not as passed.

## What the identity track delivers

| Release | Capability | Evidence kind |
|---|---|---|
| V0.5.1 | One provider with sealed credentials and bounded, DNS-pinned discovery | Compiled tests |
| V0.5.2 | Authorization code flow, state, nonce, S256 PKCE, RS256 validation, single-use handoff | Compiled tests with a signed fixture token |
| V0.5.3 | Account linking by issuer/subject, invited enrollment, no email-only linking | Compiled tests |
| V0.5.4 | Browser sign-in, system-browser handoff for desktop | Compiled UI tests |
| V0.5.5 | Required provider sign-in with break-glass administrators | Compiled server test |
| V0.5.6 | Group mapping with preview, direct grants and owners kept | Compiled storage test |
| V0.5.7 | Absolute reauthentication deadline, provider logout, back-channel logout | Compiled server and storage tests |
| V0.5.8 | Identity audit events and an identity-scoped export | Compiled storage test |
| V0.5.9 | [Security review](../security/2026-10-29-identity-review.md); confirmation code | Compiled storage, server and UI tests |

## Candidate checks run for this record

| Check | Result | Scope |
|---|---|---|
| Version, migration lock (49 files), generated types, styles, accessibility rules, documentation links | Pass | Local source checks |
| TypeScript | Pass | `npm run typecheck` |
| Clippy with warnings denied, storage and server crates, all targets | Pass | Includes compiling every test |
| Rust format | Pass | `cargo fmt --all` |
| Executing any test (Rust or web) | **Not run** | The maintainer's own run is owed; commands in the changelog's V0.3.2 entry |
| `repomemo-templates` clippy | Existing failure, unrelated | A `needless_borrows_for_generic_args` lint from the newer toolchain in `crates/templates/src/tests.rs:1220`; not touched by this line |
| Desktop crate, container jobs, npm audit, cargo deny | **Not run** | Outside this record |

## Rehearsal for the maintainer (provider and outage recovery)

Use one real provider (a test tenant) and a test server on HTTPS. Record each
result in the table at the end.

1. **Configure.** System › Settings › Sign-in provider: name, issuer, client ID and secret; register the shown callback and, for logout, the back-channel address. **Check discovery** passes; an unreachable or wrong issuer shows a clear toast and saves nothing enabled.
2. **Link.** Sign in locally, open Profile › Sign-in provider › Link, finish at the provider, type the confirmation code. The profile lists the provider.
3. **Browser sign-in.** Sign out, **Continue with <provider>**, finish at the provider, type the code. Try a wrong code (rejected, still waiting), cancel at the provider (returns to the form), and wait out five minutes (expires, recoverable).
4. **Desktop.** The same, in the Windows desktop app: the system browser opens, the app waits, the code finishes it.
5. **Enrollment.** Invite a new address, open the invitation, **Create the account with <provider>**. An address that already has an account is refused with the linking instruction.
6. **Groups.** Map a provider group to a workspace role, preview for a named person, sign in with and without the group, and confirm a directly granted membership is untouched.
7. **Required provider sign-in.** Link yourself, name a second administrator as break-glass, turn the requirement on. A member's password sign-in is refused; the break-glass administrator signs in under "Administrator break-glass sign-in" and the use is in the audit export (`scope=identity`).
8. **Outage recovery.** Block the server's access to the provider (or disable the tenant). Provider sign-in fails with a message; the break-glass administrator signs in, turns the requirement off, and members use passwords again.
9. **Logout and deadline.** Sign out (the provider page opens when it has an end-session page). With back-channel logout registered, end the session at the provider and see the next request refused. Without it, see a session refused at the configured limit.
10. **Container.** Repeat steps 3 and 7 against the container deployment behind the real public HTTPS address.

| Step | Browser | Desktop | Container | Result |
|---|---|---|---|---|
| 1–2 configure, link | Pending | n/a | Pending | Pending |
| 3 sign-in, wrong code, cancel, expiry | Pending | Pending | Pending | Pending |
| 5 enrollment | Pending | n/a | Pending | Pending |
| 6 groups | Pending | n/a | Pending | Pending |
| 7–8 required sign-in and outage recovery | Pending | Pending | Pending | Pending |
| 9 logout and deadline | Pending | Pending | Pending | Pending |

## Distribution decision

Unchanged from [V0.4.105](release-evidence-0.4.105.md): no distribution is newly
certified. The identity track is **built and compiled but not live-validated**;
claims of provider interoperability are limited to what a standards-conforming
provider that advertises code flow, RS256, S256 PKCE and `client_secret_post`
should do. Linux/macOS and platform signing remain outside the validated scope.
No push, tag, publication or release sign-off is authorized.
