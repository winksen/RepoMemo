`# Single sign-on (OIDC): the plan for the theme of V0.4.0
`
`> **Status:** **proposed again on 2026-10-27** as the theme of the 0.5.x line, with the releases renumbered: [plans/2026-10-27-0.5-single-sign-on.md](2026-10-27-0.5-single-sign-on.md). It was set aside on 2026-10-25: the maintainer chose [preparation for deployment](2026-10-25-deployment.md) as the theme of the 0.4.x line. This plan stays as the candidate for a later line, and the release numbers in it (V0.3.28–V0.3.37) are void. It was proposed on 2026-10-24, in the release that closes the 0.3.x line (`V0.3.27`), as the record of [decision 33](../ROADMAP.md#decisions-to-agree-before-work-starts) *The theme of V0.4.0*. The roadmap recommended single sign-on first, and this plan writes it down; **the maintainer confirms or changes the theme before any of it is built**, as with every decision before it. Nothing here is built.
`> **Method:** a reading of what sign-in is today ([§1](#1-what-exists-today)) and a list of single-purpose releases, numbered as 0.2.x was before `V0.3.0` and 0.3.x before this one: the work is built as `V0.3.28` onward, committed as `Update V0.3.N`, and `V0.4.0` is cut when the maintainer's tests of the clean install pass, as `V0.3.0` was.
`> **Related:** [roadmap](../ROADMAP.md) · [auth and authorization](../technical/auth-and-authorization.md) · [seventh security review](../security/2026-10-24-0.3-security-review.md)
`
`## Why this first
`
`The single-node server already holds the performance targets of [performance.md](../technical/performance.md), so scale-out is not what stops a larger organization. Sign-in is: a team of three signs in with an email and a password and an invitation link; a company of three hundred has an identity provider that already says who works there, who left yesterday and which group they are in. Without it an administrator invites each person by hand and removes each leaver by hand, and a password that the company cannot rotate or force a second factor on sits in the memory tool. Single sign-on removes that, and it is a change at the edge: the data model, the permissions, the search and the connectors stay as they are.
`
`## 1. What exists today
`
`| Part | Today |
`|---|---|
`| Accounts | An email, a name and an Argon2 password hash (`auth::hash_password`); a verified-email step ([verification.rs](../../apps/server/src/verification.rs)); sign-up is closed unless `REPOMEMO_ALLOW_REGISTRATION` is on, so people join by invitation (email-bound links) |
`| Sessions | An access token (a JWT, HS256, short) and a refresh token in a cookie, kept as a hash with the device and its reason for ending; `issue_session` creates them; sign out, sign out everywhere, and a version number on the account that ends all |
`| Other credentials | Personal access tokens with scopes (`read`, `write`, `admin`, `system`), made and shown once from a session |
`| Roles | System administrators (`REPOMEMO_SYSTEM_ADMIN_EMAILS` and the System area), organization owner / admin / member, workspace owner / admin / member / viewer; a membership remembers where it came from (`membership_origin`) |
`| Protection | Rate limits and lockouts for sign-in, the same answer for a known and an unknown address, a notice when a new device signs in |
`| Mail | Optional SMTP, with an outbox; sign-in does not need it except for verification and reset |
`
`## 2. Decisions to confirm
`
`| # | Decision | Proposal |
`|---|---|---|
`| 34 | **Protocol first** | OpenID Connect (authorization code with PKCE) for one identity provider per server. SAML, several providers and one provider per organization come later, if asked for; every large provider offers OIDC |
`| 35 | **Who gets an account** | The first sign-in of a person the provider vouches for (`email_verified`) matches the account with that verified address; with none, an account is made only if the address is in the allowed domains the administrator lists, or the person holds an invitation. Never by an unverified address, and never by name |
`| 36 | **Passwords** | Stay as they are, so the first administrator can always get in. A server administrator can turn password sign-in off for everyone but the accounts listed as break-glass; turning it off is refused until at least one administrator has signed in through the provider |
`| 37 | **Roles from the provider** | Optional: a claim (such as `groups`) maps to an organization role at each sign-in, recorded in the activity feed; a claim can raise or lower a member, admin or viewer role, but never remove the last owner, and system administration stays with `REPOMEMO_SYSTEM_ADMIN_EMAILS` |
`| 38 | **The desktop app** | Signs in through the system browser and a one-time code given back to the app (never a password in the app), because the window may not leave the bundled client |
`
`## 3. Releases
`
`One change each, as in every line before. The numbering continues from `V0.3.27`; `V0.4.0` is the tag after the last of them.
`
`### Track U: Sign in with the organization's identity provider
`
`| Release | Type | The one change | Done when |
`|---|---|---|---|
`| **V0.3.28** | ✨ | The provider's settings in System › Sign-in: issuer, client id, client secret (sealed like the other secrets), scopes, the allowed domains; a *Check* that reads the issuer's discovery document and its keys without signing anyone in | A wrong issuer, a missing key set or an address the server may not call (the egress rules of the AI providers apply) is refused with the reason |
`| **V0.3.29** | ✨ | The sign-in itself: start and callback routes with `state`, `nonce` and PKCE, the ID token checked (signature against the provider's keys, issuer, audience, expiry, nonce), and a session issued the way `issue_session` does today | A person with an existing account signs in through the provider and lands on their dashboard; a replayed, forged or expired response is refused |
`| **V0.3.30** | ✨ | Linked identities: a table of (issuer, subject) per account, kept as the account's way back in; a first sign-in matches by verified address, otherwise makes the account if the domain or an invitation allows it | A second sign-in follows the subject, not the address; a changed address at the provider does not hand the account to someone else |
`| **V0.3.31** | ✨ | The sign-in page and the desktop app: *Continue with <provider>*, errors as toasts, and the desktop flow through the system browser with a one-time code | The web page and the desktop app both sign in; a person who is refused is told why |
`| **V0.3.32** | ✨ | Turning passwords off: a server setting, break-glass accounts, the refusal until an administrator has signed in through the provider, and the sign-in page without the form | With passwords off, the listed accounts still get in and nobody else does |
`| **V0.3.33** | ✨ | Roles from the provider's groups, applied at each sign-in and written to the activity feed | A person moved to another group in the provider has the new role at the next sign-in; the last owner is never removed |
`| **V0.3.34** | 🔧 | Ending sessions with the provider: the provider's logout (RP-initiated) and, where the provider sends it, back-channel logout, plus a cap on how long a session may be refreshed | A leaver removed at the provider loses the session at the cap at the latest, and at once where back-channel logout is on |
`| **V0.3.35** | ✨ | Audit export: sign-ins, role changes and administration events as a file an auditor can take (CSV and JSON), from System › Audit trail and the console | An export for a date range holds who did what and from where, and nothing secret |
`| **V0.3.36** | 🔒 | A security pass over the identity surface: redirect and `state` handling, mix-up and token-substitution attacks, account linking, the discovery call (SSRF), the sealed secret, the break-glass accounts | No high or medium finding open |
`| **V0.3.37** | 📘 | Release checks repeated; the clean-install page gets the provider steps; the line is closed and the tag `V0.4.0` waits for the maintainer's tests | Every check passes; the clean-install record has a step for the provider |
`
`## 4. Risks and how the plan meets them
`
`| Risk | What the plan does |
`|---|---|
`| Taking over an account through an unverified or reassigned address | Only `email_verified` addresses match; after the first sign-in the (issuer, subject) pair decides (V0.3.30); a mismatch is refused, not merged |
`| The server behind a proxy builds the wrong redirect address | The callback address comes from `REPOMEMO_PUBLIC_URL` and nothing in the request; the settings page shows it for the provider to allow |
`| Locking everyone out | Passwords stay until an administrator has signed in through the provider; break-glass accounts; the console can turn the setting back |
`| The server calling an address an attacker chose | The issuer is the administrator's, checked against the same host rules as the AI providers; the discovery document's other addresses must share the issuer's host or be listed |
`| A forged or substituted token | Signature against the provider's keys (cached, refreshed on an unknown key id), issuer and audience, `nonce`, expiry with a small skew, `alg` fixed to what the provider lists, never `none` |
`| The client secret in the clear | Sealed with the same key as the other secrets, shown never, replaced by typing a new one |
`| Roles changed by a claim someone controls | Only a claim the provider signs, only the roles the administrator mapped, never the last owner, always in the activity feed |
`
`## 5. Left out, on purpose
`
`SAML; SCIM provisioning and deprovisioning; several providers or a provider for each organization; a hosted, multi-tenant service; and the other directions of [Later](../ROADMAP.md#-later-after-the-04x-line) (a scale-out server, more connectors, assistant actions), which wait for their turn after this one.
`
`## 6. What the maintainer decides now
`
`1. **The theme:** confirm single sign-on first (decision 33), or choose another direction from [Later](../ROADMAP.md#-later-after-the-04x-line); this plan is then rewritten or set aside.
`2. **Decisions 34–38**, each as proposed or changed.
`3. **The numbering:** `V0.3.28` onward until the tag, as proposed, or a `V0.4.x` line instead.
