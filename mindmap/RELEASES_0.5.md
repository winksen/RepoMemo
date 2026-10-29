# RepoMemo releases: 0.5 → 0.6

> **Current:** `V0.5.19` (2026-10-29)

> **Theme:** Team access and identity · **Baseline:** V0.4.104 · **Related:** [Roadmap](ROADMAP.md#proposed-path-to-10-team-readiness) · [Functional readiness report](analysis/2026-10-29-team-readiness.md)

**Status:** implementation authorized on 2026-10-29 through V0.5.19; completed rows are local implementation commits, with live validation separately recorded. One intended commit per row, named `Update V0.N.P`, with a short body describing the change. Split oversized rows before implementation and renumber only unshipped rows. Update the changelog, user/API guides, mindmaps, archive compatibility and focused checks with the implementation, not in this planning change. No product version is bumped by this plan.

**Entry:** [V0.4.105](technical/release-evidence-0.4.105.md) records candidate evidence and the validation scope. The maintainer authorized implementation overlap while live evidence remains pending. The existing OIDC plan supplies the identity design; the refinements below take precedence where they differ.

**Exit:** A new team can join through password/MFA or OIDC, keep a workspace restricted, inspect effective access, expire a guest and remove a leaver without losing owned work. Browser and supported desktop journeys have recorded evidence.

**Scope control:** local OCR (0.6.13), a second tracker (0.8.7) and a second chat adapter (0.8.10) require a pilot owner before implementation. Jira/Slack are the default first choices; pilot evidence may select Linear/Teams instead before work starts. Record a deferral and narrow the supported feature list when a conditional item is not justified. Access boundaries, trustworthy evidence, recovery and publication isolation are mandatory. Full details and pilot thresholds are in the [readiness report](analysis/2026-10-29-team-readiness.md).

**Type:** ✨ Feature · 🔧 Enhancement · 🐞 Fix · 🔒 Security · 📘 Update (docs, process, evidence, release).
**Status:** ✅ shipped (with date) · 🔄 in progress · 🔜 planned. Every row starts planned; a written plan is not an acceptance-test pass.

## Every release

| Release | Status | Track | Type | Area | What changes | Done when |
|---|---|---|---|---|---|---|
| **V0.5.0** | ✅ 2026-10-29 | AI. Team access | 📘 | Opening | Open the expanded team-access line and reconcile capability claims against code, including existing citation editing, guests, owner transfer and audit export | The release index, access model and guide agree; no shipped capability is planned as a new implementation |
| **V0.5.1** | ✅ 2026-10-29 | AI. Team access | ✨ | Identity | Configure and check one OIDC provider with sealed credentials and bounded discovery | An administrator can diagnose discovery/configuration errors without exposing secrets or enabling an untested provider |
| **V0.5.2** | ✅ 2026-10-29 | AI. Team access | ✨ | Identity | Implement the OIDC authorization and callback flow with state, nonce, PKCE and token validation | Valid sign-in issues the existing session; replay, wrong issuer/audience, expired tokens and mismatched state are refused |
| **V0.5.3** | ✅ 2026-10-29 | AI. Team access | 🔒 | Identity | Link accounts by issuer/subject, requiring explicit proof for an existing account rather than trusting an email match alone | A matching or reassigned email cannot take over an existing account; authenticated linking and invited new-account enrollment both work |
| **V0.5.4** | ✅ 2026-10-29 | AI. Team access | ✨ | Sign-in UX | Add provider sign-in in the browser and a system-browser handoff for desktop | Success, cancellation, expired handoff and provider failure return to the correct client with a recoverable next action |
| **V0.5.5** | ✅ 2026-10-29 | AI. Team access | 🔒 | Identity policy | Allow enforced SSO with tested break-glass access and guarded password disablement | An administrator cannot disable the last working access path; password-only deployments remain usable |
| **V0.5.6** | ✅ 2026-10-29 | AI. Team access | ✨ | Membership | Map allowlisted provider groups to existing organization/workspace memberships, retaining grant origins | Mappings preview additions/removals, never infer ownership, preserve direct grants and cannot silently demote the last owner |
| **V0.5.7** | ✅ 2026-10-29 | AI. Team access | 🔒 | Session lifecycle | Add provider logout and supported back-channel logout with an absolute reauthentication deadline | A disabled identity cannot refresh forever; an IdP without logout notifications has a documented, tested maximum access window |
| **V0.5.8** | ✅ 2026-10-29 | AI. Team access | 🔧 | Audit | Extend the existing audit/export path with identity linking, provider policy, login and group-mapping events | An authorized export explains identity changes and failures without tokens or secrets; no duplicate audit subsystem is created |
| **V0.5.9** | ✅ 2026-10-29 | AI. Team access | 🔒 | Identity review | Review and close findings on redirects, account linking, discovery, tokens and recovery | No high/medium identity finding remains open and negative cases have regression coverage |
| **V0.5.10** | ✅ 2026-10-29 | AI. Team access | 📘 | Identity checkpoint | Complete the original OIDC checkpoint using a real provider and supported browser/desktop/container paths | Provider sign-in and outage recovery are recorded against the candidate; this closes the identity track, not the expanded 0.5 line |
| **V0.5.11** | ✅ 2026-10-29 | AI. Team access | ✨ | Workspace access | Add explicit organization-shared versus restricted workspace membership policy | A restricted workspace never auto-enrolls a new org member; existing memberships migrate without silent loss or expansion and server-admin powers are explicit |
| **V0.5.12** | ✅ 2026-10-29 | AI. Team access | 🔧 | Access UX | Show effective access and its origin before membership or group-policy changes | An administrator can explain every grant and preview affected people; search, counts and direct URLs reveal no restricted workspace to unauthorized users |
| **V0.5.13** | ✅ 2026-10-29 | AI. Team access | ✨ | Guests | Add expiry and renewal to existing direct guest membership | An expired guest loses read, search, export, event and token access at the deadline; renewals and expiry are auditable |
| **V0.5.14** | ✅ 2026-10-29 | AI. Team access | 🔧 | Offboarding | Unify existing deactivation and ownership transfer with a preview of outstanding tasks and reviews | An admin reassigns work, preserves attribution and verifies revocation; the flow refuses orphaning the last owner |
| **V0.5.15** | ✅ 2026-10-29 | AI. Team access | 🔒 | API access | Constrain integration/PAT access to explicitly selected workspaces with expiry and an access inventory | A token cannot reach a different workspace, cannot outlive its grant and stops working when its owner loses access; existing-token migration is documented |
| **V0.5.16** | ✅ 2026-10-29 | AI. Team access | 🔒 | Local MFA | Add optional TOTP enforcement for local-password accounts, with hashed single-use recovery codes | Password alone cannot bypass enabled MFA through browser, token issuance or alternate login routes; replay and recovery reuse are refused |
| **V0.5.17** | ✅ 2026-10-29 | AI. Team access | ✨ | MFA UX | Add enrollment, recovery and administrator-policy screens for local MFA | Enrollment requires a successful code, recovery codes are shown once, reset requires verified authority and a user can recover without a support backdoor |
| **V0.5.18** | ✅ 2026-10-29 | AI. Team access | 🔒 | Access verification | Exercise the complete role, guest, token, MFA and restricted-workspace matrix | Denied access stays denied across caches, events, previews, exports and retrieval; upgrades retain intentional existing grants |
| **V0.5.19** | ✅ 2026-10-29 | AI. Team access | 📘 | Closing | Record team-access walkthroughs and publish identity/access operating guides | The line's exit journey passes on the supported distribution; MFA and OIDC recovery steps are rehearsed and residual limits are explicit |

## Verification and handoff

Before opening the next line, record the exit journey, changed-data upgrade and export/import checks, role-denial cases and supported-client behavior. Tests must demonstrate the user outcome, including failure and recovery. Follow the repository's execution rules; unrun checks remain explicitly unrun and the maintainer supplies live evidence.

Any new storage field, enum or entity must include its archive compatibility and migration handling in its own implementation; the line's portability row verifies the complete journey, rather than postponing data exit until the end.

When a row ships, update this index and the matching roadmap row together; put the actual behavior and limitations in the changelog. The version range is an ordered plan, not a calendar forecast.
