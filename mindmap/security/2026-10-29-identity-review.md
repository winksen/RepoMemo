# Identity security review — 2026-10-29 (V0.5.9, on what V0.5.1–V0.5.8 added)

> **Scope:** provider sign-in (V0.5.1–V0.5.4): discovery, the authorization and callback flow, token validation and the browser/desktop handoff; account linking and invited enrollment (V0.5.3); enforced provider sign-in and break-glass access (V0.5.5); group mapping (V0.5.6); the session deadline and logout (V0.5.7); identity audit events (V0.5.8).
> **Method:** a static reading of the code at `V0.5.8`, asking of each place what a person on the network, a malicious or compromised provider, someone who phishes a colleague, a person whose provider account was disabled and an administrator who makes a mistake could do. No live provider was involved.
> **Status:** closed on 2026-10-29 at `V0.5.9`. **0 high, 1 medium, 1 low, 4 info:** the medium and the low are fixed with a test each; none is open. The informational findings are accepted and written down.
> **Previous review:** [2026-10-27](2026-10-27-deployment-security-review.md) (the eighth pass, on deployment).

| ID | Severity | Finding | Status | Release |
|---|---|---|---|---|
| I1 | Medium | **A provider sign-in that someone else started could be finished by their target.** `POST /v1/auth/identity/start` is public and hands its caller the polling proof. An attacker could start a sign-in, send the real provider address to a colleague, and — if the colleague signed in at the provider — collect that colleague's session by polling with the proof. | Fixed: the return page shows an eight-digit confirmation code and the result is released only with it. Three wrong codes destroy the handoff. Browser and desktop use the same step. | V0.5.9 |
| I2 | Low | **A back-channel logout token without `exp` could be replayed.** Its identifier was remembered for an hour at most from its arrival, while the issue-time window accepted it for ten minutes from `iat`; with `exp` absent the record could be purged early. | Fixed: the identifier is remembered at least until ten minutes and a minute after `iat`. | V0.5.9 |
| I3 | Info | Up to 1,000 provider sign-ins can be pending at once and further starts are refused for up to five minutes. An attacker with many addresses could fill the pool. | Accepted: the existing per-address sign-in quota applies to `start`, the pool is a safety bound, and local sign-in and break-glass access are unaffected. | — |
| I4 | Info | Memberships granted by group mapping are re-evaluated only at the next provider sign-in. A person removed from a provider group keeps access until then or until their session deadline. | Accepted and documented: the session limit (default 12 hours) bounds it; back-channel logout ends it at once when the provider sends one. | — |
| I5 | Info | An enrollment interrupted after the account was created leaves an unconfirmed link that cannot sign in and blocks that provider subject. | Accepted: the account is recovered through the normal reset and an explicit link; there is no support bypass. | — |
| I6 | Info | Existing sessions are not ended when enforcement is turned on. | Accepted: they end at their own expiry. An administrator who needs it sooner uses *Sign out everywhere* on the accounts. | — |

| Severity | Count | IDs |
|---|---:|---|
| Critical | 0 | — |
| High | 0 | — |
| Medium | 1 | I1 |
| Low | 1 | I2 |
| Info | 4 | I3, I4, I5, I6 |

---

## 1. Redirects and the callback

The authorization address is built on the server from the discovered endpoint, which must share the issuer's HTTPS origin. The callback is fixed, takes only `state`, `code` and `error`, and answers with a short text page that carries no session token and sets `Cache-Control: no-store` and `Referrer-Policy: no-referrer`. No parameter of any request chooses where a browser is sent, so there is no open redirect. The state is single use and claimed before any network call, so a replay cannot exchange a code twice.

## 2. Account linking and enrollment

Linking is by issuer and subject only. An existing account needs the signed-in session's own authorization, checked again with its session version when the callback completes, and a subject linked elsewhere cannot be moved. A matching or later-reassigned provider email never selects an account. Enrollment is insert-only, needs a valid invitation for the verified provider email and the domain allowlist, and stays unconfirmed (unable to sign in) until the invitation is accepted. Finding I1 is the one place this chain was weak: the identity was proven, but the *client* that received the session was not.

## 3. Discovery and egress

Every provider request resolves the host once, rejects non-public addresses and metadata hosts, connects only to the checked addresses, uses no proxy, follows no redirect and bounds time and size. Endpoint addresses must share the issuer's origin and the discovery issuer must equal the configured one exactly. Error messages never include response bodies, request addresses or credentials.

## 4. Tokens

Only RS256 with a unique signing key of the right use is accepted. The issuer, audience, authorized party, expiry, issue time, authentication time (the request asks for `max_age=0`) and nonce are checked, and `email_verified` must be literally true to enroll. Logout tokens additionally need the back-channel event, no nonce and a single-use identifier (I2).

## 5. Recovery and policy

Enforced provider sign-in leaves password access only to named active system administrators, records each use and falls back to any active system administrator if none of them remains, so a mistake cannot remove the last way in. The password-reset and registration routes honor the same policy. An enrolled account has an unusable random password until an administrator issues a reset, which the same policy then limits.

## 6. Regression coverage

| Finding | Test |
|---|---|
| I1 | `a_finished_handoff_needs_the_confirmation_code_and_three_wrong_codes_destroy_it` (storage); `identity_completion_issues_the_existing_session_once` (server: no code, wrong code, right code, replay); `providerSignIn.test.tsx` |
| I2 | `provider_logout_finds_accounts_and_refuses_a_repeated_token` (storage) covers the single-use record; the window is the maximum of `exp` and `iat` plus 660 seconds in `backchannel_logout` |
| Linking, discovery, tokens | `identity_linking_never_reassigns_email_or_subject`, `identity_tokens_require_signature_issuer_audience_nonce_and_freshness` |

None of these tests was executed by the author of this review; they were compiled and the maintainer's run is owed.
